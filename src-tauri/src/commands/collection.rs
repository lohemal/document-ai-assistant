use crate::db::files_dir;
use crate::error::AppResult;
use crate::repo::collection as repo;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn collection_list(state: State<'_, AppState>) -> AppResult<Vec<repo::Collection>> {
    state.db()?.with(repo::list)
}

#[tauri::command]
pub fn collection_create(state: State<'_, AppState>, name: String) -> AppResult<i64> {
    state.db()?.with(|c| repo::create(c, &name))
}

#[tauri::command]
pub fn collection_rename(state: State<'_, AppState>, id: i64, name: String) -> AppResult<()> {
    state.db()?.with(|c| repo::rename(c, id, &name))
}

#[tauri::command]
pub fn collection_delete(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let removed = state.db()?.with(|c| repo::delete(c, id))?;

    // 자료를 지운 뒤에 파일을 치운다. 순서가 반대면, 자료 삭제가 실패했을 때
    // 파일만 없어져 원문 보기가 깨진다.
    let dir = files_dir(&state.data_dir);
    for doc_id in removed {
        let path = dir.join(format!("{doc_id}.pdf"));
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                // 파일을 못 지워도 자료집 삭제 자체는 이미 끝났다.
                // 사용자에게 실패라고 알릴 일은 아니고, 로그로만 남긴다.
                log::warn!("파일을 지우지 못했습니다 {}: {e}", path.display());
            }
        }
    }
    Ok(())
}
