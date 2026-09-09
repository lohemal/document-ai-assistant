//! Ollama 클라이언트를 **가짜 서버**로 시험한다.
//!
//! 진짜 Ollama 를 깔지 않고도 확인할 수 있어야 하는 것들이 있다 —
//! "포트에 다른 프로그램이 있는 경우", "받다가 저장공간이 모자란 경우" 같은
//! 것은 진짜로 만들려면 디스크를 채워야 한다.
//!
//! 서버는 Rust 로 짠다. 시험이 밖의 것에 기대지 않게.

use super::*;
use crate::ai::catalog;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Mutex, MutexGuard};

/// `OLLAMA_HOST` 는 프로세스 하나에 하나뿐이라, 이 시험들은 줄 서서 돈다.
static LOCK: Mutex<()> = Mutex::new(());

fn serialized() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// 요청을 헤더와 본문까지 다 읽는다.
fn read_request(stream: &mut std::net::TcpStream) -> String {
    stream
        .set_read_timeout(Some(std::time::Duration::from_millis(400)))
        .ok();
    let mut data: Vec<u8> = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&buf[..n]);
                if let Some(end) = find(&data, b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&data[..end]).to_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if data.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            Err(_) => break, // 시간이 다 됐다 — 더 올 것이 없다
        }
    }
    String::from_utf8_lossy(&data).to_string()
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// 정해진 답만 돌려주는 가짜 서버를 띄우고, 그리로 `OLLAMA_HOST` 를 돌린다.
///
/// `reply` 는 요청 첫 줄(예: "GET /api/version HTTP/1.1")을 받아 **본문**을
/// 돌려준다. None 이면 404.
fn with_server<T>(
    reply: impl Fn(&str) -> Option<String> + Send + 'static,
    body: impl FnOnce() -> T,
) -> T {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let handle = std::thread::spawn(move || {
        // 시험마다 몇 번만 부르므로 넉넉히 받는다
        for stream in listener.incoming().take(8) {
            let mut stream = match stream {
                Ok(s) => s,
                Err(_) => break,
            };
            // **요청을 끝까지 읽는다.** 헤더만 읽고 답한 뒤 닫으면, 아직 본문을
            // 보내던 쪽이 연결이 끊긴 것으로 보고 오류를 낸다 (Windows 10054).
            let req = read_request(&mut stream);
            let first = req.lines().next().unwrap_or("").to_string();

            let out = match reply(&first) {
                Some(b) => format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{b}"
                ),
                None => "HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n".to_string(),
            };
            let _ = stream.write_all(out.as_bytes());
            let _ = stream.flush();
        }
    });

    std::env::set_var("OLLAMA_HOST", format!("127.0.0.1:{port}"));
    let result = body();
    std::env::remove_var("OLLAMA_HOST");
    drop(handle); // 남은 스레드는 프로세스가 끝날 때 정리된다
    result
}

// ── 어디에 붙는가 ────────────────────────────────────────────────────

#[test]
fn 기본은_언제나_이_pc_안이다() {
    let _g = serialized();
    std::env::remove_var("OLLAMA_HOST");
    assert_eq!(address(), "127.0.0.1:11434");
    assert!(endpoint("/api/version").starts_with("http://127.0.0.1:11434/"));
}

#[test]
fn 포트를_바꿔_쓰는_사람도_있다() {
    let _g = serialized();
    std::env::set_var("OLLAMA_HOST", "127.0.0.1:9999");
    assert_eq!(address(), "127.0.0.1:9999");
    std::env::remove_var("OLLAMA_HOST");
}

#[test]
fn 이_pc_밖을_가리키면_무시한다() {
    // 업무자료를 다루는 프로그램이 남의 컴퓨터에 붙는 일은 없어야 한다
    let _g = serialized();
    for outside in ["192.168.0.5:11434", "ollama.example.com:11434", "8.8.8.8:80"] {
        std::env::set_var("OLLAMA_HOST", outside);
        assert_eq!(address(), "127.0.0.1:11434", "{outside} 를 받아들였습니다");
    }
    std::env::remove_var("OLLAMA_HOST");
}

// ── 상태 알아보기 ────────────────────────────────────────────────────

