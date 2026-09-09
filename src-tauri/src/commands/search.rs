use crate::error::AppResult;
use crate::repo::search as repo;
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
