pub mod ai;
pub mod answer;
pub mod commands;
pub mod domain;
pub mod db;
pub mod error;
#[cfg(test)]
mod eval;
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

/// WebView2 가 스스로 하는 바깥 통신을 줄인다.
///
/// 우리 화면에서 나가는 것은 CSP 로 막았지만(설계안 8-2), **WebView2 런타임은
/// 제 나름의 통신을 한다.** 실제로 재어 보니 `msedgewebview2.exe` 의
/// NetworkService 가 마이크로소프트 주소로 붙어 있었다. 우리 업무자료가 나가는
/// 것은 아니지만, 학교 자료를 다루는 프로그램에서 설명할 수 없는 연결이 남아
/// 있는 것은 좋지 않다.
///
/// 그래서 껍데기가 뜨기 **전에** 배경 통신을 끄는 깃발을 넘긴다.
/// 이걸로 다 막히지는 않는다 — 남는 것은 문서에 적어 둔다.
fn quiet_webview() {
    const FLAGS: &[&str] = &[
        "--disable-background-networking",
        "--disable-component-update",
        "--disable-sync",
        "--disable-domain-reliability",
        "--no-pings",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-breakpad",
        "--disable-features=OptimizationHints,Translate,MediaRouter",
    ];
    // 사용자가 이미 무언가 넘겼다면 뒤에 덧붙인다
    let existing = std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").unwrap_or_default();
    let joined = if existing.trim().is_empty() {
        FLAGS.join(" ")
    } else {
        format!("{existing} {}", FLAGS.join(" "))
    };
    std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", joined);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    quiet_webview();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::system::app_info,
            commands::system::app_open_data_dir,
            commands::system::app_backup_now,
            commands::answer::answer_expect,
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
            commands::search::search_keyword,
            commands::search::search_query,
            commands::search::search_neighbors,
            commands::ai::ai_status,
            commands::ai::ai_install_help,
            commands::ai::ai_pull_model,
            commands::ai::ai_cancel_pull,
            commands::ai::ai_test_model,
            commands::index::index_status,
            commands::index::index_collection_summary,
            commands::index::index_models,
            commands::index::index_set_model,
            commands::index::index_start,
            commands::index::index_stop,
            commands::index::index_drop_unusable,
            commands::answer::answer_ask,
            commands::answer::answer_cancel,
            commands::answer::draft_make,
            commands::job::job_list,
            commands::job::job_get,
            commands::job::job_pin,
            commands::job::job_delete,
            commands::job::job_retention,
            commands::job::job_set_retention,
            commands::job::job_purge,
        ])
        .setup(|app| {
            // 자료를 여는 데 실패해도 앱은 뜬다. 화면에서 이유를 보여 준다.
            let dir = data_dir(&app.handle()).unwrap_or_else(|e| {
                log::error!("자료 폴더를 찾지 못했습니다: {e}");
                PathBuf::from(".")
            });
            app.manage(AppState::new(dir));
            // 모델 받기를 멈출 수 있게 들고 있는다
            app.manage(commands::ai::PullControl::default());
            // 색인을 멈출 수 있게 들고 있는다
            app.manage(commands::index::IndexControl::default());
            // 답변 만들기를 멈출 수 있게 들고 있는다
            app.manage(commands::answer::AskControl::default());
            // 보존기간이 지난 작업 기록은 켠 뒤 몇 초 있다가 따로 지운다 — 시작을 기다리게 하지 않는다
            commands::job::purge_later(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("앱을 시작하지 못했습니다");
}
