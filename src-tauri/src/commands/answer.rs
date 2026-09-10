//! 물음에 답한다 — **단계를 하나로 뭉치지 않는다.**
//!
//! ```text
//! ① 찾기        commands::search::best_search   (섞기 또는 낱말)
//! ② 근거 고르기  answer::context::build           (상위 5개 + 이웃 ±1)
//! ③ 물음 만들기  answer::prompt                   (규칙 + 근거 + 물음)
//! ④ 답 받기      ai::ollama::chat_stream          (흘려 받기 · 멈출 수 있음)
//! ⑤ 읽기        answer::parse                    (JSON, 너그럽게)
//! ⑥ 인용·숫자   answer::verify                   (인용된 근거만 본다)
//! ⑦ 판단        answer::refuse                   (여러 신호를 함께)
//! ```
//!
//! 단계를 나눈 까닭은 **어디서 잘못됐는지 알 수 있어야** 하기 때문이다. 검색이
//! 못 찾은 것과, 찾았는데 모델이 못 읽은 것과, 읽었는데 지어낸 것은 서로 다른
//! 고장이고 고치는 자리도 다르다. 화면과 평가 시험이 그 셋을 갈라 볼 수 있게
//! 각 단계의 결과를 그대로 담아 돌려준다.
//!
//! **답변 모델이 없어도 ①②까지는 한다** (요구사항 13). AI 때문에 검색이
//! 막히는 일은 없다.

use crate::ai::{self, catalog, ollama};
use crate::answer::{context, focus, parse, prompt, refuse, verify};
use crate::error::{AppError, AppResult};
use crate::repo::chunk;
use crate::repo::search::SearchResult;
use crate::state::AppState;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, State};

/// 답변이 만들어지는 동안 화면에 흘려 주는 이름
pub const ANSWER_EVENT: &str = "ai://answer";

#[derive(Default)]
pub struct AskControl(pub Mutex<Option<Arc<AtomicBool>>>);

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AnswerEvent {
    /// searching | reading | writing | verifying
    pub phase: &'static str,
    /// 사람에게 보여 줄 한 줄
    pub note: String,
    /// 모델이 지금까지 내놓은 글자 수 (형식이 JSON 이라 글은 보여 주지 않는다)
    pub chars: usize,
}

