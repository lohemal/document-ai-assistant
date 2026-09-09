use crate::error::AppResult;
use crate::repo::chunk as repo;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn chunk_save(
    state: State<'_, AppState>,
    document_id: i64,
    chunks: Vec<repo::ChunkIn>,
) -> AppResult<usize> {
    state
        .db()?
        .with_mut(|c| repo::replace_all(c, document_id, &chunks))
}

#[tauri::command]
pub fn chunk_list(state: State<'_, AppState>, document_id: i64) -> AppResult<Vec<repo::Chunk>> {
    state.db()?.with(|c| repo::list(c, document_id))
}

#[tauri::command]
pub fn chunk_get(state: State<'_, AppState>, chunk_id: i64) -> AppResult<repo::Chunk> {
    state.db()?.with(|c| repo::get(c, chunk_id))
}

#[tauri::command]
pub fn chunk_count(state: State<'_, AppState>, document_id: i64) -> AppResult<i64> {
    state.db()?.with(|c| repo::count(c, document_id))
}
