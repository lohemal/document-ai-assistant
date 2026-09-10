//! 로컬 AI 실행환경 — 있는지 보고, 없으면 무엇을 하면 되는지 알려 준다.
//!
//! **이 뭉치가 없거나 말썽이어도 앱은 그대로 돌아야 한다** (설계안 2-12).
//! 그래서 여기서는 오류를 던져 올리는 대신, "지금 어떤 상태인지" 를 담아
//! 돌려준다. 화면이 그 상태를 보고 무엇을 보여 줄지 정한다.

pub mod catalog;
pub mod ollama;
pub mod system;

use catalog::ModelSpec;
use serde::Serialize;

/// 화면에 보여 주는 이름. 사용자는 Ollama 를 몰라도 된다.
pub const ENGINE_NAME: &str = "Ollama";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineState {
    /// 이 PC 에 깔려 있지 않다
    NotInstalled,
    /// 깔려 있는데 켜져 있지 않다
    NotRunning,
    /// 켜져 있는 것 같은데 앱에서 붙지 못한다 (방화벽 등)
    Unreachable,
    /// 11434 포트에 Ollama 가 아닌 다른 것이 있다
    NotOllama,
    /// 쓸 수 있다
    Ready,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub engine: EngineState,
    /// "Ollama" — 설정·상세에서만 보여 준다
    pub engine_name: &'static str,
    pub engine_version: Option<String>,
    /// 지금 어떤 상태인지 (사용자에게 그대로 보여 준다)
    pub detail: String,
    /// 무엇을 하면 되는지
    pub hint: String,

    /// 실행 파일을 찾은 자리 (있으면)
    pub binary_path: Option<String>,
    pub has_winget: bool,

    pub ram_gb: Option<u64>,
    pub free_gb: Option<f64>,

    pub installed: Vec<ollama::InstalledModel>,
    /// 쓸 수 있는 답변 모델의 태그 (받아 둔 것 중에서)
    pub chat_ready: Option<String>,
    pub embed_ready: Option<String>,

    pub recommended_chat: &'static ModelSpec,
    pub recommended_embed: &'static ModelSpec,
    pub recommend_reason: String,
    pub models: &'static [ModelSpec],

    /// AI 없이도 되는 일이 있다는 것을 화면이 늘 말할 수 있게 한다
    pub works_without_ai: [&'static str; 6],
}

const WITHOUT_AI: [&str; 6] = [
    "자료집 관리",
    "PDF 등록",
    "글자 뽑기",
    "원문 보기",
    "낱말로 찾기",
    "찾은 자리 형광펜",
];

/// 지금 상태를 알아본다. **절대 오류를 던지지 않는다.**
pub fn status() -> AiStatus {
    let ram_gb = system::ram_gb();
    let (rec_chat, rec_embed) = catalog::recommended(ram_gb);
    let free_gb = system::ollama_models_dir()
        .and_then(|d| system::free_bytes(&d))
        .map(|b| (b as f64) / 1024.0 / 1024.0 / 1024.0);
    let binary = system::ollama_binary();

    let mut s = AiStatus {
        engine: EngineState::NotInstalled,
        engine_name: ENGINE_NAME,
        engine_version: None,
        detail: String::new(),
        hint: String::new(),
        binary_path: binary.as_ref().map(|p| p.display().to_string()),
        has_winget: system::has_winget(),
        ram_gb,
        free_gb,
        installed: vec![],
        chat_ready: None,
        embed_ready: None,
        recommended_chat: rec_chat,
        recommended_embed: rec_embed,
        recommend_reason: catalog::reason(ram_gb),
        models: catalog::MODELS,
        works_without_ai: WITHOUT_AI,
    };

    // 프로세스 이름만 보지 않는다. 실제로 붙어 보고, 정말 Ollama 인지까지 본다.
    match ollama::probe_port() {
        ollama::Reach::Refused => {
            if binary.is_some() {
                s.engine = EngineState::NotRunning;
                s.detail = "AI 실행환경이 이 PC 에 있지만 켜져 있지 않습니다.".into();
                s.hint = "시작 메뉴에서 Ollama 를 실행한 뒤 [다시 확인] 을 눌러 주세요.".into();
            } else {
                s.engine = EngineState::NotInstalled;
                s.detail = "AI 실행환경이 이 PC 에 없습니다.".into();
                s.hint = "AI 답변 기능을 쓰려면 먼저 실행환경을 설치해야 합니다.".into();
            }
            return s;
        }
        ollama::Reach::Timeout => {
            s.engine = EngineState::Unreachable;
            s.detail = "AI 실행환경이 켜져 있는 것 같은데 프로그램이 붙지 못했습니다.".into();
            s.hint = format!(
                "방화벽이나 보안 프로그램이 {} 연결을 막고 있을 수 있습니다.",
                ollama::address()
            );
            return s;
        }
        ollama::Reach::Blocked => {
            s.engine = EngineState::Unreachable;
            s.detail = "이 PC 안이 아닌 주소로는 연결하지 않습니다.".into();
            s.hint = "설정이 잘못되었습니다. 개발자에게 알려 주세요.".into();
            return s;
        }
        ollama::Reach::Error(e) => {
            s.engine = EngineState::Unreachable;
            s.detail = "AI 실행환경에 붙는 중 문제가 생겼습니다.".into();
            s.hint = e;
            return s;
        }
        ollama::Reach::Open => {}
    }

    // 포트는 열려 있다. 정말 Ollama 인가?
    match ollama::version() {
        Ok(v) => {
            s.engine = EngineState::Ready;
            s.engine_version = Some(v);
        }
        Err(e) => {
            s.engine = EngineState::NotOllama;
            s.detail = format!("{} 포트가 열려 있지만 AI 실행환경이 아닙니다.", ollama::address());
            s.hint = e;
            return s;
        }
    }

    // 어떤 모델을 받아 두었는가
    match ollama::installed_models() {
        Ok(list) => {
            for m in &list {
                if let Some(spec) = catalog::by_tag(&m.tag) {
                    if spec.role == catalog::Role::Embed && s.embed_ready.is_none() {
                        s.embed_ready = Some(m.tag.clone())
                    }
                }
            }
            // 답변 모델은 답할 때와 같은 규칙으로 고른다 (catalog::pick_chat)
            s.chat_ready = catalog::pick_chat(list.iter().map(|m| m.tag.as_str()), s.ram_gb)
                .and_then(|spec| list.iter().find(|m| catalog::by_tag(&m.tag).map(|x| x.id) == Some(spec.id)))
                .map(|m| m.tag.clone());
            s.installed = list;
        }
        Err(e) => {
            s.detail = "AI 실행환경은 켜져 있는데 모델 목록을 읽지 못했습니다.".into();
            s.hint = e;
            return s;
        }
    }

    s.detail = match (&s.chat_ready, &s.embed_ready) {
        (Some(_), Some(_)) => "AI 를 쓸 수 있습니다.".into(),
        (Some(_), None) => "답변 모델은 있고 검색 모델이 없습니다.".into(),
        (None, Some(_)) => "검색 모델은 있고 답변 모델이 없습니다.".into(),
        (None, None) => "AI 실행환경은 준비됐지만 모델이 아직 없습니다.".into(),
    };
    s.hint = if s.chat_ready.is_some() && s.embed_ready.is_some() {
        String::new()
    } else {
        "필요한 모델을 받으면 AI 기능을 쓸 수 있습니다.".into()
    };

    s
}

// ── 어디서 잘못됐는가 ────────────────────────────────────────────────

/// 실패를 한 덩어리로 뭉뚱그리지 않는다. 사용자가 할 일이 저마다 다르다.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    /// 프로그램이 가르는 이름
    pub code: &'static str,
    /// 사람에게 보여 줄 말
    pub message: String,
    /// 무엇을 하면 되는지
    pub hint: String,
}

