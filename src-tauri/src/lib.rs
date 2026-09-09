pub mod error;

use error::{AppError, AppResult};
use serde::Serialize;
use std::path::PathBuf;
use tauri::Manager;

/// 앱이 자기 자신에 대해 아는 것. 설정 화면과 "자료 폴더 열기"에 쓴다.
#[derive(Debug, Serialize)]
pub struct AppInfo {
    /// 화면에 보이는 이름 (한글)
    pub display_name: String,
    pub version: String,
    /// 자료가 실제로 저장되는 폴더
    pub data_dir: String,
}

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

#[tauri::command]
fn app_info(app: tauri::AppHandle) -> AppResult<AppInfo> {
    Ok(AppInfo {
        display_name: "업무자료 AI 도우미".into(),
        version: app.package_info().version.to_string(),
        data_dir: data_dir(&app)?.to_string_lossy().into_owned(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![app_info])
        .setup(|app| {
            // 자료 폴더는 앱이 뜨는 즉시 만들어 둔다.
            // 여기서 실패해도 앱은 뜬다 — 화면에서 오류를 보여 주는 편이 낫다.
            if let Err(e) = data_dir(&app.handle()) {
                log::error!("자료 폴더를 만들지 못했습니다: {e}");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("앱을 시작하지 못했습니다");
}
