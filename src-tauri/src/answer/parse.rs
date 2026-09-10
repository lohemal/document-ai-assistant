//! 모델이 내놓은 글을 **너그럽게** 읽는다.
//!
//! 형식을 스키마로 못 박아도 작은 모델은 어길 때가 있다 — 앞에 설명을 붙이고,
//! ```json 울타리로 감싸고, 뒤에 말을 더 쓴다. 여기서 실패하면 답변이 통째로
//! 버려지므로, 읽을 수 있는 데까지 읽는다.
//!
//! 다만 **없는 것을 지어 채우지는 않는다.** `answer` 가 없으면 실패로 본다 —
//! 빈 답을 정상으로 둘 수는 없다.

use super::numbers;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClaimKind {
    /// 근거에 그렇게 적혀 있다
    Fact,
    /// 근거를 그렇게 읽을 수 있다
    Interpretation,
    /// 인사말·마무리·문장 연결 — 사실이 아닌 말 (문서 작성, P7).
    /// 뒷받침·인용 검사에서 빼되, **숫자는 문장 종류를 가리지 않고** 본다.
    Style,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Claim {
    pub text: String,
    /// 이 주장이 나온 근거 이름들
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default = "fact")]
    pub kind: ClaimKind,
}

fn fact() -> ClaimKind {
    ClaimKind::Fact
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub answer: String,
    /// 가정통신문 제목 (문서 작성에서만). 물음 답변에는 없다.
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub claims: Vec<Claim>,
    #[serde(default)]
    pub insufficient_evidence: bool,
    #[serde(default)]
    pub confidence: Option<String>,
    #[serde(default)]
    pub interpretation_notes: Vec<String>,
}

impl Draft {
    /// 주장 가운데 해석이 하나라도 있는가
    pub fn has_interpretation(&self) -> bool {
        self.claims.iter().any(|c| c.kind == ClaimKind::Interpretation)
            || !self.interpretation_notes.is_empty()
    }

    /// 인용된 근거 이름 (겹치는 것 없이)
    pub fn cited(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for c in &self.claims {
            for s in &c.sources {
                let s = s.trim().to_string();
                if !s.is_empty() && !out.contains(&s) {
                    out.push(s);
                }
            }
        }
        out
    }

    /// 답변 글에 든 숫자
    pub fn numbers(&self) -> Vec<numbers::Num> {
        numbers::extract(&self.answer)
    }
}

/// JSON 을 찾아 읽는다.
pub fn parse(raw: &str) -> Result<Draft, String> {
    let body = slice_json(raw).ok_or_else(|| {
        // 시작은 JSON 인데 닫히지 않았다면 **길어서 잘린 것**이다.
        // 까닭이 다르면 할 일도 다르므로 갈라 말한다.
        if raw.trim_start().starts_with('{') || raw.contains("\"answer\"") {
            "AI 가 답을 너무 길게 쓰다가 잘렸습니다. 물음을 좁혀 다시 물어봐 주세요.".to_string()
        } else {
            format!(
                "모델이 정해진 형식으로 답하지 않았습니다. 받은 글: {}",
                raw.chars().take(120).collect::<String>()
            )
        }
    })?;

    let mut draft: Draft = serde_json::from_str(&body).map_err(|e| {
        format!(
            "모델의 답을 읽지 못했습니다 ({e}). 받은 글: {}",
            body.chars().take(120).collect::<String>()
        )
    })?;

    // 답 자리에 **자리 이름을 그대로 적어 놓는** 일이 있다.
    //
    //   {"answer": "insufficientEvidence", ..., "insufficientEvidence": true}
    //
    // gemma3:4b 에서 실제로 나왔다. 그대로 두면 화면에 답변으로
    // `insufficientEvidence` 가 뜬다. 모델이 하려던 말은 "근거가 없다" 이므로
    // 답을 비우고 그 뜻만 남긴다.
    let bare = draft.answer.trim().to_lowercase().replace([' ', '_', '-'], "");
    if matches!(
        bare.as_str(),
        "insufficientevidence" | "insufficient" | "none" | "null" | "n/a" | "na" | "false" | "true"
    ) {
        draft.answer.clear();
        draft.insufficient_evidence = true;
    }

    Ok(draft)
}

/// 앞뒤에 붙은 말과 울타리를 걷고 JSON 몸통만 남긴다.
fn slice_json(raw: &str) -> Option<String> {
    let t = raw.trim();
    // ```json … ``` 울타리
    let t = if let Some(rest) = t.strip_prefix("```") {
        let rest = rest.strip_prefix("json").unwrap_or(rest);
        rest.trim_start().trim_end_matches('`').trim_end()
    } else {
        t
    };
    let start = t.find('{')?;
    // 괄호 짝을 세어 끝을 찾는다. 문자열 안의 괄호는 세지 않는다.
    let bytes: Vec<char> = t[start..].chars().collect();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    for (i, c) in bytes.iter().enumerate() {
        if in_str {
            if escaped {
                escaped = false;
            } else if *c == '\\' {
                escaped = true;
            } else if *c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(bytes[..=i].iter().collect());
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
