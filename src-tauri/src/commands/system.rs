use crate::error::AppResult;
use crate::state::AppState;
use serde::Serialize;
use tauri::State;

/// 앱이 자기 자신에 대해 아는 것. 설정 화면과 오류 안내에 쓴다.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// 화면에 보이는 이름 (한글)
    pub display_name: String,
    pub version: String,
    /// 자료가 실제로 저장되는 폴더
    pub data_dir: String,
    /// 자료를 정상적으로 열었는가
    pub storage_ready: bool,
    /// 못 열었으면 그 이유 (사용자에게 그대로 보여 준다)
    pub storage_error: Option<String>,
}

#[tauri::command]
pub fn app_info(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<AppInfo> {
    Ok(AppInfo {
        display_name: "업무자료 AI 도우미".into(),
        version: app.package_info().version.to_string(),
        data_dir: state.data_dir.to_string_lossy().into_owned(),
        storage_ready: state.is_ready(),
        storage_error: state.open_error().map(str::to_owned),
    })
}
