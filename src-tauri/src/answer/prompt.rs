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

// ── 문서 작성 (P7) — 같은 파이프라인, 다른 옷 ────────────────────────

/// 무엇을 만들 것인가. 파이프라인은 하나고, 이 값이 프롬프트와 화면만 바꾼다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    /// 규정 해석 — 물음에 답한다 (P5)
    Interpret,
    /// 가정통신문 초안 — 제목 + 본문
    Letter,
    /// 문자 메시지 초안 — 짧은 본문
    Sms,
}

impl Task {
    pub fn from_name(s: &str) -> Option<Task> {
        match s {
            "interpret" => Some(Task::Interpret),
            "letter" => Some(Task::Letter),
            "sms" => Some(Task::Sms),
            _ => None,
        }
    }

    /// 기록(`job.kind`)과 화면에 쓰는 이름
    pub fn name(self) -> &'static str {
        match self {
            Task::Interpret => "interpret",
            Task::Letter => "letter",
            Task::Sms => "sms",
        }
    }

    pub fn is_draft(self) -> bool {
        self != Task::Interpret
    }

    /// 검색과 초점 낱말 검사에 넣을 글.
    ///
    /// 물음은 그대로 쓴다. 문서 요청은 **지시문**이라("…학부모에게 안내할 가정통신문을
    /// 간략하게 작성해줘") 지시어를 떼고 남은 것("3학년 지원금 관련 내용")으로 찾는다.
    /// 그대로 넣으면 낱말 검색이 `가정통신문`·`작성` 같은 말을 찾고, 초점 낱말이
    /// `작성` 이 되어 자료집에 없다고 모든 요청을 거부한다.
    pub fn query_text(self, request: &str) -> String {
        if !self.is_draft() {
            return request.to_string();
        }
        strip_instruction(request)
    }
}

/// 문서 요청에서 지시어를 뗀다. 물음말 목록(`domain::query`)과 같은 성격의 짧은 자다 —
/// 동의어 사전이 아니라 "이 말은 무엇을 찾을지가 아니라 어떻게 쓸지를 말한다" 는 목록.
pub fn strip_instruction(request: &str) -> String {
    const INSTRUCTION: &[&str] = &[
        "가정통신문", "가정통신문을", "가정통신문으로", "통신문", "문자", "문자를", "문자로",
        "메시지", "메시지를", "메세지", "안내문", "안내문을", "초안", "초안을", "초안으로",
        "작성", "작성해", "작성해줘", "작성해주세요", "작성하여", "써", "써줘", "써주세요",
        "만들어", "만들어줘", "만들어주세요", "정리해", "정리해줘", "요약해", "요약해줘",
        "검토해서", "검토하여", "검토해", "참고해서", "바탕으로", "토대로", "이용해서",
        "안내할", "안내하는", "안내", "알리는", "알릴", "보낼", "보내는", "발송할",
        "학부모에게", "학부모께", "학부모님께", "학부모용", "학생에게", "교직원에게", "가정에",
        "간략하게", "간단하게", "간단히", "짧게", "자세히", "정중하게", "친절하게", "부드럽게",
        "내용을", "내용으로", "내용", "관련", "관련된", "관한", "대한", "대해", "형식으로",
    ];
    let kept: Vec<&str> = request
        .split_whitespace()
        .filter(|w| {
            let bare = w.trim_matches(|c: char| !c.is_alphanumeric());
            !INSTRUCTION.contains(&bare)
        })
        .collect();
    let s = kept.join(" ");
    // 다 떼어 버렸으면 원문으로 — 아무것도 못 찾는 것보다 낫다
    if s.trim().is_empty() { request.to_string() } else { s }
}

/// 형식 규칙 — 규칙 ①~⑥(`SYSTEM`) 뒤에 붙는다. **근거 규칙은 바꾸지 않는다.**
const LETTER_RULES: &str = "\

[가정통신문 형식]
- title 에 제목을, answer 에 본문을 쓴다. 본문은 300~500자, 존댓말.
- 본문 차례: 인사 한 문장 → 안내 내용(근거에 있는 것만) → 마무리 한 문장.
- 학교명·날짜·담당자·연락처·기간처럼 근거에 없는 값은 **지어내지 않는다.** 필요하면
  [학교명], [안내 기간], [담당자 연락처] 처럼 대괄호로 사용자가 채울 자리를 두거나 뺀다.
- claims 에는 본문의 문장을 하나씩 적는다. 근거에서 온 사실은 kind=fact 로 근거 이름을
  붙이고, 인사말·마무리·문장 연결처럼 사실이 아닌 말은 kind=style 로 sources 를 비운다.
- 금액·날짜·기한·횟수·비율·대상·학년은 근거의 표현을 그대로 옮긴다.";

const SMS_RULES: &str = "\

[문자 메시지 형식]
- answer 에 본문만 쓴다. 90자 안팎, 길어도 200자를 넘기지 않는다. 제목은 없다.
- 첫머리는 \"[학교명]입니다.\" 로 시작한다. 학교명을 지어내지 않는다.
- 핵심만: 누구에게(대상) · 무엇을(내용) · 언제까지(기한) · 무엇을 해야 하는지(요청).
  인사말과 설명은 넣지 않는다.
- 근거에 없는 값은 지어내지 않는다. 필요하면 [안내 기간] 처럼 대괄호 자리를 둔다.
- claims 에는 본문의 문장을 하나씩 적는다. 근거에서 온 사실은 kind=fact 로 근거 이름을
  붙이고, 사실이 아닌 말(\"[학교명]입니다.\" 같은 것)은 kind=style 로 sources 를 비운다.
- 금액·날짜·기한·횟수·비율·대상·학년은 근거의 표현을 그대로 옮긴다.";

/// 일에 맞는 규칙 글.
pub fn system(task: Task) -> String {
    match task {
        Task::Interpret => SYSTEM.to_string(),
        Task::Letter => format!("{SYSTEM}{LETTER_RULES}"),
        Task::Sms => format!("{SYSTEM}{SMS_RULES}"),
    }
}

/// 일에 맞는 물음/요청 글.
pub fn user_for(task: Task, request: &str, evidence: &str) -> String {
    if !task.is_draft() {
        return user(request, evidence);
    }
    let what = if task == Task::Letter { "가정통신문" } else { "문자 메시지" };
    if evidence.trim().is_empty() {
        return format!(
            "[근거]\n(없음)\n\n[요청]\n{}\n\n근거가 없으므로 insufficientEvidence 를 true 로 하고 {what} 을 쓰지 않는다.",
            request.trim()
        );
    }
    format!(
        "[근거]\n{}\n[요청]\n{}\n\n위 근거에 있는 내용만으로 {what} 초안을 쓴다.",
        evidence.trim_end(),
        request.trim()
    )
}

/// 일에 맞는 JSON 꼴. 문서 작성은 `title` 과 `style` 주장이 더 있다.
pub fn schema_for(task: Task) -> serde_json::Value {
    let mut s = schema();
    if task.is_draft() {
        s["properties"]["claims"]["items"]["properties"]["kind"]["enum"] =
            json!(["fact", "interpretation", "style"]);
    }
    if task == Task::Letter {
        s["properties"]["title"] = json!({ "type": "string", "description": "가정통신문 제목" });
        s["required"] = json!(["title", "answer", "claims", "insufficientEvidence"]);
    }
    s
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

#[cfg(test)]
#[path = "prompt_task_tests.rs"]
mod task_tests;
