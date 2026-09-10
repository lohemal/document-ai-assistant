use crate::error::{AppError, AppResult};
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
    /// 그 안의 자리들 — 사용자가 백업할 때 무엇을 챙겨야 하는지 그대로 보여 준다
    pub db_path: String,
    pub files_dir: String,
    pub backups_dir: String,
    /// AI 모델이 저장되는 곳 (Ollama 의 폴더). 앱 자료와 **다른 곳**이다.
    pub models_dir: Option<String>,
    /// 자료를 정상적으로 열었는가
    pub storage_ready: bool,
    /// 못 열었으면 그 이유 (사용자에게 그대로 보여 준다)
    pub storage_error: Option<String>,
}

#[tauri::command]
pub fn app_info(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<AppInfo> {
    let dir = &state.data_dir;
    Ok(AppInfo {
        display_name: "업무자료 AI 도우미".into(),
        version: app.package_info().version.to_string(),
        data_dir: dir.to_string_lossy().into_owned(),
        db_path: crate::db::db_path(dir).to_string_lossy().into_owned(),
        files_dir: crate::db::files_dir(dir).to_string_lossy().into_owned(),
        backups_dir: dir.join("backups").to_string_lossy().into_owned(),
        models_dir: crate::ai::system::ollama_models_dir().map(|p| p.to_string_lossy().into_owned()),
        storage_ready: state.is_ready(),
        storage_error: state.open_error().map(str::to_owned),
    })
}

/// 자료 폴더를 탐색기로 연다. 사용자가 백업하거나 옮길 때 어디인지 눈으로 보게.
#[tauri::command]
pub fn app_open_data_dir(state: State<'_, AppState>) -> AppResult<()> {
    let dir = &state.data_dir;
    std::fs::create_dir_all(dir)?;
    std::process::Command::new("explorer")
        .arg(dir)
        .spawn()
        .map_err(|e| AppError::msg(format!("탐색기를 열지 못했습니다: {e}")))?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupOut {
    pub path: String,
    pub bytes: u64,
}

/// 지금 자료(data.db)를 백업 폴더에 한 파일로 복사한다.
///
/// **등록한 PDF 사본(`files/`)은 여기 들어가지 않는다** — 크기가 크고, 이미 파일이라
/// 그대로 복사하면 된다. 화면과 README 가 "data.db 백업 + files 폴더 복사" 두 가지를
/// 함께 말한다. 복원 화면은 v0.2 — 지금은 파일을 제자리에 되돌려 놓는 손 복원이다.
#[tauri::command]
pub fn app_backup_now(state: State<'_, AppState>) -> AppResult<BackupOut> {
    let db = state.db()?;
    let dir = state.data_dir.clone();
    let path = db.with(|c| crate::db::backup_named(c, &dir, "manual"))?;
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(BackupOut {
        path: path.to_string_lossy().into_owned(),
        bytes,
    })
}
