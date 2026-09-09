//! 쓸 수 있는 AI 모델 목록.
//!
//! **모델 이름을 코드 곳곳에 박지 않는다** (설계안 결정사항 3). 바꿔 끼울 수
//! 있어야 하고, 사양에 따라 다른 것을 권해야 하고, 사용자가 딴 것을 고를 수도
//! 있어야 한다. 그래서 한자리에 모아 둔다.
//!
//! 지금은 코드 안의 표지만, 생김새를 설정 파일과 같게 해 두었으므로 나중에
//! 밖으로 빼기 쉽다.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// 답변을 쓰는 모델
    Chat,
    /// 뜻으로 찾기에 쓰는 모델
    Embed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSpec {
    /// 프로그램 안에서 부르는 이름. 설정에 저장되는 값이다
    pub id: &'static str,
    /// 화면에 보이는 이름. 사용자는 모델 태그를 몰라도 된다
    pub name: &'static str,
    /// Ollama 에 넘기는 이름
    pub tag: &'static str,
    pub role: Role,
    /// 이만큼은 있어야 쓸 만하다
    pub min_ram_gb: u64,
    /// 내려받을 크기 어림 (GB)
    pub download_gb: f64,
    pub note: &'static str,
    /// 검색 모델이 돌려주는 벡터 길이. 답변 모델은 0.
    ///
    /// **이 값이 실제와 다르면 색인이 조용히 어긋난다.** 그래서 모델을 받은 뒤
    /// `test_embed` 로 실제 길이를 확인하고, 다르면 화면에서 알린다.
    pub dim: i64,
    /// 색인이 얼마나 걸리는지 어림 — 청크 하나당 밀리초.
    ///
    /// P4b 에서 이 PC(100% CPU)로 잰 값이다. GPU 를 쓰면 훨씬 빠르지만,
    /// 학교 PC 는 대개 CPU 로 돈다. 남은 시간을 처음 보여 줄 때만 쓰고,
    /// 그 뒤에는 실제 속도로 고친다.
    pub ms_per_chunk: i64,
}

pub const MODELS: &[ModelSpec] = &[
    // ── 답변 모델 ────────────────────────────────────────────────────
    ModelSpec {
        id: "chat-light",
        name: "가벼운 답변 모델",
        tag: "gemma3:4b",
        role: Role::Chat,
        min_ram_gb: 8,
        download_gb: 3.3,
        note: "메모리가 넉넉하지 않은 PC 에서도 돕니다. 요약과 자료 검색 답변에 쓸 만합니다.",
        dim: 0,
        ms_per_chunk: 0,
    },
    ModelSpec {
        id: "chat-standard",
        name: "기본 답변 모델",
        tag: "qwen3:8b",
        role: Role::Chat,
        min_ram_gb: 16,
        download_gb: 5.2,
        note: "규정 해석처럼 길게 따져야 하는 일에서 가벼운 모델보다 낫습니다.",
        dim: 0,
        ms_per_chunk: 0,
    },
    // ── 검색용 모델 ──────────────────────────────────────────────────
    ModelSpec {
        id: "embed-standard",
        name: "기본 검색 모델",
        tag: "bge-m3",
        role: Role::Embed,
        min_ram_gb: 8,
        download_gb: 1.2,
        note: "한국어를 포함해 여러 말을 다룹니다. 긴 문서에 강합니다.",
        dim: 1024,
        ms_per_chunk: 770,
    },
    ModelSpec {
        id: "embed-light",
        name: "가벼운 검색 모델",
        tag: "paraphrase-multilingual",
        role: Role::Embed,
        min_ram_gb: 4,
        download_gb: 0.6,
        note: "메모리가 아주 적은 PC 용입니다. 찾는 솜씨는 기본 모델보다 떨어집니다.",
        dim: 768,
        ms_per_chunk: 90,
    },
];

pub fn by_id(id: &str) -> Option<&'static ModelSpec> {
    MODELS.iter().find(|m| m.id == id)
}

