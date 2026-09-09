//! 거부 판단 시험.
//!
//! **거부와 오답 둘 다를 본다.** 답할 수 있는 물음을 거부하는 것도 고장이다.

use super::*;
use crate::answer::context::Evidence;
use crate::answer::parse::parse;
use crate::answer::verify::verify;

fn ev(source_id: &str, text: &str) -> Evidence {
    Evidence {
        source_id: source_id.into(),
        document_id: 1,
        doc_title: "지침".into(),
        chunk_id: 1,
        ord: 1,
        page_start: 3,
        page_end: 3,
        heading_path: None,
        text: text.into(),
        spans: vec![],
        neighbor: false,
        keyword_rank: Some(1),
        semantic_rank: None,
        search_rank: Some(1),
    }
}

/// 답변 JSON + 근거로 판단까지 한 번에
fn judge(json: &str, evidence: &[Evidence], hits: usize) -> Judgement {
    let d = parse(json).unwrap();
    let v = verify(&d, evidence);
    decide(&d, &v, evidence.len(), hits)
}

const GOOD: &str = r#"{"answer":"1인당 50,000원 이내입니다.",
  "claims":[{"text":"1인당 50,000원 이내다","sources":["근거1"],"kind":"fact"}],
  "insufficientEvidence":false}"#;

#[test]
fn 근거가_맞으면_답한다() {
    let e = vec![ev("근거1", "경조사비는 1인당 50,000원 이내 집행 가능")];
    let j = judge(GOOD, &e, 5);
    assert_eq!(j.decision, Decision::Answer, "{j:?}");
    assert!(j.reasons.is_empty());
}

#[test]
fn 검색이_아무것도_못_찾으면_거부한다() {
    let j = judge(GOOD, &[], 0);
    assert_eq!(j.decision, Decision::Refuse);
    assert!(j.reasons[0].contains("찾지 못했습니다"), "{j:?}");
}

#[test]
fn 모델이_없다고_하면_거부한다() {
    let e = vec![ev("근거1", "아무 상관 없는 글")];
    let j = judge(
        r#"{"answer":"모르겠습니다","claims":[],"insufficientEvidence":true}"#,
        &e,
        3,
    );
    assert_eq!(j.decision, Decision::Refuse);
    assert!(j.reasons[0].contains("인용된 근거가 하나도") || j.reasons[0].contains("근거만으로는"), "{j:?}");
}

