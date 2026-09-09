//! 의미 색인을 만든다 — 오래 걸리는 일이라, 진행률·멈추기·이어서 하기가 함께 있다.
//!
//! **등록과 색인을 나눈 까닭.** PDF 를 등록하고 글자를 뽑는 일은 몇 초면 끝나지만,
//! 벡터를 만드는 일은 청크 하나에 0.8초쯤 걸린다(bge-m3, CPU). 368쪽 지침이면
//! 6분에 가깝다. 등록할 때 함께 해 버리면 사용자는 "등록이 왜 이렇게 느리지" 라고
//! 여긴다. 그래서 등록은 바로 끝내고 낱말 검색부터 되게 하고, 색인은 사용자가
//! 시작한다.
//!
//! **청크 하나마다 저장한다.** 문서 단위로 모아서 저장하면, 5분 뒤에 멈춘 사용자가
//! 5분을 버린다. 청크마다 저장하면 멈춰도 이어서 할 수 있고, 앱을 껐다 켜도 남는다.

use crate::ai::{catalog, ollama};
use crate::error::{AppError, AppResult};
use crate::repo::{embed_index, setting, vector};
use crate::state::AppState;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{Emitter, State};

/// 진행률을 이 이름으로 화면에 보낸다
pub const INDEX_EVENT: &str = "ai://index";

/// 한 번에 이만큼 묶어 보낸다.
///
/// Ollama 는 글을 하나씩 셈하므로 묶음 크기가 속도를 크게 바꾸지 않는다.
/// 대신 **묶음이 클수록 진행률이 뜸하게 움직인다** — 8개면 6초에 한 번이다.
/// 4개면 3초마다 움직인다. 기다리는 사람에게는 그 차이가 크다.
const BATCH: usize = 4;

/// 남은 시간을 이 만큼의 최근 기록으로 셈한다
const RECENT: usize = 8;

#[derive(Default)]
pub struct IndexControl(pub Mutex<Option<Arc<AtomicBool>>>);

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IndexEvent {
    pub document_id: i64,
    pub title: String,
    /// 쓰고 있는 검색 모델의 보이는 이름
    pub model_name: String,
    pub model_tag: String,
    pub total: i64,
    pub done: i64,
    pub percent: u8,
    pub elapsed_ms: i64,
    /// 남은 시간 어림. 처음에는 카탈로그의 어림값, 그 뒤에는 실제 속도로 고친다
    pub remaining_ms: Option<i64>,
    /// running | paused | done | failed
    pub state: &'static str,
    pub error: Option<String>,
    /// 이 판에서 아직 남은 문서 수 (여러 개를 걸었을 때)
    pub queued: i64,
}

/// 지금 쓰기로 한 검색 모델
fn current_model(state: &AppState) -> AppResult<&'static catalog::ModelSpec> {
    let id = state.db()?.with(|c| setting::get(c, setting::EMBED_MODEL))?;
    catalog::by_id(&id)
        .filter(|m| m.role == catalog::Role::Embed)
        .ok_or_else(|| {
            AppError::msg(format!(
                "설정에 담긴 검색 모델({id})을 모릅니다. 설정에서 다시 골라 주세요."
            ))
        })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexOverview {
    pub model: catalog::ModelSpec,
    pub documents: Vec<embed_index::DocIndex>,
    /// 색인이 지금 돌고 있는가
    pub running: bool,
}

#[tauri::command]
pub fn index_status(
    state: State<'_, AppState>,
    control: State<'_, IndexControl>,
    collection_id: Option<i64>,
) -> AppResult<IndexOverview> {
    let model = current_model(&state)?;
    let ids: Vec<i64> = collection_id.into_iter().collect();
    let documents = state
        .db()?
        .with(|c| embed_index::status(c, model.tag, model.dim, &ids))?;
    let running = control.0.lock().map(|g| g.is_some()).unwrap_or(false);
    Ok(IndexOverview {
        model: model.clone(),
        documents,
        running,
    })
}

