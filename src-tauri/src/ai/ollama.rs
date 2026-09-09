//! Ollama 와 이야기한다. **오직 이 PC 안(127.0.0.1)하고만.**
//!
//! 업무자료를 다루는 프로그램이므로, 여기서 바깥으로 나갈 길이 있으면 안 된다.
//! 두 겹으로 막았다.
//!
//! 1. `ureq` 를 TLS 없이 넣었다. https 로는 아예 붙을 수 없다.
//! 2. 주소를 만들 때마다 루프백인지 확인한다 (`endpoint`).
//!
//! 모델 파일을 인터넷에서 받아 오는 것은 **Ollama 가 한다.** 우리는 이 PC 의
//! Ollama 에게 "받아 줘" 라고 말하고 진행률을 되받을 뿐이다.

use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const HOST: &str = "127.0.0.1";
pub const PORT: u16 = 11434;

/// 어디에 붙을 것인가.
///
/// `OLLAMA_HOST` 를 바꿔 쓰는 사람이 있어서 그것도 본다. 다만 **이 PC 안이
/// 아닌 주소는 받지 않는다** — 업무자료를 다루는 프로그램이 남의 컴퓨터에
/// 붙는 일은 없어야 한다. 루프백이 아니면 조용히 기본값으로 돌아간다.
fn target() -> (String, u16) {
    let raw = std::env::var("OLLAMA_HOST").unwrap_or_default();
    let raw = raw.trim().trim_start_matches("http://").trim_end_matches('/');
    if raw.is_empty() {
        return (HOST.to_string(), PORT);
    }
    let (host, port) = match raw.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(PORT)),
        None => (raw.to_string(), PORT),
    };
    if is_loopback(&host) {
        (host, port)
    } else {
        log::warn!("OLLAMA_HOST 가 이 PC 밖을 가리켜 무시했습니다: {raw}");
        (HOST.to_string(), PORT)
    }
}

fn is_loopback(host: &str) -> bool {
    host == "127.0.0.1" || host == "localhost" || host == "::1" || host == "[::1]"
}

/// 지금 붙으려는 곳. 화면에 보여 줄 때 쓴다.
pub fn address() -> String {
    let (h, p) = target();
    format!("{h}:{p}")
}

/// 붙는 데 이만큼 넘게 걸리면 뭔가 이상한 것이다
const CONNECT_TIMEOUT: Duration = Duration::from_millis(1200);
/// 물어보고 답을 기다리는 시간
const CALL_TIMEOUT: Duration = Duration::from_secs(8);
/// 답변을 만들어 내는 데는 오래 걸릴 수 있다
const GENERATE_TIMEOUT: Duration = Duration::from_secs(180);

/// 주소를 만든다. **루프백이 아니면 만들지 않는다.**
fn endpoint(path: &str) -> String {
    debug_assert!(path.starts_with('/'));
    let (h, p) = target();
    format!("http://{h}:{p}{path}")
}

/// 127.0.0.1 인지 다시 한 번 본다. 누가 상수를 잘못 고쳐도 막히도록.
fn guard() -> Result<(), Reach> {
    if is_loopback(&target().0) {
        Ok(())
    } else {
        Err(Reach::Blocked)
    }
}

/// 포트가 어떤 상태인가. HTTP 를 걸어 보기 **전에** 먼저 본다.
///
/// 이렇게 나누면 "안 켜짐"(연결 거부)과 "켜졌는데 못 붙음"(시간 초과)을
/// 오류 메시지 글자를 짐작하지 않고 갈라낼 수 있다.
#[derive(Debug, PartialEq)]
pub enum Reach {
    /// 붙었다
    Open,
    /// 아무도 안 듣고 있다 — Ollama 가 안 켜져 있다
    Refused,
    /// 답이 없다 — 방화벽이 막거나 뭔가 붙잡고 있다
    Timeout,
    /// 그 밖의 문제
    Error(String),
    /// 루프백이 아니어서 아예 걸지 않았다
    Blocked,
}

/// 붙어 보고 답을 기다리는 한도.
///
/// Windows 에서 거부당하는 데 2초쯤 걸려서 넉넉히 잡았다. 아래를 보라.
const PROBE_DEADLINE: Duration = Duration::from_millis(3500);