fn tell(app: &tauri::AppHandle, phase: &'static str, note: impl Into<String>, chars: usize) {
    let _ = app.emit(
        ANSWER_EVENT,
        AnswerEvent {
            phase,
            note: note.into(),
            chars,
        },
    );
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMeta {
    pub mode: String,
    pub mode_note: Option<String>,
    pub hits: usize,
    pub elapsed_ms: i64,
    pub terms: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerOut {
    pub question: String,
    /// 이 물음이 남은 작업 기록 번호 (P6). 기록을 못 남겼으면 None — 답은 그대로 돌려준다.
    pub job_id: Option<i64>,
    /// 물음의 초점 낱말이 자료집·근거·인용 청크에 있는가 (P5b). 거부 사유와 경고의 바탕.
    pub focus: Option<focus::FocusCheck>,
    pub decision: refuse::Decision,
    /// 화면에 보여 줄 답. 거부면 정해진 거부 문구가 들어간다.
    pub answer: String,
    pub claims: Vec<parse::Claim>,
    /// LLM 에게 넘긴 근거 (화면에 그대로 보여 준다)
    pub evidence: Vec<context::Evidence>,
    /// 그 가운데 답변이 인용한 것
    pub cited: Vec<String>,
    pub verdict: Option<verify::Verdict>,
    pub judgement: refuse::Judgement,
    /// 해석이 섞였는가
    pub interpretation: bool,
    pub interpretation_notes: Vec<String>,
    /// 모델이 스스로 적은 자신감. **판단에는 쓰지 않는다** —
    /// 작은 모델의 자기 평가는 믿을 수 없다. 개발용으로만 보여 준다.
    pub confidence: Option<String>,
    pub search: SearchMeta,
    /// 쓴 답변 모델 (없으면 None)
    pub model: Option<String>,
    pub model_note: Option<String>,
    pub tokens: u64,
    pub llm_ms: u64,
    pub total_ms: u64,
    pub cancelled: bool,
    /// 모델이 내놓은 글 그대로 — 개발용
    pub raw: Option<String>,
}

/// 답변 모델이 쓸 수 있는가. 못 쓰면 왜 못 쓰는지 한국어로.
fn chat_model(state: &AppState) -> Result<&'static catalog::ModelSpec, String> {
    let _ = state;
    let s = ai::status();
    if s.engine != ai::EngineState::Ready {
        return Err(format!("{} {}", s.detail, s.hint));
    }
    // 받아 둔 답변 모델 가운데 아무 것 (권장하는 것을 먼저 본다)
    let mut ready: Vec<&'static catalog::ModelSpec> = s
        .installed
        .iter()
        .filter_map(|m| catalog::by_tag(&m.tag))
        .filter(|m| m.role == catalog::Role::Chat)
        .collect();
    if ready.is_empty() {
        return Err(
            "답변 모델이 아직 없습니다. [AI 기능 설치] 에서 답변 모델을 받아 주세요. \
             그때까지도 검색과 원문 보기는 그대로 됩니다."
                .to_string(),
        );
    }
    // 메모리에 맞는 큰 쪽을 먼저
    ready.sort_by_key(|m| std::cmp::Reverse(m.min_ram_gb));
    let ram = s.ram_gb.unwrap_or(8);
    Ok(ready
        .iter()
        .find(|m| m.min_ram_gb <= ram)
        .copied()
        .unwrap_or(ready[ready.len() - 1]))
}

#[tauri::command]
pub async fn answer_ask(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    control: State<'_, AskControl>,
    text: String,
    collection_ids: Vec<i64>,
) -> AppResult<AnswerOut> {
    let ids = collection_ids.clone();
    let mut out = ask_inner(app, &state, &control, text, collection_ids).await?;

    // ── ⑧ 기록 (P6) — **어떤 결과든 남긴다.** ────────────────────────
    //
    // 답한 것, 제한적으로 답한 것, 거부한 것, AI 가 없어 근거만 보여 준 것,
    // 사용자가 멈춘 것까지. 멈춘 것도 물음과 그때 찾은 근거는 업무 기록으로
    // 쓸모가 있다 — '답변 생성 중지' 로 담는다. 기록에 실패해도 답은 그대로
    // 돌려준다: 기록은 답을 돕는 것이고, 답을 막는 것이 아니다.
    match state.db().and_then(|db| save_job(db, &out, &ids)) {
        Ok(id) => out.job_id = Some(id),
        Err(e) => log::warn!("작업 기록을 남기지 못했습니다: {e}"),
    }
    Ok(out)
}

/// 답변을 작업 기록으로 담는다 — **당시 근거를 통째로 복사한다.**
///
/// 문서 이름·쪽·원문·형광펜 자리·파일 지문(sha256)·자료집 이름까지 지금 값으로
/// 굳혀 둔다. 나중에 문서가 바뀌거나 지워져도 이 기록은 그대로다 (`repo::job`).
fn save_job(db: &crate::db::Db, out: &AnswerOut, collection_ids: &[i64]) -> AppResult<i64> {
    use crate::repo::{collection, document, job, setting};

    let status = if out.cancelled {
        "cancelled"
    } else {
        match out.decision {
            refuse::Decision::Answer => "answer",
            refuse::Decision::Limited => "limited",
            refuse::Decision::Refuse => "refuse",
            refuse::Decision::NoModel => "no_model",
        }
    };

    db.with_mut(|c| {
        // 자료집 이름은 지금 것을 굳힌다 — 자료집이 지워져도 기록에는 남는다
        let mut colls: Vec<serde_json::Value> = Vec::new();
        for id in collection_ids {
            if let Ok(col) = collection::get(c, *id) {
                colls.push(serde_json::json!({ "id": id, "name": col.name }));
            }
        }
        let embed_model = if out.search.mode == "hybrid" {
            setting::get(c, "embed_model")
                .ok()
                .and_then(|id| catalog::by_id(&id).map(|m| m.tag.to_string()))
        } else {
            None
        };

        let mut evidence: Vec<job::EvidenceIn> = Vec::with_capacity(out.evidence.len());
        for e in &out.evidence {
            let (sha, coll_name) = match document::get(c, e.document_id) {
                Ok(d) => (
                    d.sha256,
                    collection::get(c, d.collection_id).map(|x| x.name).unwrap_or_default(),
                ),
                Err(_) => (String::new(), String::new()),
            };
            evidence.push(job::EvidenceIn {
                source_id: e.source_id.clone(),
                document_id: e.document_id,
                doc_title: e.doc_title.clone(),
                doc_sha256: sha,
                collection_name: coll_name,
                page_start: e.page_start,
                page_end: e.page_end,
                heading_path: e.heading_path.clone(),
                quoted_text: e.text.clone(),
                chunk_id: Some(e.chunk_id),
                spans_json: serde_json::to_string(&e.spans).unwrap_or_else(|_| "[]".into()),
                cited: out.cited.contains(&e.source_id),
            });
        }

        job::save(
            c,
            &job::JobIn {
                kind: "interpret".into(),
                question: out.question.clone(),
                collections_json: serde_json::to_string(&colls).unwrap_or_else(|_| "[]".into()),
                llm_model: out.model.clone(),
                embed_model,
                search_mode: out.search.mode.clone(),
                status: status.into(),
                answer_json: serde_json::to_string(out)
                    .map_err(|e| AppError::msg(format!("답변을 기록으로 옮기지 못했습니다: {e}")))?,
                evidence,
            },
        )
    })
}

async fn ask_inner(
    app: tauri::AppHandle,
    state: &State<'_, AppState>,
    control: &State<'_, AskControl>,
    text: String,
    collection_ids: Vec<i64>,
) -> AppResult<AnswerOut> {
    let began = std::time::Instant::now();
    let db = state.db()?;

    // ── ① 찾기 ─────────────────────────────────────────────────────
    tell(&app, "searching", "자료를 찾고 있습니다…", 0);
    let found: SearchResult =
        super::search::best_search(db, &text, &collection_ids, context::DEFAULT_PLAN.top_k as i64 * 2)
            .await?;

    // ── ② 근거 고르기 ───────────────────────────────────────────────
    let evidence = db.with(|c| context::build(c, &found.hits, context::DEFAULT_PLAN))?;
    let search = SearchMeta {
        mode: found.mode.clone(),
        mode_note: found.mode_note.clone(),
        hits: found.hits.len(),
        elapsed_ms: found.elapsed_ms,
        terms: found.terms.clone(),
    };

    // 답변 모델이 없으면 여기까지. **검색 결과와 근거는 그대로 돌려준다.**
    let model = match chat_model(&state) {
        Ok(m) => m,
        Err(why) => {
            return Ok(AnswerOut {
                question: text,
                job_id: None,
                focus: None,
                decision: refuse::Decision::NoModel,
                answer: String::new(),
                claims: vec![],
                cited: vec![],
                evidence,
                verdict: None,
                judgement: refuse::Judgement {
                    decision: refuse::Decision::NoModel,
                    reasons: vec![why.clone()],
                },
                interpretation: false,
                interpretation_notes: vec![],
                confidence: None,
                search,
                model: None,
                model_note: Some(why),
                tokens: 0,
                llm_ms: 0,
                total_ms: began.elapsed().as_millis() as u64,
                cancelled: false,
                raw: None,
            });
        }
    };

    // 근거가 아예 없으면 모델을 부르지 않는다. 부를 까닭이 없고, 20초가 아깝다.
    if evidence.is_empty() {
        let judgement = refuse::Judgement {
            decision: refuse::Decision::Refuse,
            reasons: vec!["검색에서 관련 있는 자료를 찾지 못했습니다.".into()],
        };
        return Ok(AnswerOut {
            question: text,
            job_id: None,
            focus: None,
            decision: refuse::Decision::Refuse,
            answer: refuse::REFUSAL.to_string(),
            claims: vec![],
            cited: vec![],
            evidence,
            verdict: None,
            judgement,
            interpretation: false,
            interpretation_notes: vec![],
            confidence: None,
            search,
            model: Some(model.name.to_string()),
            model_note: None,
            tokens: 0,
            llm_ms: 0,
            total_ms: began.elapsed().as_millis() as u64,
            cancelled: false,
            raw: None,
        });
    }

    // ── ②′ 초점 낱말 — 모델을 부르기 **전에** 본다 (P5b) ─────────────
    //
    // 물음이 묻는 그 낱말이 자료집 어디에도 없으면(A), 또는 자료집엔 있는데
    // 찾아온 근거에 없으면(B) 모델을 불러도 답이 나올 수 없다 — 있는 근거를
    // 인용해 물음과 다른 이야기를 답으로 내놓을 뿐이다 (P5 에서 실제로 그랬다:
    // "제재" 를 물었는데 "연 2회" 라고 답했다). 여기서 멈추면 CPU 에서 1분
    // 남짓을 아끼고, **근거는 그대로 보여 준다.** 사유는 A·B 가 다르다.
    // 자료집을 훑는 데 문제가 있으면 "있다" 로 보아 거부하지 않는다 — 검사가
    // 고장났다고 답까지 막으면 안 된다.
    let evidence_texts: Vec<String> = evidence.iter().map(|e| e.text.clone()).collect();
    let mut focus_check = focus::FocusCheck::before_answer(
        &text,
        |w| {
            db.with(|c| chunk::any_contains(c, &collection_ids, &focus::needles(w)))
                .unwrap_or(true)
        },
        &evidence_texts,
    );
    if let Some(why) = focus_check.refusal() {
        let judgement = refuse::Judgement {
            decision: refuse::Decision::Refuse,
            reasons: vec![why],
        };
        return Ok(AnswerOut {
            question: text,
            job_id: None,
            focus: Some(focus_check),
            decision: refuse::Decision::Refuse,
            answer: refuse::REFUSAL.to_string(),
            claims: vec![],
            cited: vec![],
            evidence,
            verdict: None,
            judgement,
            interpretation: false,
            interpretation_notes: vec![],
            confidence: None,
            search,
            model: Some(model.name.to_string()),
            model_note: None,
            tokens: 0,
            llm_ms: 0,
            total_ms: began.elapsed().as_millis() as u64,
            cancelled: false,
            raw: None,
        });
    }

    // ── ③ 물음 만들기 ───────────────────────────────────────────────
    let rendered = context::render(&evidence);
    let user_prompt = prompt::user(&text, &rendered);

    // ── ④ 답 받기 ──────────────────────────────────────────────────
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = control
            .0
            .lock()
            .map_err(|_| AppError::msg("답변 상태를 확인하지 못했습니다."))?;
        if guard.is_some() {
            return Err(AppError::msg("이미 답변을 만들고 있습니다."));
        }
        *guard = Some(cancel.clone());
    }

    tell(
        &app,
        "reading",
        format!("근거 {}개를 읽고 있습니다…", evidence.len()),
        0,
    );

    let app2 = app.clone();
    let tag = model.tag.to_string();
    let cancel2 = cancel.clone();

    // **멈추기가 곧바로 들어야 한다.**
    //
    // `chat_stream` 은 토큰이 오는 대로 멈춤을 살피지만, 첫 토큰이 오기까지가
    // 문제다. 이 PC(CPU)에서는 근거 6,000자를 읽는 데만 1분이 걸리고, 그동안은
    // HTTP 응답을 기다리며 막혀 있다. 그 1분 동안 [멈추기] 가 아무 일도 하지
    // 않으면 사용자는 프로그램이 굳은 줄 안다.
    //
    // 그래서 부르는 일은 따로 실 하나에 맡기고, 여기서는 멈춤을 함께 살핀다.
    // 멈추면 **기다리지 않고 곧바로 돌려준다.** 남은 실은 제 일을 마치고
    // 조용히 끝나며, 그 결과는 버린다 — 사용자가 버리라고 했기 때문이다.
    let out = tauri::async_runtime::spawn_blocking(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let watch = cancel2.clone();
        std::thread::spawn(move || {
            let mut chars = 0usize;
            let r = ollama::chat_stream(
                &tag,
                prompt::SYSTEM,
                &user_prompt,
                Some(prompt::schema()),
                cancel2,
                |part| {
                    chars += part.chars().count();
                    // 글자마다 보내면 화면이 바빠지므로 띄엄띄엄 알린다
                    if chars % 40 < part.chars().count() {
                        tell(&app2, "writing", "답을 쓰고 있습니다…", chars);
                    }
                },
            );
            let _ = tx.send(r);
        });

        loop {
            match rx.recv_timeout(std::time::Duration::from_millis(120)) {
                Ok(r) => return r,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if watch.load(Ordering::Relaxed) {
                        return Ok(ollama::ChatOut {
                            text: String::new(),
                            cancelled: true,
                            tokens: 0,
                            elapsed_ms: 0,
                        });
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("답변을 만들던 일이 사라졌습니다.".to_string())
                }
            }
        }
    })
    .await;

    if let Ok(mut guard) = control.0.lock() {
        *guard = None;
    }

    let chat = match out {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => return Err(AppError::msg(e)),
        Err(e) => return Err(AppError::msg(format!("답변이 끊겼습니다: {e}"))),
    };

    // 사용자가 멈췄으면 **검색 결과와 근거는 그대로 두고** 답만 비운다
    if chat.cancelled {
        return Ok(AnswerOut {
            question: text,
            job_id: None,
            focus: None,
            decision: refuse::Decision::NoModel,
            answer: String::new(),
            claims: vec![],
            cited: vec![],
            evidence,
            verdict: None,
            judgement: refuse::Judgement {
                decision: refuse::Decision::NoModel,
                reasons: vec!["답변 만들기를 멈췄습니다. 찾은 근거는 그대로 있습니다.".into()],
            },
            interpretation: false,
            interpretation_notes: vec![],
            confidence: None,
            search,
            model: Some(model.name.to_string()),
            model_note: None,
            tokens: chat.tokens,
            llm_ms: chat.elapsed_ms,
            total_ms: began.elapsed().as_millis() as u64,
            cancelled: true,
            raw: Some(chat.text),
        });
    }

    // ── ⑤ 읽기 · ⑥ 검증 · ⑦ 판단 ──────────────────────────────────
    tell(&app, "verifying", "인용과 숫자를 확인하고 있습니다…", chat.text.chars().count());

    // 형식을 못 읽었어도 **오류로 끝내지 않는다.** 찾은 근거는 사용자에게
    // 값진 것이므로 그대로 보여 주고, 답만 없다고 말한다.
    let draft = match parse::parse(&chat.text) {
        Ok(d) => d,
        Err(why) => {
            log::warn!("모델 답을 읽지 못했습니다: {why}");
            return Ok(AnswerOut {
                question: text,
                job_id: None,
                focus: None,
                decision: refuse::Decision::Refuse,
                answer: String::new(),
                claims: vec![],
                cited: vec![],
                evidence,
                verdict: None,
                judgement: refuse::Judgement {
                    decision: refuse::Decision::Refuse,
                    reasons: vec![why],
                },
                interpretation: false,
                interpretation_notes: vec![],
                confidence: None,
                search,
                model: Some(model.name.to_string()),
                model_note: None,
                tokens: chat.tokens,
                llm_ms: chat.elapsed_ms,
                total_ms: began.elapsed().as_millis() as u64,
                cancelled: false,
                raw: Some(chat.text),
            });
        }
    };
    let verdict = verify::verify(&draft, &evidence);
    let mut judgement = refuse::decide(
        &draft,
        &verdict,
        evidence.len(),
        found.hits.len(),
        model.trusts_insufficient,
    );

    // 초점 낱말 C — 근거엔 있는데 **답이 인용한 청크**에는 없다. 거부하지 않는다
    // (골든 셋에서 맞는 답을 버렸다: `하루` 대 `1일`). 답은 보이되 경고를 단다.
    let cited_texts: Vec<String> = verdict
        .sources
        .iter()
        .filter_map(|s| s.chunk_id)
        .filter_map(|id| evidence.iter().find(|e| e.chunk_id == id))
        .map(|e| e.text.clone())
        .collect();
    focus_check.after_answer(&text, &cited_texts);
    if let Some(warn) = focus_check.warning() {
        if judgement.decision == refuse::Decision::Answer {
            judgement.decision = refuse::Decision::Limited;
        }
        judgement.reasons.push(warn);
    }

    let answer = if judgement.decision == refuse::Decision::Refuse {
        refuse::REFUSAL.to_string()
    } else {
        draft.answer.clone()
    };

    Ok(AnswerOut {
        question: text,
        job_id: None,
        focus: Some(focus_check),
        decision: judgement.decision,
        answer,
        claims: draft.claims.clone(),
        // **검증을 마친 근거 이름**을 넘긴다. 모델이 적은 이름 그대로가 아니다 —
        // 인용을 고쳐 붙인 경우(`verify` 의 ①) 화면의 "답변이 인용" 표가
        // 엉뚱한 근거에 붙어, 검증한 것과 보여 주는 것이 어긋나게 된다.
        // 모델이 원래 무엇을 적었는지는 개발 정보에서 볼 수 있다.
        cited: verdict.sources.iter().map(|s| s.source_id.clone()).collect(),
        evidence,
        interpretation: verdict.has_interpretation,
        interpretation_notes: draft.interpretation_notes.clone(),
        confidence: draft.confidence.clone(),
        verdict: Some(verdict),
        judgement,
        search,
        model: Some(model.name.to_string()),
        model_note: None,
        tokens: chat.tokens,
        llm_ms: chat.elapsed_ms,
        total_ms: began.elapsed().as_millis() as u64,
        cancelled: false,
        raw: Some(chat.text),
    })
}

/// 답변 만들기를 멈춘다. **검색 결과는 그대로 남는다.**
#[tauri::command]
pub fn answer_cancel(control: State<'_, AskControl>) -> AppResult<bool> {
    let guard = control
        .0
        .lock()
        .map_err(|_| AppError::msg("답변 상태를 확인하지 못했습니다."))?;
    match guard.as_ref() {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            Ok(true)
        }
        None => Ok(false),
    }
}