#[tauri::command]
pub fn index_collection_summary(
    state: State<'_, AppState>,
    collection_id: i64,
) -> AppResult<embed_index::CollectionIndex> {
    let model = current_model(&state)?;
    state
        .db()?
        .with(|c| embed_index::collection_summary(c, model.tag, model.dim, collection_id))
}

/// 검색 모델 목록과 지금 고른 것
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedModels {
    pub models: Vec<catalog::ModelSpec>,
    pub current: String,
}

#[tauri::command]
pub fn index_models(state: State<'_, AppState>) -> AppResult<EmbedModels> {
    let current = state.db()?.with(|c| setting::get(c, setting::EMBED_MODEL))?;
    Ok(EmbedModels {
        models: catalog::MODELS
            .iter()
            .filter(|m| m.role == catalog::Role::Embed)
            .cloned()
            .collect(),
        current,
    })
}

/// 검색 모델을 바꾼다.
///
/// **여기서 벡터를 지우지 않는다.** 모델을 바꿨다고 옛 벡터를 지우면, 잘못
/// 눌렀을 때 되돌릴 수 없다. 대신 그 문서는 "다른 모델로 색인됨" 으로 보이고,
/// 사용자가 [현재 모델로 다시 색인] 을 누를 때 지운다 (요구사항 6).
#[tauri::command]
pub fn index_set_model(state: State<'_, AppState>, model_id: String) -> AppResult<()> {
    catalog::by_id(&model_id)
        .filter(|m| m.role == catalog::Role::Embed)
        .ok_or_else(|| AppError::msg(format!("검색 모델이 아닙니다: {model_id}")))?;
    state
        .db()?
        .with(|c| setting::set(c, setting::EMBED_MODEL, &model_id))
}

/// 색인을 시작한다(또는 이어서 한다). 진행률은 `ai://index` 로 나간다.
///
/// `reindex` 가 참이면 지금 모델로 쓸 수 없는 벡터를 먼저 지운다 —
/// 모델을 바꿨거나 자료가 바뀐 경우다.
#[tauri::command]
pub async fn index_start(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    control: State<'_, IndexControl>,
    document_ids: Vec<i64>,
    reindex: bool,
) -> AppResult<()> {
    let model = current_model(&state)?;

    // AI 실행환경이 없으면 시작하지 않는다. 반쯤 하다 멈추는 것보다 낫다.
    let engine = crate::ai::status();
    if engine.engine != crate::ai::EngineState::Ready {
        return Err(AppError::msg(format!("{} {}", engine.detail, engine.hint)));
    }
    let has_model = engine
        .installed
        .iter()
        .any(|m| catalog::by_tag(&m.tag).map(|s| s.id) == Some(model.id));
    if !has_model {
        return Err(AppError::msg(format!(
            "검색 모델 '{}' 이 아직 없습니다. [AI 기능 설치] 에서 먼저 받아 주세요.",
            model.name
        )));
    }

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = control
            .0
            .lock()
            .map_err(|_| AppError::msg("색인 상태를 확인하지 못했습니다."))?;
        if guard.is_some() {
            return Err(AppError::msg("이미 색인이 돌고 있습니다."));
        }
        *guard = Some(cancel.clone());
    }

    // 걸어 둔 문서를 먼저 '대기' 로 적어 둔다 — 화면이 바로 그것을 보여 준다
    let db = state.db()?;
    db.with(|c| {
        for id in &document_ids {
            embed_index::set_work_state(c, *id, embed_index::QUEUED, None)?;
        }
        Ok(())
    })?;

    let app2 = app.clone();
    let ids = document_ids.clone();
    let data_dir = state.data_dir.clone();
    let cancel2 = cancel.clone();
    let spec = model.clone();

    // 오래 걸리는 일이라 따로 돌린다. **DB 연결을 새로 연다** — 화면 쪽 명령이
    // 그동안 막히지 않게.
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let db = crate::db::open(&data_dir)?;
        run_all(&app2, &db, &spec, &ids, reindex, cancel2)
    })
    .await
    .map_err(|e| AppError::msg(format!("색인이 끊겼습니다: {e}")))?;

    if let Ok(mut guard) = control.0.lock() {
        *guard = None;
    }
    outcome
}

