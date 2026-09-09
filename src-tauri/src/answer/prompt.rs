//! LLM 에게 무엇을 어떻게 시킬지.
//!
//! 이 프로그램에서 프롬프트가 하는 일은 "답을 잘 쓰게" 하는 것이 아니라
//! **없는 것을 지어내지 않게** 하는 것이다. 그래서 규칙이 짧고 셈,
//! 그리고 형식이 정해져 있다.
//!
//! **왜 JSON 으로 받는가.** 자유 글로 받으면 "근거2에 따르면" 이라고 쓴 것을
//! 우리가 다시 글자로 찾아야 하고, 모델이 `근거 2`·`[근거2]`·`두 번째 근거`
//! 처럼 조금씩 다르게 쓰면 인용을 놓친다. 인용을 놓치면 검증이 통째로
//! 헛것이 된다. 그래서 주장과 인용을 **자리로** 받는다.
//!
//! Ollama 에는 형식을 스키마로 못 박는 길이 있다(`format`). 4B 모델은 형식을
//! 자주 어기므로 이걸 쓴다 — 그래도 어길 때가 있어서 `parse` 는 너그럽게 읽는다.

use serde_json::json;

/// 지켜야 할 것. **짧게, 그리고 셈이 되게.**
///
/// 길게 쓰면 작은 모델은 앞부분만 따른다. 그래서 규칙을 번호로 여섯 개만 둔다.
pub const SYSTEM: &str = "\
너는 학교 업무자료를 읽고 답하는 도우미다. 아래 규칙을 반드시 지켜라.

1. [근거] 에 적힌 내용만 쓴다. 네가 따로 아는 지식으로 빈칸을 채우지 않는다.
   물음에 직접 답하는 근거만 쓴다. 물음과 상관없는 근거는 쓰지 않는다.
2. 근거에 없는 것은 없다고 말한다. 짐작하거나 그럴듯하게 만들지 않는다.
3. 모든 주장에 그 주장이 나온 근거 이름을 붙인다. 없는 근거 이름을 쓰지 않는다.
4. 금액·날짜·기한·횟수·비율·대상·학년·조항은 근거에 적힌 표현을 그대로 옮긴다.
   단위나 자릿수를 바꾸지 않는다.
5. 근거에 적혀 있는 사실(fact)과 네 해석(interpretation)을 갈라 적는다.
   근거가 그렇게 읽힌다는 뜻일 뿐이면 해석이다.
6. 근거로 답할 수 없으면 insufficientEvidence 를 true 로 하고 answer 는 비운다.

answer 는 물음에 바로 답하는 짧은 글로 쓴다. 근거를 그대로 길게 옮기지 않는다.
claims 의 text 는 주장 하나를 **한 문장으로** 적는다. 근거 문단을 통째로
복사하지 않는다.

답은 한국어로 쓴다. 정해진 JSON 하나만 내놓고 다른 말은 붙이지 않는다.";

/// 물음과 근거를 붙여 넘길 글을 만든다.
pub fn user(question: &str, evidence: &str) -> String {
    if evidence.trim().is_empty() {
        return format!(
            "[근거]\n(없음)\n\n[물음]\n{}\n\n근거가 없으므로 insufficientEvidence 를 true 로 한다.",
            question.trim()
        );
    }
    format!(
        "[근거]\n{}\n[물음]\n{}",
        evidence.trim_end(),
        question.trim()
    )
}

/// 받을 꼴을 못 박는다.
///
/// `claims` 가 이 구조의 핵심이다. 주장 하나마다 인용을 묶어 두면, 나중에
/// "그 숫자가 그 주장의 근거 안에 있는가" 를 주장 단위로 볼 수 있다.
/// (지금은 인용된 근거 전체에서 보지만, 자리를 미리 만들어 둔다 — 설계안 5-5)
pub fn schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "answer": {
                "type": "string",
                "description": "물음에 바로 답하는 짧은 글. 근거가 없으면 빈 문자열"
            },
            "claims": {
                "type": "array",
                "description": "답에 담긴 주장을 하나씩. 주장마다 근거 이름을 붙인다",
                "items": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "주장 하나를 한 문장으로" },
                        "sources": {
                            "type": "array",
                            "items": { "type": "string" }
                        },
                        "kind": { "type": "string", "enum": ["fact", "interpretation"] }
                    },
                    "required": ["text", "sources", "kind"]
                }
            },
            "insufficientEvidence": { "type": "boolean" },
            "confidence": { "type": "string", "enum": ["high", "medium", "low"] },
            "interpretationNotes": {
                "type": "array",
                "items": { "type": "string" }
            }
        },
        "required": ["answer", "claims", "insufficientEvidence"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 지켜야_할_것이_다_적혀_있다() {
        // 이 여섯 가지가 P5 의 요구사항 3 이다. 하나라도 빠지면 안 된다.
        for must in [
            "근거] 에 적힌 내용만",
            "없는 것은 없다고",
            "근거 이름을 붙인다",
            "그대로 옮긴다",
            "해석(interpretation)을 갈라",
            "insufficientEvidence",
        ] {
            assert!(SYSTEM.contains(must), "규칙이 빠졌습니다: {must}");
        }
    }

    #[test]
    fn 물음과_근거를_함께_넘긴다() {
        let p = user("지원 대상은?", "[근거1] 길라잡이 5쪽\n저소득층 학생\n");
        assert!(p.contains("지원 대상은?"));
        assert!(p.contains("[근거1]"));
        assert!(p.contains("저소득층 학생"));
    }

    #[test]
    fn 근거가_없으면_없다고_말하라고_넘긴다() {
        let p = user("지원 대상은?", "   ");
        assert!(p.contains("(없음)"), "{p}");
        assert!(p.contains("insufficientEvidence"), "{p}");
    }

    #[test]
    fn 스키마에_주장과_인용이_있다() {
        let s = schema();
        let props = &s["properties"];
        assert!(props["answer"].is_object());
        assert!(props["claims"]["items"]["properties"]["sources"].is_object());
        assert_eq!(
            props["claims"]["items"]["properties"]["kind"]["enum"][1],
            "interpretation"
        );
        // 이 셋은 반드시 받아야 한다
        let req = s["required"].as_array().unwrap();
        for k in ["answer", "claims", "insufficientEvidence"] {
            assert!(req.iter().any(|v| v == k), "{k} 가 필수가 아닙니다");
        }
    }
}
