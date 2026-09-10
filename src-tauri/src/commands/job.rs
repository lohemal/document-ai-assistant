//! 작업 기록 명령 (P6). 담는 일은 `commands::answer` 가 답을 돌려주기 전에 한다.

use crate::error::AppResult;
use crate::repo::job;
use crate::state::AppState;
use tauri::{Manager, State};

#[tauri::command]
pub fn job_list(state: State<'_, AppState>, pinned_only: bool, limit: Option<i64>) -> AppResult<Vec<job::JobRow>> {
    state.db()?.with(|c| job::list(c, pinned_only, limit.unwrap_or(200)))
}

#[tauri::command]
pub fn job_get(state: State<'_, AppState>, job_id: i64) -> AppResult<job::JobDetail> {
    state.db()?.with(|c| job::get(c, job_id))
}

#[tauri::command]
pub fn job_pin(state: State<'_, AppState>, job_id: i64, pinned: bool) -> AppResult<()> {
    state.db()?.with(|c| job::set_pinned(c, job_id, pinned))
}

#[tauri::command]
pub fn job_delete(state: State<'_, AppState>, job_id: i64) -> AppResult<()> {
    state.db()?.with(|c| job::delete(c, job_id))
}

#[tauri::command]
pub fn job_retention(state: State<'_, AppState>) -> AppResult<i64> {
    state.db()?.with(job::retention_days)
}

#[tauri::command]
pub fn job_set_retention(state: State<'_, AppState>, days: i64) -> AppResult<()> {
    state.db()?.with(|c| job::set_retention_days(c, days))
}

/// 기한이 지난 기록을 지금 지운다. 지운 수를 돌려준다.
#[tauri::command]
pub fn job_purge(state: State<'_, AppState>) -> AppResult<usize> {
    state.db()?.with(|c| {
        let days = job::retention_days(c)?;
        job::purge_expired(c, days, &job::now())
    })
}

/// 앱을 켤 때 부른다 — **기다리게 하지 않는다.** 따로 실 하나에서 잠깐 뒤에 돈다.
///
/// 지우는 일은 `created_at` 색인으로 몇 ms 지만, 자료를 여는 첫 순간에 겹치지
/// 않게 몇 초 미룬다. 실패해도 앱은 그대로 돈다 — 기록이 조금 더 남아 있을 뿐이다.
pub fn purge_later(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(4));
        let state = app.state::<AppState>();
        let Ok(db) = state.db() else { return };
        match db.with(|c| {
            let days = job::retention_days(c)?;
            job::purge_expired(c, days, &job::now())
        }) {
            Ok(0) => {}
            Ok(n) => log::info!("보존기간이 지난 작업 기록 {n}개를 지웠습니다."),
            Err(e) => log::warn!("작업 기록을 정리하지 못했습니다: {e}"),
        }
    });
}