/// 걸어 둔 문서를 차례로 색인한다
fn run_all(
    app: &tauri::AppHandle,
    db: &crate::db::Db,
    model: &catalog::ModelSpec,
    ids: &[i64],
    reindex: bool,
    cancel: Arc<AtomicBool>,
) -> AppResult<()> {
    for (i, doc_id) in ids.iter().enumerate() {
        let left = (ids.len() - i - 1) as i64;
        if cancel.load(Ordering::Relaxed) {
            db.with(|c| embed_index::set_work_state(c, *doc_id, embed_index::PAUSED, None))?;
            continue;
        }
        if let Err(e) = run_one(app, db, model, *doc_id, reindex, &cancel, left) {
            let msg = format!("{e}");
            db.with(|c| {
                embed_index::set_work_state(c, *doc_id, embed_index::FAILED, Some(&msg))
            })?;
            emit(app, db, model, *doc_id, 0, "failed", Some(msg.clone()), left, 0);
            return Err(e);
        }
    }
    Ok(())
}

fn run_one(
    app: &tauri::AppHandle,
    db: &crate::db::Db,
    model: &catalog::ModelSpec,
    doc_id: i64,
    reindex: bool,
    cancel: &Arc<AtomicBool>,
    left: i64,
) -> AppResult<()> {
    if reindex {
        let dropped = db.with(|c| embed_index::drop_unusable(c, doc_id, model.tag, model.dim))?;
        if dropped > 0 {
            log::info!("문서 {doc_id}: 쓸 수 없는 벡터 {dropped}개를 지우고 다시 만듭니다.");
        }
    }

    let todo = db.with(|c| embed_index::todo(c, doc_id, model.tag, model.dim))?;
    let total = db.with(|c| crate::repo::chunk::count(c, doc_id))?;
    if todo.is_empty() {
        db.with(|c| embed_index::set_work_state(c, doc_id, embed_index::IDLE, None))?;
        emit(app, db, model, doc_id, total, "done", None, left, 0);
        return Ok(());
    }

    db.with(|c| embed_index::set_work_state(c, doc_id, embed_index::RUNNING, None))?;
    let started = Instant::now();
    let mut recent: std::collections::VecDeque<(usize, u128)> = std::collections::VecDeque::new();
    let mut made = 0usize;

    for batch in todo.chunks(BATCH) {
        if cancel.load(Ordering::Relaxed) {
            db.with(|c| embed_index::set_work_state(c, doc_id, embed_index::PAUSED, None))?;
            let done = total - (todo.len() - made) as i64;
            emit(app, db, model, doc_id, done, "paused", None, left, started.elapsed().as_millis() as i64);
            return Ok(());
        }

        let texts: Vec<String> = batch.iter().map(|(_, t, _)| t.clone()).collect();
        let at = Instant::now();
        let vectors = ollama::embed(model.tag, &texts).map_err(AppError::msg)?;

        // 카탈로그에 적어 둔 길이와 다르면, 그대로 담으면 조용히 어긋난다
        if let Some(v) = vectors.first() {
            if v.len() as i64 != model.dim {
                return Err(AppError::msg(format!(
                    "'{}' 이 {}차원 벡터를 돌려주었습니다(설정은 {}차원). \
                     모델이 바뀐 것 같습니다. 프로그램을 업데이트하거나 다른 모델을 골라 주세요.",
                    model.name,
                    v.len(),
                    model.dim
                )));
            }
        }

        db.with(|c| {
            for ((chunk_id, _, hash), v) in batch.iter().zip(vectors.iter()) {
                vector::save(c, *chunk_id, model.tag, hash, v)?;
            }
            Ok(())
        })?;

        made += batch.len();
        recent.push_back((batch.len(), at.elapsed().as_millis()));
        if recent.len() > RECENT {
            recent.pop_front();
        }

        let done = total - (todo.len() - made) as i64;
        let per = if recent.is_empty() {
            model.ms_per_chunk as f64
        } else {
            let (n, ms): (usize, u128) = recent.iter().fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
            ms as f64 / n.max(1) as f64
        };
        let remaining = ((todo.len() - made) as f64 * per) as i64;
        emit_with_eta(
            app, db, model, doc_id, done, "running", None, left,
            started.elapsed().as_millis() as i64, Some(remaining),
        );
    }

    db.with(|c| embed_index::set_work_state(c, doc_id, embed_index::IDLE, None))?;
    emit(app, db, model, doc_id, total, "done", None, left, started.elapsed().as_millis() as i64);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit(
    app: &tauri::AppHandle,
    db: &crate::db::Db,
    model: &catalog::ModelSpec,
    doc_id: i64,
    done: i64,
    state: &'static str,
    error: Option<String>,
    queued: i64,
    elapsed_ms: i64,
) {
    emit_with_eta(app, db, model, doc_id, done, state, error, queued, elapsed_ms, None)
}

#[allow(clippy::too_many_arguments)]
fn emit_with_eta(
    app: &tauri::AppHandle,
    db: &crate::db::Db,
    model: &catalog::ModelSpec,
    doc_id: i64,
    done: i64,
    state: &'static str,
    error: Option<String>,
    queued: i64,
    elapsed_ms: i64,
    remaining_ms: Option<i64>,
) {
    let (title, total) = db
        .with(|c| {
            let t: String = c.query_row(
                "SELECT title FROM document WHERE id = ?1",
                [doc_id],
                |r| r.get(0),
            )?;
            let n = crate::repo::chunk::count(c, doc_id)?;
            Ok((t, n))
        })
        .unwrap_or_else(|_| (String::new(), 0));

    let percent = if total > 0 {
        ((done.max(0) as f64 / total as f64) * 100.0).round() as u8
    } else {
        0
    };

    let _ = app.emit(
        INDEX_EVENT,
        IndexEvent {
            document_id: doc_id,
            title,
            model_name: model.name.to_string(),
            model_tag: model.tag.to_string(),
            total,
            done: done.max(0),
            percent: percent.min(100),
            elapsed_ms,
            remaining_ms,
            state,
            error,
            queued,
        },
    );
}

/// 색인을 멈춘다. **만들어 둔 벡터는 그대로 남는다.**
#[tauri::command]
pub fn index_stop(control: State<'_, IndexControl>) -> AppResult<bool> {
    let guard = control
        .0
        .lock()
        .map_err(|_| AppError::msg("색인 상태를 확인하지 못했습니다."))?;
    match guard.as_ref() {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            Ok(true)
        }
        None => Ok(false),
    }
}

/// 지금 모델로 쓸 수 없는 벡터를 지운다 — "낱말 검색만 쓰겠다" 를 고른 경우다.
///
/// 다시 색인하지 않겠다는 뜻이므로, 자리를 차지하고 있을 까닭이 없다.
/// (모델을 되돌릴 생각이면 [기존 색인 유지] 를 골라 그대로 두면 된다)
#[tauri::command]
pub fn index_drop_unusable(
    state: State<'_, AppState>,
    document_ids: Vec<i64>,
) -> AppResult<usize> {
    let model = current_model(&state)?;
    state.db()?.with(|c| {
        let mut n = 0;
        for id in &document_ids {
            n += embed_index::drop_unusable(c, *id, model.tag, model.dim)?;
            embed_index::set_work_state(c, *id, embed_index::IDLE, None)?;
        }
        Ok(n)
    })
}