impl Failure {
    fn new(code: &'static str, message: impl Into<String>, hint: impl Into<String>) -> Self {
        Failure {
            code,
            message: message.into(),
            hint: hint.into(),
        }
    }
}

/// 모델 받기가 왜 실패했는지 가른다.
///
/// Ollama 는 오류를 영어 한 줄로 준다. 그걸 그대로 보여 주면 사용자는 무엇을
/// 해야 할지 모른다. **"AI 설치 실패" 하나로 뭉치지도 않는다.**
pub fn classify_pull_error(raw: &str) -> Failure {
    let low = raw.to_lowercase();

    if low.contains("멈췄") || low.contains("cancel") {
        return Failure::new("cancelled", "모델 받기를 멈췄습니다.", "다시 받으려면 [모델 받기] 를 눌러 주세요.");
    }
    if low.contains("no space") || low.contains("not enough space") || low.contains("disk full") {
        return Failure::new(
            "disk_space",
            "저장공간이 모자랍니다.",
            "필요 없는 파일을 정리한 뒤 다시 받아 주세요. 모델은 사용자 폴더의 .ollama 안에 들어갑니다.",
        );
    }
    if low.contains("lookup")
        || low.contains("no such host")
        || low.contains("dial tcp")
        || low.contains("connection refused")
        || low.contains("network")
        || low.contains("i/o timeout")
        || low.contains("tls")
        || low.contains("certificate")
    {
        return Failure::new(
            "network",
            "인터넷에 연결하지 못했습니다.",
            "모델은 처음 한 번 인터넷에서 받아야 합니다. 학교 네트워크가 막고 있다면, 다른 PC 에서 받아 모델 폴더를 옮기는 방법도 있습니다.",
        );
    }
    if low.contains("not found") || low.contains("manifest") && low.contains("404") {
        return Failure::new(
            "model_not_found",
            "그 모델을 찾지 못했습니다.",
            "모델 이름이 바뀌었을 수 있습니다. 다른 모델을 골라 보세요.",
        );
    }
    if low.contains("connection refused") || low.contains("연결하지 못했습니다") {
        return Failure::new(
            "engine_down",
            "AI 실행환경이 꺼졌습니다.",
            "Ollama 를 다시 실행한 뒤 [다시 확인] 을 눌러 주세요.",
        );
    }

    Failure::new("pull_failed", format!("모델을 받지 못했습니다. ({raw})"), "잠시 뒤에 다시 해 보세요.")
}

