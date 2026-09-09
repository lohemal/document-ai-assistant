pub mod commands;
pub mod db;
pub mod error;
pub mod repo;
pub mod state;

use error::{AppError, AppResult};
use state::AppState;
use std::path::PathBuf;
use tauri::Manager;

/// 자료 폴더. **설치 폴더가 아니라** `%APPDATA%\<identifier>\` 다.
/// 프로그램을 지웠다 다시 깔아도, 업데이트해도 여기 자료는 그대로 남는다.
pub fn data_dir(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| AppError::msg("자료를 저장할 폴더를 찾지 못했습니다."))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::system::app_info,
            commands::collection::collection_list,
            commands::collection::collection_create,
            commands::collection::collection_rename,
            commands::collection::collection_delete,
            commands::document::document_register,
            commands::document::document_bytes,
            commands::document::document_save_pages,
            commands::document::document_finish,
            commands::document::document_list,
            commands::document::document_get,
            commands::document::document_pages,
            commands::document::document_page,
            commands::document::document_delete,
            commands::chunk::chunk_save,
            commands::chunk::chunk_list,
            commands::chunk::chunk_get,
            commands::chunk::chunk_count,
        ])
        .setup(|app| {
            // 자료를 여는 데 실패해도 앱은 뜬다. 화면에서 이유를 보여 준다.
            let dir = data_dir(&app.handle()).unwrap_or_else(|e| {
                log::error!("자료 폴더를 찾지 못했습니다: {e}");
                PathBuf::from(".")
            });
            app.manage(AppState::new(dir));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("앱을 시작하지 못했습니다");
}