pub fn by_tag(tag: &str) -> Option<&'static ModelSpec> {
    // Ollama 는 `bge-m3` 를 `bge-m3:latest` 로 적어 돌려준다
    let base = |s: &str| s.split(':').next().unwrap_or(s).to_string();
    MODELS
        .iter()
        .find(|m| m.tag == tag || base(m.tag) == base(tag))
}

/// 이 PC 에 무엇을 권할까.
///
/// 어디까지나 **권하는 것**이다. 사용자가 딴 것을 고르면 그대로 따른다.
pub fn recommended(ram_gb: Option<u64>) -> (&'static ModelSpec, &'static ModelSpec) {
    let ram = ram_gb.unwrap_or(8);
    let chat = if ram >= 16 { "chat-standard" } else { "chat-light" };
    let embed = if ram >= 8 { "embed-standard" } else { "embed-light" };
    (by_id(chat).unwrap(), by_id(embed).unwrap())
}

/// 권하는 까닭을 사람 말로.
pub fn reason(ram_gb: Option<u64>) -> String {
    match ram_gb {
        None => "이 PC 의 메모리를 알아내지 못했습니다. 무난한 쪽으로 권합니다.".into(),
        Some(r) if r >= 16 => format!("이 PC 의 메모리는 {r}GB 입니다. 더 좋은 모델을 쓸 수 있습니다."),
        Some(r) if r >= 8 => format!("이 PC 의 메모리는 {r}GB 입니다. 기본 조합을 권합니다."),
        Some(r) => format!("이 PC 의 메모리는 {r}GB 입니다. 가벼운 쪽으로 권합니다."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 메모리에_따라_다른_것을_권한다() {
        assert_eq!(recommended(Some(4)).0.id, "chat-light");
        assert_eq!(recommended(Some(4)).1.id, "embed-light");
        assert_eq!(recommended(Some(8)).0.id, "chat-light");
        assert_eq!(recommended(Some(8)).1.id, "embed-standard");
        assert_eq!(recommended(Some(32)).0.id, "chat-standard");
        assert_eq!(recommended(Some(32)).1.id, "embed-standard");
    }

    #[test]
    fn 메모리를_모르면_무난한_쪽으로() {
        let (chat, embed) = recommended(None);
        assert_eq!(chat.id, "chat-light");
        assert_eq!(embed.id, "embed-standard");
    }

    #[test]
    fn latest_가_붙어_와도_알아본다() {
        // Ollama 는 `bge-m3` 를 `bge-m3:latest` 로 적어 돌려준다
        assert_eq!(by_tag("bge-m3:latest").map(|m| m.id), Some("embed-standard"));
        assert_eq!(by_tag("gemma3:4b").map(|m| m.id), Some("chat-light"));
        assert!(by_tag("llama3:70b").is_none());
    }

    #[test]
    fn 모든_모델의_id_가_다르다() {
        for (i, a) in MODELS.iter().enumerate() {
            for b in &MODELS[i + 1..] {
                assert_ne!(a.id, b.id, "id 가 겹칩니다: {}", a.id);
            }
        }
    }

    #[test]
    fn 검색_모델은_벡터_길이를_적어_둔다() {
        // 이 값으로 "쓸 수 있는 벡터" 를 가린다. 비어 있으면 색인이 조용히 어긋난다.
        for m in MODELS {
            match m.role {
                Role::Embed => {
                    assert!(m.dim > 0, "{} 에 벡터 길이가 없습니다", m.id);
                    assert!(m.ms_per_chunk > 0, "{} 에 색인 속도 어림이 없습니다", m.id);
                }
                Role::Chat => assert_eq!(m.dim, 0),
            }
        }
    }

    #[test]
    fn 권하는_것은_반드시_목록에_있다() {
        for ram in [2, 4, 8, 16, 64] {
            let (c, e) = recommended(Some(ram));
            assert!(by_id(c.id).is_some());
            assert!(by_id(e.id).is_some());
            assert_eq!(c.role, Role::Chat);
            assert_eq!(e.role, Role::Embed);
        }
    }
}