pub fn probe_port() -> Reach {
    if let Err(e) = guard() {
        return e;
    }
    let (h, p) = target();
    let addr: SocketAddr = match format!("{h}:{p}").parse() {
        Ok(a) => a,
        Err(e) => return Reach::Error(e.to_string()),
    };

    // ⚠ `TcpStream::connect_timeout` 을 쓰면 안 된다.
    //
    // 이 PC(Windows 11)에서 재어 보니, **연결이 거부당했는데도** 시간 제한을
    // 다 채운 뒤 `TimedOut` 을 돌려준다. 반대로 그냥 `connect` 는
    // `ConnectionRefused`(10061)를 제대로 준다. 대신 2초쯤 걸린다.
    //
    //   connect_timeout(1.2s) -> TimedOut   (1.21초)
    //   connect()             -> ConnectionRefused (2.03초)
    //
    // 그대로 두면 **"Ollama 가 꺼져 있음" 을 모두 "방화벽이 막고 있음" 으로
    // 잘못 안내**하게 된다. 사용자가 할 일이 완전히 달라진다.
    //
    // 그래서 딴 갈래에서 그냥 `connect` 를 하고, 우리 쪽에서 시간을 잰다.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let outcome = TcpStream::connect(addr).map(|_| ()).map_err(|e| e.kind());
        let _ = tx.send(outcome); // 이미 포기했으면 받는 쪽이 없다
    });

    match rx.recv_timeout(PROBE_DEADLINE) {
        Ok(Ok(())) => Reach::Open,
        Ok(Err(kind)) => match kind {
            std::io::ErrorKind::ConnectionRefused => Reach::Refused,
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => Reach::Timeout,
            other => Reach::Error(format!("{other:?}")),
        },
        Err(_) => Reach::Timeout,
    }
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout(timeout)
        .build()
}

/// ureq 오류를 사람이 읽을 말로. 원문도 남겨 둔다.
fn describe(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            let body = body.trim();
            if body.is_empty() {
                format!("Ollama 가 {code} 를 돌려주었습니다.")
            } else {
                format!("Ollama 가 {code} 를 돌려주었습니다: {body}")
            }
        }
        ureq::Error::Transport(t) => format!("Ollama 에 연결하지 못했습니다: {t}"),
    }
}

/// Ollama 판. 이게 나와야 **정말로 Ollama** 다 (다른 프로그램이 포트를 잡고
/// 있을 수도 있으므로).
pub fn version() -> Result<String, String> {
    guard().map_err(|_| "루프백이 아닌 주소는 쓰지 않습니다.".to_string())?;
    let resp = agent(CALL_TIMEOUT)
        .get(&endpoint("/api/version"))
        .call()
        .map_err(describe)?;
    // 여기서 오는 영어 오류를 그대로 보여 주지 않는다. 사용자가 할 일을 적는다.
    let not_ollama = || {
        format!(
            "{} 를 다른 프로그램이 쓰고 있는 것 같습니다. 그 프로그램을 끄거나, Ollama 를 다른 포트로 옮긴 뒤 [다시 확인] 을 눌러 주세요.",
            address()
        )
    };
    let json: serde_json::Value = resp.into_json().map_err(|e| {
        log::info!("Ollama 판을 읽지 못했습니다: {e}");
        not_ollama()
    })?;
    json.get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(not_ollama)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModel {
    pub tag: String,
    pub size_bytes: u64,
}

/// 이 PC 에 받아 둔 모델들
pub fn installed_models() -> Result<Vec<InstalledModel>, String> {
    guard().map_err(|_| "루프백이 아닌 주소는 쓰지 않습니다.".to_string())?;
    let resp = agent(CALL_TIMEOUT)
        .get(&endpoint("/api/tags"))
        .call()
        .map_err(describe)?;
    let json: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
    let list = json
        .get("models")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(list
        .iter()
        .filter_map(|m| {
            Some(InstalledModel {
                tag: m.get("name")?.as_str()?.to_string(),
                size_bytes: m.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
            })
        })
        .collect())
}

// ── 모델 받기 ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullProgress {
    /// Ollama 가 알려 주는 지금 단계 ("pulling manifest", "verifying …")
    pub status: String,
    /// 사람이 읽을 단계 이름
    pub step: String,
    pub completed_bytes: u64,
    pub total_bytes: u64,
    /// 0~100. 전체 크기를 아직 모르면 None
    pub percent: Option<u8>,
    pub done: bool,
}

