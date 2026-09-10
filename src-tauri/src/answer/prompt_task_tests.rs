//! 문서 작성(P7)의 옷 — 지시어 떼기, 형식 규칙, JSON 꼴.

use super::*;

#[test]
fn 문서_요청에서_지시어를_떼고_찾을_말만_남긴다() {
    // ★ 사용자가 든 예. 그대로 찾으면 `가정통신문`·`작성` 을 찾고, 초점 낱말이 `작성` 이
    // 되어 자료집에 없다고 모든 요청을 거부한다.
    let q = Task::Letter.query_text("3학년 지원금 관련 내용을 검토해서 학부모에게 안내할 가정통신문을 간략하게 작성해줘");
    assert_eq!(q, "3학년 지원금");
    let q = Task::Sms.query_text("자유수강권 신청 기간을 학부모께 문자로 알릴 내용을 짧게 써줘");
    assert_eq!(q, "자유수강권 신청 기간을");
}

#[test]
fn 물음은_그대로_둔다() {
    let q = "경조사비는 1인당 얼마까지 집행할 수 있어?";
    assert_eq!(Task::Interpret.query_text(q), q);
}

#[test]
fn 다_떼어_버리면_원문으로_찾는다() {
    // 아무것도 못 찾는 것보다 낫다
    assert_eq!(strip_instruction("가정통신문 작성해줘"), "가정통신문 작성해줘");
}

#[test]
fn 형식_규칙은_근거_규칙_뒤에_붙는다() {
    // 근거 규칙(①~⑥)을 바꾸지 않는다 — 문서 작성도 같은 규칙으로 묶인다
    for t in [Task::Letter, Task::Sms] {
        let s = system(t);
        assert!(s.starts_with(SYSTEM), "{t:?}");
        assert!(s.contains("지어내지 않는다"), "{t:?}");
        assert!(s.contains("kind=style"), "{t:?}");
    }
    assert!(system(Task::Letter).contains("[학교명]"));
    assert!(system(Task::Sms).contains("90자"));
    assert_eq!(system(Task::Interpret), SYSTEM);
}

#[test]
fn 가정통신문은_제목이_필수고_문자는_없다() {
    let l = schema_for(Task::Letter);
    assert!(l["properties"]["title"].is_object());
    assert!(l["required"].as_array().unwrap().iter().any(|v| v == "title"));
    let s = schema_for(Task::Sms);
    assert!(s["properties"].get("title").is_none());
    // 둘 다 style 주장을 받는다
    for sch in [&l, &s] {
        let kinds = &sch["properties"]["claims"]["items"]["properties"]["kind"]["enum"];
        assert!(kinds.as_array().unwrap().iter().any(|v| v == "style"));
    }
    // 규정 해석의 꼴은 그대로 (style 없음)
    let i = schema_for(Task::Interpret);
    let kinds = &i["properties"]["claims"]["items"]["properties"]["kind"]["enum"];
    assert!(!kinds.as_array().unwrap().iter().any(|v| v == "style"));
}

#[test]
fn 요청_글에는_형식_이름이_들어간다() {
    let u = user_for(Task::Letter, "지원금 안내", "근거1 …");
    assert!(u.contains("[요청]") && u.contains("가정통신문"));
    let u = user_for(Task::Sms, "지원금 안내", "");
    assert!(u.contains("insufficientEvidence") && u.contains("문자 메시지"));
    assert_eq!(user_for(Task::Interpret, "q", "e"), user("q", "e"));
}

#[test]
fn 이름과_되읽기가_맞는다() {
    for t in [Task::Interpret, Task::Letter, Task::Sms] {
        assert_eq!(Task::from_name(t.name()), Some(t));
    }
    assert_eq!(Task::from_name("compare"), None);
}