#[test]
fn 없다고_표시했지만_제대로_답했으면_버리지_않는다() {
    // ★ gemma3:4b 가 실제로 이렇게 한다 — 답과 인용을 제대로 붙여 놓고도
    // insufficientEvidence 를 true 로 둔다. 그 표시만 보고 거부하면
    // **맞는 답을 버린다.** 대신 표시했다는 사실을 함께 적는다.
    let e = vec![ev("근거1", "소득을 기준으로 저소득층 학생을 우선 지원한다")];
    let j = judge(
        r#"{"answer":"저소득층 학생을 우선 지원합니다.",
            "claims":[{"text":"저소득층 학생을 우선 지원한다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":true}"#,
        &e,
        5,
    );
    assert_eq!(j.decision, Decision::Limited, "{j:?}");
    assert!(j.reasons.iter().any(|r| r.contains("넉넉하지 않다고 표시")), "{j:?}");
}

#[test]
fn 답이_비어_있으면_거부한다() {
    let e = vec![ev("근거1", "글")];
    let j = judge(r#"{"answer":"   ","claims":[],"insufficientEvidence":false}"#, &e, 3);
    assert_eq!(j.decision, Decision::Refuse);
}

#[test]
fn 인용이_하나도_없으면_거부한다() {
    // 근거를 주었는데도 아무것도 인용하지 않았다면, 근거에서 나온 답이 아니다
    let e = vec![ev("근거1", "경조사비는 1인당 50,000원")];
    let j = judge(
        r#"{"answer":"5만원입니다.","claims":[],"insufficientEvidence":false}"#,
        &e,
        3,
    );
    assert_eq!(j.decision, Decision::Refuse);
    assert!(j.reasons[0].contains("인용된 근거가 하나도"), "{j:?}");
}

#[test]
fn 인용을_다_지어냈으면_거부한다() {
    let e = vec![ev("근거1", "경조사비는 1인당 50,000원")];
    let j = judge(
        r#"{"answer":"전교생이 대상입니다.",
            "claims":[{"text":"전교생이 대상이다","sources":["근거9"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
        &e,
        3,
    );
    assert_eq!(j.decision, Decision::Refuse);
    assert!(j.reasons[0].contains("자료에 없습니다"), "{j:?}");
}

#[test]
fn 인용_일부만_지어냈으면_제한적으로_답한다() {
    let e = vec![ev("근거1", "경조사비는 1인당 50,000원")];
    let j = judge(
        r#"{"answer":"1인당 50,000원입니다.",
            "claims":[{"text":"1인당 50,000원이다","sources":["근거1"],"kind":"fact"},
                      {"text":"전교생이 대상이다","sources":["근거9"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
        &e,
        3,
    );
    assert_eq!(j.decision, Decision::Limited, "{j:?}");
    assert!(j.reasons.iter().any(|r| r.contains("일부 인용")), "{j:?}");
}

#[test]
fn 숫자를_확인하지_못하면_제한적으로_답한다() {
    // 근거에 없는 금액을 적었다 — 지어낸 값일 수 있다
    let e = vec![ev("근거1", "경조사비는 1인당 50,000원 이내")];
    let j = judge(
        r#"{"answer":"1인당 70,000원 이내입니다.",
            "claims":[{"text":"1인당 70,000원 이내다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
        &e,
        3,
    );
    assert_eq!(j.decision, Decision::Limited);
    assert!(j.reasons.iter().any(|r| r.contains("숫자")), "{j:?}");
}

#[test]
fn 인용이_빠진_주장이_있으면_제한적으로_답한다() {
    let e = vec![ev("근거1", "경조사비는 1인당 50,000원 이내")];
    let j = judge(
        r#"{"answer":"1인당 50,000원 이내입니다.",
            "claims":[{"text":"1인당 50,000원 이내다","sources":["근거1"],"kind":"fact"},
                      {"text":"학교장이 정한다","sources":[],"kind":"fact"}],
            "insufficientEvidence":false}"#,
        &e,
        3,
    );
    assert_eq!(j.decision, Decision::Limited);
}

#[test]
fn 해석이_섞여_있어도_그것만으로는_거부하지_않는다() {
    // 규정 해석은 이 프로그램이 하려는 일 가운데 하나다. 해석이라고 막으면 안 된다.
    let e = vec![ev("근거1", "학교운영위원회 심의를 거쳐야 한다")];
    let j = judge(
        r#"{"answer":"심의를 거치면 가능합니다.",
            "claims":[{"text":"심의를 거쳐야 한다","sources":["근거1"],"kind":"fact"},
                      {"text":"따라서 가능하다고 볼 수 있다","sources":["근거1"],"kind":"interpretation"}],
            "insufficientEvidence":false}"#,
        &e,
        3,
    );
    assert_eq!(j.decision, Decision::Answer, "{j:?}");
}

#[test]
fn 거부_문구는_정해진_말을_쓴다() {
    // 사용자가 정해 준 문구다
    assert_eq!(
        REFUSAL,
        "등록된 자료에서는 이 질문에 답할 수 있는 근거를 충분히 찾지 못했습니다."
    );
    assert!(RELATED.contains("관련이 있습니다"));
}

#[test]
fn 인용한_근거에_그_말이_없으면_거부한다() {
    // 자료에 없는 것을 물었을 때의 마지막 방어선이다.
    // 인용은 실재하고 숫자도 근거에 있지만, 주장이 그 근거에서 나오지 않았다.
    let e = vec![ev(
        "근거1",
        "교직원 생일기념 경비 - 소속 교직원의 생일시 소액(1인당 3만원이하)의 상품권",
    )];
    let j = judge(
        r#"{"answer":"명절 휴가비는 1인당 3만원 이하입니다.",
            "claims":[{"text":"명절 휴가비는 1인당 3만원 이하다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
        &e,
        5,
    );
    assert_eq!(j.decision, Decision::Refuse, "{j:?}");
    assert!(j.reasons.iter().any(|r| r.contains("확인되지 않습니다")), "{j:?}");
}

#[test]
fn 물음말은_핵심어로_세지_않는다() {
    // `얼마까지`, `있어` 같은 말이 근거에 없다고 경고를 붙이면
    // **모든 답에 경고가 붙어 아무 뜻이 없어진다.**
    let g = question_gap(
        "경조사비는 1인당 얼마까지 집행할 수 있어?",
        &["경조사비는 1인당 50,000원 이내 집행 가능".to_string()],
    );
    assert!(g.missing.is_empty(), "{:?}", g.missing);
    assert!(g.total >= 2, "핵심어를 뽑기는 해야 한다: {g:?}");
}

#[test]
fn 인용이_없으면_핵심어를_보지_않는다() {
    // 볼 근거가 없으면 "핵심어가 없다" 가 아니라 **잴 수 없다** 다.
    let g = question_gap("명절 휴가비는 얼마야?", &[]);
    assert_eq!(g.total, 0);
    assert!(g.missing.is_empty());
    assert_eq!(g.coverage(), 1.0);
}

#[test]
fn 물음_핵심어가_근거에_없어도_그것만으로는_거부하지_않는다() {
    // ★ 재 보고 정한 것이다 (`refuse::question_gap` 의 표).
    //
    // 여기서 모델은 근거를 **제대로 옮겨 적었다.** 다만 그 근거가 물음
    // ("명절 휴가비")에 대한 답이 아니다. 물음의 `명절`·`휴가비` 가 인용
    // 근거에 없다는 것이 유일한 단서다 — 그리고 그 단서로 자르면 지어낸 답
    // 하나를 막는 값이 맞는 답 둘이었다. 그래서 자르지 않는다.
    //
    // 이 시험은 그 결정을 못 박아 둔다. 나중에 이 신호로 거부하게 되면
    // 이 시험이 깨지고, 깨질 때 위의 값을 다시 재게 된다.
    let e = vec![ev(
        "근거1",
        "교직원 생일기념 경비 - 소속 교직원의 생일시 소액(1인당 3만원이하)의 상품권",
    )];
    let j = judge(
        r#"{"answer":"1인당 3만원 이하입니다.",
            "claims":[{"text":"소속 교직원의 생일시 소액(1인당 3만원이하)의 상품권","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
        &e,
        5,
    );
    assert_ne!(j.decision, Decision::Refuse, "{j:?}");

    // 신호 자체는 이 경우를 알아본다 — 쓰지 않기로 한 것이지 못 보는 것이 아니다
    let g = question_gap(
        "교직원 명절 휴가비는 1인당 얼마까지 집행할 수 있어?",
        &[e[0].text.clone()],
    );
    assert!(g.missing.iter().any(|w| w.contains("명절")), "{g:?}");
    assert!(g.missing.iter().any(|w| w.contains("휴가비")), "{g:?}");
    assert!(g.coverage() < 0.5, "{g:?}");
}
