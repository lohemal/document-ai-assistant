use crate::ai::{self, catalog, ollama};
use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, State};

/// 받는 중인 일을 멈출 수 있게 들고 있는 것
#[derive(Default)]
pub struct PullControl(pub Mutex<Option<Arc<AtomicBool>>>);

/// 진행률을 이 이름으로 화면에 보낸다
pub const PULL_EVENT: &str = "ai://pull";

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PullEvent {
    pub model_id: String,
    pub tag: String,
    pub step: String,
    pub status: String,
    pub completed_bytes: u64,
    pub total_bytes: u64,
    pub percent: Option<u8>,
    pub done: bool,
}

#[tauri::command]
pub fn ai_status() -> ai::AiStatus {
    ai::status()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallHelp {
    /// 공식 내려받기 쪽
    pub download_url: &'static str,
    /// winget 이 있을 때 보여 줄 명령
    pub winget_command: &'static str,
    pub has_winget: bool,
    /// 앱이 왜 대신 깔아 주지 않는지
    pub why_manual: &'static str,
    /// 폐쇄망 학교를 위한 안내
    pub offline_note: &'static str,
}

#[tauri::command]
pub fn ai_install_help() -> InstallHelp {
    InstallHelp {
        download_url: "https://ollama.com/download/windows",
        winget_command: "winget install Ollama.Ollama",
        has_winget: ai::system::has_winget(),
        why_manual: "설치 프로그램을 이 앱이 대신 실행하지는 않습니다. 관리자 권한이 필요할 수 있고, \
                     학교 PC 정책으로 막혀 있을 수도 있어서, 무엇이 막혔는지 화면에서 바로 알 수 있게 \
                     사용자가 직접 설치하도록 했습니다.",
        offline_note: "인터넷이 막힌 PC 라면, 다른 PC 에서 모델을 받은 뒤 사용자 폴더의 \
                       .ollama\\models 를 통째로 옮겨도 됩니다.",
    }
}

/// 모델을 받는다. 진행률은 `ai://pull` 로 나간다.
#[tauri::command]
pub async fn ai_pull_model(
    app: tauri::AppHandle,
    control: State<'_, PullControl>,
    model_id: String,
) -> AppResult<()> {
    let spec = catalog::by_id(&model_id)
        .ok_or_else(|| AppError::msg(format!("모르는 모델입니다: {model_id}")))?;

    // 받기 전에 저장공간을 본다. 다 받고 나서 모자란 것을 아는 것보다 낫다.
    if let Some(f) = ai::check_space(spec.download_gb) {
        return Err(AppError::msg(format!("{} {}", f.message, f.hint)));
    }

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = control
            .0
            .lock()
            .map_err(|_| AppError::msg("받는 중인 일을 확인하지 못했습니다."))?;
        if guard.is_some() {
            return Err(AppError::msg("이미 다른 모델을 받고 있습니다."));
        }
        *guard = Some(cancel.clone());
    }

    let tag = spec.tag.to_string();
    let id = spec.id.to_string();
    let app2 = app.clone();
    let cancel2 = cancel.clone();

    // 받는 일은 오래 걸린다. 다른 화면이 멈추지 않게 따로 돌린다.
    let result = tauri::async_runtime::spawn_blocking(move || {
        ollama::pull(&tag, cancel2, |p| {
            let _ = app2.emit(
                PULL_EVENT,
                PullEvent {
                    model_id: id.clone(),
                    tag: tag.clone(),
                    step: p.step,
                    status: p.status,
                    completed_bytes: p.completed_bytes,
                    total_bytes: p.total_bytes,
                    percent: p.percent,
                    done: p.done,
                },
            );
        })
    })
    .await
    .map_err(|e| AppError::msg(format!("받는 일이 끊겼습니다: {e}")))?;

    if let Ok(mut guard) = control.0.lock() {
        *guard = None;
    }

    result.map_err(|raw| {
        let f = ai::classify_pull_error(&raw);
        AppError::msg(format!("{} {}", f.message, f.hint))
    })
}

#[tauri::command]
pub fn ai_cancel_pull(control: State<'_, PullControl>) -> AppResult<bool> {
    let guard = control
        .0
        .lock()
        .map_err(|_| AppError::msg("받는 중인 일을 확인하지 못했습니다."))?;
    match guard.as_ref() {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            Ok(true)
        }
        None => Ok(false),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTest {
    pub ok: bool,
    /// 무엇을 확인했는지 (사람에게 보여 준다)
    pub detail: String,
}

/// 받은 것만으로 끝내지 않는다. 정말 도는지 짧게 시켜 본다.
///
/// **업무자료는 보내지 않는다.** 모델이 살아 있는지만 본다.
#[tauri::command]
pub async fn ai_test_model(model_id: String) -> AppResult<ModelTest> {
    let spec = catalog::by_id(&model_id)
        .ok_or_else(|| AppError::msg(format!("모르는 모델입니다: {model_id}")))?;
    let tag = spec.tag.to_string();
    let role = spec.role;

    let out = tauri::async_runtime::spawn_blocking(move || match role {
        catalog::Role::Chat => ollama::test_chat(&tag).map(|answer| {
            format!("짧은 물음에 답했습니다: \"{}\"", answer.chars().take(40).collect::<String>())
        }),
        catalog::Role::Embed => {
            ollama::test_embed(&tag).map(|dim| format!("{dim}차원 벡터를 돌려주었습니다."))
        }
    })
    .await
    .map_err(|e| AppError::msg(format!("확인이 끊겼습니다: {e}")))?;

    match out {
        Ok(detail) => Ok(ModelTest { ok: true, detail }),
        Err(e) => Ok(ModelTest {
            ok: false,
            detail: format!("모델이 제대로 돌지 않습니다. ({e})"),
        }),
    }
}