/// 모델을 받는다. 진행률이 나올 때마다 `on_progress` 를 부른다.
///
/// 받아 오는 일 자체는 **Ollama 가 한다.** 우리는 이 PC 의 Ollama 에게
/// 부탁하고 진행 상황을 되받을 뿐이다.
pub fn pull(
    tag: &str,
    cancel: Arc<AtomicBool>,
    mut on_progress: impl FnMut(PullProgress),
) -> Result<(), String> {
    guard().map_err(|_| "루프백이 아닌 주소는 쓰지 않습니다.".to_string())?;

    let resp = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        // 받는 데 걸리는 시간은 길다. 읽기 시간 제한을 두지 않는다.
        .build()
        .post(&endpoint("/api/pull"))
        .send_json(ureq::json!({ "model": tag, "stream": true }))
        .map_err(describe)?;

    let reader = BufReader::new(resp.into_reader());

    // 층(layer)마다 크기가 따로 온다. 다 더해야 전체 진행률이 된다.
    let mut totals: Vec<(String, u64, u64)> = Vec::new(); // (digest, total, completed)
    let mut last_error: Option<String> = None;

    for line in reader.lines() {
        if cancel.load(Ordering::Relaxed) {
            return Err("사용자가 멈췄습니다.".into());
        }
        let line = match line {
            Ok(l) => l,
            Err(e) => return Err(format!("받는 도중 끊겼습니다: {e}")),
        };
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            last_error = Some(err.to_string());
            break;
        }

        let status = v
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();

        if let (Some(digest), Some(total)) = (
            v.get("digest").and_then(|d| d.as_str()),
            v.get("total").and_then(|t| t.as_u64()),
        ) {
            let completed = v.get("completed").and_then(|c| c.as_u64()).unwrap_or(0);
            match totals.iter_mut().find(|(d, _, _)| d == digest) {
                Some(entry) => {
                    entry.1 = total;
                    entry.2 = completed;
                }
                None => totals.push((digest.to_string(), total, completed)),
            }
        }

        let total_bytes: u64 = totals.iter().map(|(_, t, _)| *t).sum();
        let completed_bytes: u64 = totals.iter().map(|(_, _, c)| *c).sum();
        let percent = if total_bytes > 0 {
            Some(((completed_bytes as f64 / total_bytes as f64) * 100.0).min(100.0) as u8)
        } else {
            None
        };

        let done = status == "success";
        on_progress(PullProgress {
            step: step_name(&status),
            status,
            completed_bytes,
            total_bytes,
            percent,
            done,
        });

        if done {
            return Ok(());
        }
    }

    match last_error {
        Some(e) => Err(e),
        None => Err("모델을 다 받았다는 신호가 오지 않았습니다.".into()),
    }
}

/// Ollama 가 주는 영어 단계 이름을 사람 말로 바꾼다
fn step_name(status: &str) -> String {
    let s = status.to_lowercase();
    if s.contains("manifest") && s.contains("pull") {
        "목록 확인 중".into()
    } else if s.starts_with("pulling") {
        "내려받는 중".into()
    } else if s.contains("verifying") {
        "확인하는 중".into()
    } else if s.contains("writing") {
        "갈무리하는 중".into()
    } else if s.contains("success") {
        "끝".into()
    } else if s.contains("exist") {
        "이미 있음".into()
    } else {
        status.to_string()
    }
}

// ── 받은 뒤 정말 도는지 ──────────────────────────────────────────────

/// 답변 모델이 실제로 도는지 짧게 물어본다.
///
/// **업무자료는 보내지 않는다.** 모델이 살아 있는지만 본다.
pub fn test_chat(tag: &str) -> Result<String, String> {
    guard().map_err(|_| "루프백이 아닌 주소는 쓰지 않습니다.".to_string())?;
    let resp = agent(GENERATE_TIMEOUT)
        .post(&endpoint("/api/generate"))
        .send_json(ureq::json!({
            "model": tag,
            "prompt": "1 + 1 = ? 숫자만 답하세요.",
            "stream": false,
            "options": { "temperature": 0, "num_predict": 8 }
        }))
        .map_err(describe)?;
    let json: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
    let text = json
        .get("response")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if text.is_empty() {
        Err("모델이 아무 답도 하지 않았습니다.".into())
    } else {
        Ok(text)
    }
}

/// 검색용 모델이 실제로 도는지 본다. 벡터 길이를 돌려준다.
pub fn test_embed(tag: &str) -> Result<usize, String> {
    guard().map_err(|_| "루프백이 아닌 주소는 쓰지 않습니다.".to_string())?;
    let resp = agent(GENERATE_TIMEOUT)
        .post(&endpoint("/api/embed"))
        .send_json(ureq::json!({ "model": tag, "input": "확인" }))
        .map_err(describe)?;
    let json: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
    let dim = json
        .get("embeddings")
        .and_then(|e| e.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_array())
        .map(|v| v.len())
        .unwrap_or(0);
    if dim == 0 {
        Err("모델이 빈 결과를 돌려주었습니다.".into())
    } else {
        Ok(dim)
    }
}

#[cfg(test)]
#[path = "ollama_tests.rs"]
mod server_tests;