/// 받기 전에 저장공간을 본다. 다 받고 나서 모자란 것을 아는 것보다 낫다.
pub fn check_space(needed_gb: f64) -> Option<Failure> {
    let dir = system::ollama_models_dir()?;
    let free = system::free_bytes(&dir)?;
    let free_gb = (free as f64) / 1024.0 / 1024.0 / 1024.0;
    // 받는 도중에는 임시 파일이 함께 있으므로 넉넉히 본다
    let want = needed_gb * 1.3;
    if free_gb < want {
        Some(Failure::new(
            "disk_space",
            format!("저장공간이 모자랍니다. {free_gb:.1}GB 남았는데 약 {want:.1}GB 가 필요합니다."),
            "필요 없는 파일을 정리한 뒤 다시 해 주세요.",
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 상태를_묻는_것만으로는_죽지_않는다() {
        // Ollama 가 있든 없든, 이 함수는 늘 답을 준다.
        // 여기서 죽으면 앱 시작이 통째로 막힌다.
        let s = status();
        assert_eq!(s.engine_name, "Ollama");
        assert!(!s.detail.is_empty() || s.engine == EngineState::Ready);
        assert_eq!(s.models.len(), catalog::MODELS.len());
        assert_eq!(s.works_without_ai.len(), 6);
    }

    #[test]
    fn 실패를_저마다_다르게_가른다() {
        assert_eq!(classify_pull_error("no space left on device").code, "disk_space");
        assert_eq!(
            classify_pull_error("Get \"https://registry.ollama.ai\": dial tcp: lookup registry.ollama.ai: no such host").code,
            "network"
        );
        assert_eq!(classify_pull_error("사용자가 멈췄습니다.").code, "cancelled");
        assert_eq!(classify_pull_error("model not found").code, "model_not_found");
        assert_eq!(classify_pull_error("뭔가 이상한 일").code, "pull_failed");
    }

    #[test]
    fn 실패마다_무엇을_하면_되는지_알려_준다() {
        for raw in [
            "no space left on device",
            "dial tcp: lookup registry.ollama.ai",
            "model not found",
            "알 수 없는 오류",
        ] {
            let f = classify_pull_error(raw);
            assert!(!f.message.is_empty(), "{raw}");
            assert!(!f.hint.is_empty(), "{raw} 에 할 일이 없습니다");
        }
    }

    #[test]
    fn 터무니없는_크기를_달라면_저장공간_모자람이_나온다() {
        // 이 PC 에 100TB 가 남아 있을 리 없다
        if system::ollama_models_dir().is_some() {
            let f = check_space(100_000.0);
            assert!(f.is_some());
            assert_eq!(f.unwrap().code, "disk_space");
        }
    }
}
