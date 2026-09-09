use crate::ai::{catalog, ollama};
use crate::error::AppResult;
use crate::repo::hybrid;
use crate::repo::search as repo;
use crate::repo::setting;
use crate::state::AppState;
use tauri::State;

/// 낱말로 찾기. AI 모델이 없어도 된다.
#[tauri::command]
pub fn search_keyword(
    state: State<'_, AppState>,
    text: String,
    collection_ids: Vec<i64>,
    limit: Option<i64>,
) -> AppResult<repo::SearchResult> {
    let req = repo::Request {
        text,
        collection_ids,
        limit: limit.unwrap_or(20),
    };
    state.db()?.with(|c| repo::keyword_search(c, &req))
}

/// 찾기. **사용자는 방식을 고르지 않는다.**
///
/// 쓸 수 있는 것을 쓴다:
///   의미 색인이 있고 AI 가 돌면  → 섞어 찾기(Hybrid)
///   그 밖의 모든 경우            → 낱말로 찾기
///
/// 내려앉는 자리마다 **왜 그렇게 되었는지**를 `modeNote` 에 담는다. 검색 품질이
/// 갑자기 달라졌을 때 사용자가 까닭을 알 수 있어야 한다.
#[tauri::command]
pub async fn search_query(
    state: State<'_, AppState>,
    text: String,
    collection_ids: Vec<i64>,
    limit: Option<i64>,
) -> AppResult<repo::SearchResult> {
    let limit = limit.unwrap_or(20);
    let db = state.db()?;

    let keyword_only = |note: Option<String>| -> AppResult<repo::SearchResult> {
        let mut r = db.with(|c| {
            repo::keyword_search(
                c,
                &repo::Request {
                    text: text.clone(),
                    collection_ids: collection_ids.clone(),
                    limit,
                },
            )
        })?;
        r.mode_note = note;
        Ok(r)
    };

    // ① 어느 모델로 색인해 두었나
    let model_id = db.with(|c| setting::get(c, setting::EMBED_MODEL))?;
    let Some(model) = catalog::by_id(&model_id).filter(|m| m.role == catalog::Role::Embed) else {
        return keyword_only(Some(
            "설정에 담긴 검색 모델을 몰라서 낱말로만 찾았습니다.".into(),
        ));
    };

    // ② 쓸 수 있는 벡터가 이 범위에 하나라도 있나.
    //    없으면 물음을 벡터로 만들 필요조차 없다 (0.8초를 아낀다).
    let usable = db.with(|c| {
        crate::repo::embed_index::status(c, model.tag, model.dim, &collection_ids)
    })?;
    let ready: i64 = usable.iter().map(|d| d.done).sum();
    if ready == 0 {
        return keyword_only(Some(
            "의미 검색 색인이 아직 없어 낱말로만 찾았습니다.".into(),
        ));
    }

    // ③ 물음을 벡터로. AI 가 꺼져 있으면 여기서 걸린다 — 그때도 검색은 된다.
    let tag = model.tag.to_string();
    let q = text.clone();
    let made = tauri::async_runtime::spawn_blocking(move || ollama::embed(&tag, &[q]))
        .await
        .map_err(|e| format!("물음을 벡터로 만들지 못했습니다: {e}"));

    let vector = match made {
        Ok(Ok(mut v)) if !v.is_empty() => v.remove(0),
        Ok(Err(e)) | Err(e) => {
            log::warn!("의미 검색을 쓰지 못했습니다: {e}");
            return keyword_only(Some(
                "AI 실행환경에 붙지 못해 낱말로만 찾았습니다. AI 를 켜면 섞어 찾기를 씁니다.".into(),
            ));
        }
        Ok(Ok(_)) => {
            return keyword_only(Some("물음을 벡터로 만들지 못해 낱말로만 찾았습니다.".into()))
        }
    };

    // ④ 섞어 찾기. 색인 안 된 문서도 낱말 쪽으로 계속 걸린다.
    let mut result = db.with(|c| {
        hybrid::hybrid_search(
            c,
            &hybrid::Hybrid {
                text: &text,
                query_vec: &vector,
                model: model.tag,
                collection_ids: collection_ids.clone(),
                limit,
                depth: hybrid::DEFAULT_DEPTH,
            },
        )
    })?;

    // 자료집 안에 색인이 덜 된 문서가 있으면 그대로 말해 준다
    let total_docs = usable.len();
    let ready_docs = usable
        .iter()
        .filter(|d| d.state.semantic_usable())
        .count();
    if ready_docs < total_docs {
        result.mode_note = Some(format!(
            "문서 {total_docs}개 중 {ready_docs}개만 의미 검색 준비됐습니다. \
             나머지는 낱말로 찾았습니다."
        ));
    }
    Ok(result)
}

/// 고른 청크의 이웃. 검색 순위와는 상관이 없다 (P5 에서 쓴다).
#[tauri::command]
pub fn search_neighbors(
    state: State<'_, AppState>,
    chunk_id: i64,
    radius: Option<i64>,
) -> AppResult<Vec<i64>> {
    state
        .db()?
        .with(|c| repo::neighbors(c, chunk_id, radius.unwrap_or(1)))
}