#[test]
fn 판을_읽는다() {
    let _g = serialized();
    let v = with_server(
        |req| {
            if req.contains("/api/version") {
                Some(r#"{"version":"0.12.3"}"#.to_string())
            } else {
                None
            }
        },
        version,
    );
    assert_eq!(v.unwrap(), "0.12.3");
}

#[test]
fn 포트에_다른_프로그램이_있으면_알아챈다() {
    // 열려 있다고 다 Ollama 가 아니다. 판을 못 읽으면 아니라고 해야 한다.
    let _g = serialized();
    let v = with_server(
        |_| Some("<html>나는 Ollama 가 아닙니다</html>".to_string()),
        version,
    );
    assert!(v.is_err());
    let msg = v.unwrap_err();
    // 영어 파싱 오류를 그대로 보여 주지 않고, 무엇을 하면 되는지 적어야 한다
    assert!(msg.contains("다른 프로그램"), "{msg}");
    assert!(msg.contains("다시 확인"), "할 일이 적혀 있지 않습니다: {msg}");
    assert!(!msg.contains("JSON"), "영어 오류가 새어 나왔습니다: {msg}");
}

#[test]
fn 판_대신_엉뚱한_json_이_와도_알아챈다() {
    let _g = serialized();
    let v = with_server(|_| Some(r#"{"hello":"world"}"#.to_string()), version);
    assert!(v.is_err());
}

#[test]
fn 받아_둔_모델을_읽는다() {
    let _g = serialized();
    let list = with_server(
        |req| {
            if req.contains("/api/tags") {
                Some(
                    r#"{"models":[{"name":"bge-m3:latest","size":1228000000},
                                  {"name":"gemma3:4b","size":3338801152}]}"#
                        .to_string(),
                )
            } else {
                None
            }
        },
        installed_models,
    )
    .unwrap();

    assert_eq!(list.len(), 2);
    assert_eq!(list[0].tag, "bge-m3:latest");
    assert_eq!(list[1].size_bytes, 3_338_801_152);
    // 목록에 있는 모델과 이어져야 한다
    assert_eq!(catalog::by_tag(&list[0].tag).map(|m| m.id), Some("embed-standard"));
}

#[test]
fn 모델이_하나도_없어도_괜찮다() {
    let _g = serialized();
    let list = with_server(
        |_| Some(r#"{"models":[]}"#.to_string()),
        installed_models,
    )
    .unwrap();
    assert!(list.is_empty());
}

// ── 받은 뒤 정말 도는지 ──────────────────────────────────────────────

#[test]
fn 답변_모델이_도는지_본다() {
    let _g = serialized();
    let r = with_server(
        |req| {
            if req.contains("/api/generate") {
                Some(r#"{"response":"2","done":true}"#.to_string())
            } else {
                None
            }
        },
        || test_chat("gemma3:4b"),
    );
    assert_eq!(r.unwrap(), "2");
}

#[test]
fn 빈_답을_주면_실패로_본다() {
    // 받아지긴 했는데 모델이 망가진 경우
    let _g = serialized();
    let r = with_server(
        |_| Some(r#"{"response":"  ","done":true}"#.to_string()),
        || test_chat("gemma3:4b"),
    );
    assert!(r.is_err());
}

#[test]
fn 검색_모델이_도는지_본다() {
    let _g = serialized();
    let dim = with_server(
        |req| {
            if req.contains("/api/embed") {
                let v = (0..1024).map(|_| "0.01").collect::<Vec<_>>().join(",");
                Some(format!(r#"{{"embeddings":[[{v}]]}}"#))
            } else {
                None
            }
        },
        || test_embed("bge-m3"),
    );
    assert_eq!(dim.unwrap(), 1024);
}

#[test]
fn 빈_벡터를_주면_실패로_본다() {
    let _g = serialized();
    let r = with_server(
        |_| Some(r#"{"embeddings":[]}"#.to_string()),
        || test_embed("bge-m3"),
    );
    assert!(r.is_err());
}

// ── 붙지 못할 때 ─────────────────────────────────────────────────────

#[test]
fn 아무도_안_듣고_있으면_거부로_본다() {
    // "안 켜짐"(거부)과 "켜졌는데 못 붙음"(시간 초과)을 갈라야 화면에서
    // 다른 안내를 보여 줄 수 있다.
    //
    // 1번 포트를 쓴다. 방금 닫은 포트는 잠시 이상하게 굴 수 있어서
    // (TIME_WAIT) 시험이 들쭉날쭉해진다.
    let _g = serialized();
    std::env::set_var("OLLAMA_HOST", "127.0.0.1:1");
    let r = probe_port();
    std::env::remove_var("OLLAMA_HOST");
    assert_eq!(r, Reach::Refused, "{r:?}");
}

#[test]
fn 단계_이름을_사람_말로_바꾼다() {
    assert_eq!(step_name("pulling manifest"), "목록 확인 중");
    assert_eq!(step_name("pulling 8934d96d3f08"), "내려받는 중");
    assert_eq!(step_name("verifying sha256 digest"), "확인하는 중");
    assert_eq!(step_name("writing manifest"), "갈무리하는 중");
    assert_eq!(step_name("success"), "끝");
}
