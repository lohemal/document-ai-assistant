//! 인용·숫자 검증 시험.
//!
//! **가장 중요한 시험은 `숫자가_있어도_틀린_답을_잡지_못한다` 다.** 그것이
//! 이 검증의 한계이고, 화면 문구가 그 한계를 넘어서지 않는지를 여기서 못 박는다.

use super::*;
use crate::answer::parse::parse;

fn ev(source_id: &str, chunk_id: i64, text: &str) -> Evidence {
    Evidence {
        source_id: source_id.to_string(),
        document_id: 1,
        doc_title: "길라잡이".into(),
        chunk_id,
        ord: chunk_id,
        page_start: 10,
        page_end: 10,
        heading_path: None,
        text: text.to_string(),
        spans: vec![],
        neighbor: false,
        keyword_rank: Some(1),
        semantic_rank: None,
        search_rank: Some(1),
    }
}

fn draft(json: &str) -> super::Draft {
    parse(json).unwrap()
}

#[test]
fn 인용한_근거가_다_있으면_확인된다() {
    let e = vec![ev("근거1", 11, "저소득층 학생을 우선 지원한다")];
    let d = draft(
        r#"{"answer":"저소득층 학생을 우선 지원합니다.",
            "claims":[{"text":"저소득층 학생을 우선 지원한다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    assert!(v.citations_ok);
    assert!(v.unknown_sources.is_empty());
    assert_eq!(v.sources[0].chunk_id, Some(11));
    assert_eq!(v.sources[0].page, Some(10));
    assert!(v.citation_message.contains("모두 자료에서 확인"), "{}", v.citation_message);
}

#[test]
fn 없는_근거를_들면_인용이_깨진다() {
    // 작은 모델이 자주 하는 짓 — 넘기지 않은 이름을 만들어 쓴다
    let e = vec![ev("근거1", 11, "저소득층 학생")];
    let d = draft(
        r#"{"answer":"근거7에 따르면 전교생입니다.",
            "claims":[{"text":"전교생이 대상이다","sources":["근거7"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    assert!(!v.citations_ok);
    assert_eq!(v.unknown_sources, vec!["근거7"]);
    assert!(v.citation_message.contains("믿을 수 없습니다"), "{}", v.citation_message);
}

#[test]
fn 인용이_아예_없으면_확인할_수_없다() {
    let e = vec![ev("근거1", 11, "저소득층 학생")];
    let d = draft(r#"{"answer":"저소득층 학생입니다.","claims":[],"insufficientEvidence":false}"#);
    let v = verify(&d, &e);
    assert!(!v.citations_ok);
    assert!(v.citation_message.contains("인용된 근거가 없습니다"), "{}", v.citation_message);
}

#[test]
fn 인용이_빠진_주장을_센다() {
    let e = vec![ev("근거1", 11, "저소득층 학생")];
    let d = draft(
        r#"{"answer":"a",
            "claims":[{"text":"저소득층 학생","sources":["근거1"],"kind":"fact"},
                      {"text":"전교생도 가능하다","sources":[],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    assert_eq!(v.claims_without_source.len(), 1);
    assert!(v.citation_message.contains("인용이 붙지 않은 주장"), "{}", v.citation_message);
}

#[test]
fn 숫자를_인용된_근거에서만_찾는다() {
    // 근거2 에도 50,000원이 있지만, 주장은 근거1 만 인용했다
    let e = vec![
        ev("근거1", 11, "간담회 경비는 1인 1회당 4만원 이하"),
        ev("근거2", 12, "경조사비는 1인당 50,000원 이내"),
    ];
    let d = draft(
        r#"{"answer":"간담회 경비는 50,000원까지입니다.",
            "claims":[{"text":"간담회 경비는 50,000원까지다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    let n = v.numbers.iter().find(|n| n.raw.contains("50,000")).unwrap();
    assert!(!n.found, "인용하지 않은 근거의 숫자를 찾았습니다: {:?}", v.numbers);
    assert!(v.number_message.contains("확인하지 못했습니다"), "{}", v.number_message);
}

#[test]
fn 표기가_달라도_같은_값이면_확인된다() {
    let e = vec![ev("근거1", 11, "경조사비는 1인당 50,000원 이내 집행 가능")];
    let d = draft(
        r#"{"answer":"1인당 5만원 이내입니다.",
            "claims":[{"text":"1인당 5만원 이내다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    assert_eq!(v.numbers_missing(), 0, "{:?}", v.numbers);
    assert!(v.number_message.contains("확인되었습니다"), "{}", v.number_message);
}

#[test]
fn 검증_문구가_과장되지_않는다() {
    let e = vec![ev("근거1", 11, "1인당 50,000원 이내")];
    let d = draft(
        r#"{"answer":"50,000원입니다.",
            "claims":[{"text":"50,000원이다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    // 이 문구는 사용자가 직접 정해 준 것이다 (설계안 결정사항 7)
    assert_eq!(
        v.number_message,
        "답변에 사용된 주요 숫자가 인용 근거에서 확인되었습니다."
    );
    assert!(!v.number_message.contains("검증 완료"));
    assert!(!v.citation_message.contains("검증 완료"));
}

#[test]
fn 숫자가_있어도_틀린_답을_잡지_못한다() {
    // ★ 이 검증의 한계를 못 박는 시험이다 (사용자가 든 A/B 보기).
    //
    //   근거:  A 지원금 250,000원 / B 지원금 500,000원
    //   답변:  "A 지원금은 500,000원"   ← 틀렸다. 그런데 숫자는 근거에 있다.
    //
    // 지금 구조로는 **통과한다.** 그러니 통과를 "답이 맞다" 로 읽으면 안 된다.
    // 화면 문구가 "숫자가 인용 근거에서 확인되었다" 까지만 말하는 까닭이다.
    let e = vec![ev(
        "근거1",
        11,
        "A 지원금은 250,000원이다. B 지원금은 500,000원이다.",
    )];
    let d = draft(
        r#"{"answer":"A 지원금은 500,000원입니다.",
            "claims":[{"text":"A 지원금은 500,000원이다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);

    assert_eq!(v.numbers_missing(), 0, "숫자 검사는 통과한다 — 그것이 한계다");
    // 그래서 문구는 여기까지만이어야 한다
    assert!(v.number_message.contains("인용 근거에서 확인"));
    assert!(
        !v.number_message.contains("맞습니다") && !v.number_message.contains("정확"),
        "숫자 검사 결과를 답의 정확성으로 말하고 있습니다: {}",
        v.number_message
    );
}

#[test]
fn 해석이_섞였는지_알려_준다() {
    let e = vec![ev("근거1", 11, "학교운영위원회 심의를 거쳐야 한다")];
    let d = draft(
        r#"{"answer":"심의를 거치면 가능해 보입니다.",
            "claims":[{"text":"심의를 거쳐야 한다","sources":["근거1"],"kind":"fact"},
                      {"text":"이 경우 가능하다고 볼 수 있다","sources":["근거1"],"kind":"interpretation"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    assert!(v.has_interpretation);
}

#[test]
fn 숫자가_없는_답도_괜찮다() {
    let e = vec![ev("근거1", 11, "학교운영위원회 심의를 거쳐야 한다")];
    let d = draft(
        r#"{"answer":"심의를 거쳐야 합니다.",
            "claims":[{"text":"심의를 거쳐야 한다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    assert!(v.numbers.is_empty());
    assert!(v.number_message.contains("확인할 숫자가 없습니다"));
}

#[test]
fn 같은_숫자를_여러_번_세지_않는다() {
    let e = vec![ev("근거1", 11, "1인당 50,000원 이내")];
    let d = draft(
        r#"{"answer":"50,000원입니다. 다시 말해 50,000원 이내입니다.",
            "claims":[{"text":"50,000원 이내다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    assert_eq!(v.numbers.len(), 1, "{:?}", v.numbers);
}

#[test]
fn 근거_이름을_조금_다르게_써도_알아본다() {
    // ★ 실제로 나온 고장이다. 모델이 `근거 6` 이라고 빈칸을 넣어 쓰자
    // 없는 근거로 잡혀 **답을 지어낸 것으로 잘못 판단하고 거부했다.**
    let e = vec![
        ev("근거6", 11, "경조사비는 1인당 50,000원 이내"),
        ev("근거7", 12, "간담회 경비는 1인 1회당 4만원"),
    ];
    let d = draft(
        r#"{"answer":"1인당 50,000원 이내입니다.",
            "claims":[{"text":"1인당 50,000원 이내다","sources":["근거 6"],"kind":"fact"},
                      {"text":"간담회는 4만원","sources":["[근거7]"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    assert!(v.unknown_sources.is_empty(), "{:?}", v.unknown_sources);
    assert!(v.citations_ok);
    // 숫자도 그 근거에서 찾아야 한다
    assert_eq!(v.numbers_missing(), 0, "{:?}", v.numbers);
}

#[test]
fn 그래도_없는_근거는_없다고_한다() {
    // 다듬는다고 해서 없는 것을 있다고 하면 안 된다
    let e = vec![ev("근거1", 11, "글")];
    let d = draft(
        r#"{"answer":"a","claims":[{"text":"x","sources":["근거 9"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    assert_eq!(v.unknown_sources.len(), 1);
}

#[test]
fn 인용은_실재하는데_그_근거에_없는_말이면_알아챈다() {
    // ★ 자료에 없는 것을 물었을 때 모델이 하는 짓이다.
    //
    //   물음:  "교직원 명절 휴가비 한도는?"  (자료에 없다)
    //   근거:  생일기념 경비 1인당 3만원 이하
    //   답:    "명절 휴가비는 1인당 3만원 이하입니다"  ← 근거를 인용했지만 없는 말
    //
    // 인용이 실재하고 숫자도 근거에 있으므로 그 두 검사는 통과한다.
    // 주장의 낱말이 근거에 없다는 것으로 잡아낸다.
    let e = vec![ev(
        "근거1",
        11,
        "교직원 생일기념 경비 - 소속 교직원의 생일시 소액(1인당 3만원이하)의 상품권, 케익 등",
    )];
    let d = draft(
        r#"{"answer":"명절 휴가비는 1인당 3만원 이하입니다.",
            "claims":[{"text":"명절 휴가비는 1인당 3만원 이하다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);

    assert!(v.citations_ok, "인용은 실재한다");
    assert_eq!(v.numbers_missing(), 0, "숫자도 근거에 있다 — 그래서 이 검사만으로는 못 잡는다");
    assert!(v.nothing_supported(), "주장이 근거로 뒷받침되지 않아야 합니다: {:?}", v.claims);
}

#[test]
fn 어미를_바꿔_써도_뒷받침으로_본다() {
    // 근거의 `3만원이하` 를 답이 `3만원 이하다` 로 쓰는 것은 정상이다.
    // 이것을 "근거에 없다" 고 하면 맞는 답을 버린다.
    let e = vec![ev(
        "근거1",
        11,
        "교직원 생일기념 경비는 1인당 3만원이하의 상품권을 지급할 수 있다",
    )];
    let d = draft(
        r#"{"answer":"생일 상품권은 1인당 3만원 이하입니다.",
            "claims":[{"text":"교직원 생일기념 경비는 1인당 3만원 이하다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    assert!(!v.nothing_supported(), "{:?}", v.claims);
    assert!(v.claims[0].overlap > 0.6, "{:?}", v.claims);
}

#[test]
fn 근거_번호를_잘못_붙였으면_고쳐_붙인다() {
    // ★ 작은 모델이 가장 자주 하는 실수다. 근거는 제대로 옮겨 적고 번호만
    // 틀린다. 실제로 gemma3:4b 는 `유치원: 연 1회` 를 그대로 쓰고 근거1을
    // 인용했다. 그때 거부하면 **맞는 답을 버린다.** 그 말이 실제로 있는
    // 근거를 찾아 인용을 고쳐 붙이고, 고쳤다는 사실을 화면에 밝힌다.
    let e = vec![
        ev("근거1", 11, "정보통신윤리교육 추진 근거 - 교육기본법 제9조"),
        ev("근거2", 12, "교육 시수 - 초·중·고 연 2회 이상, 유치원 연 1회 이상"),
    ];
    let d = draft(
        r#"{"answer":"유치원은 연 1회 이상입니다.",
            "claims":[{"text":"유치원은 연 1회 이상 교육한다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);

    assert!(v.claims[0].supported, "{:?}", v.claims);
    assert_eq!(v.claims[0].sources, vec!["근거2"], "내용이 있는 근거로 옮겨 붙는다");
    assert_eq!(v.claims[0].repaired_from.as_deref(), Some("근거1"));
    assert_eq!(v.repaired, vec!["근거1 → 근거2"]);
    assert!(v.citations_ok);
    assert!(v.citation_message.contains("바로잡았습니다"), "{}", v.citation_message);
    // 숫자도 **고쳐 붙인 근거** 안에서 찾는다
    assert_eq!(v.numbers_missing(), 0, "{:?}", v.numbers);
}

#[test]
fn 어디에도_없는_말은_고쳐_붙이지_않는다() {
    // 고쳐 붙이기가 "아무 근거나 갖다 붙이는 일" 이 되면 검증이 무너진다.
    let e = vec![
        ev("근거1", 11, "교직원 생일기념 경비 1인당 3만원이하"),
        ev("근거2", 12, "경조사비는 1인당 50,000원 이내"),
    ];
    let d = draft(
        r#"{"answer":"명절 휴가비는 1인당 3만원 이하입니다.",
            "claims":[{"text":"명절 휴가비는 1인당 3만원 이하다","sources":["근거1"],"kind":"fact"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    assert!(v.repaired.is_empty(), "{:?}", v.repaired);
    assert!(v.nothing_supported(), "{:?}", v.claims);
}

#[test]
fn 해석은_뒷받침_검사에_넣지_않는다() {
    // 해석은 근거의 말을 옮기는 것이 아니므로 낱말이 겹치지 않는 것이 정상이다.
    // 해석까지 세면 규정을 읽어 주는 답이 모두 거부된다.
    let e = vec![ev("근거1", 11, "학교운영위원회 심의를 거쳐야 한다")];
    let d = draft(
        r#"{"answer":"심의를 거치면 가능합니다.",
            "claims":[{"text":"학교운영위원회 심의를 거쳐야 한다","sources":["근거1"],"kind":"fact"},
                      {"text":"따라서 가능하다고 볼 수 있다","sources":["근거1"],"kind":"interpretation"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    assert!(v.unsupported_claims().is_empty(), "{:?}", v.claims);
    assert!(!v.nothing_supported());
    assert!(v.has_interpretation, "해석은 대신 화면에 밝힌다");
}

#[test]
fn 문체_주장은_뒷받침을_묻지_않되_숫자는_본다() {
    // 문서 작성(P7): 인사말·마무리는 근거가 없어도 되는 말이다. 그러나 그 안에 숫자를
    // 적으면 그 숫자는 여전히 인용 근거에 있어야 한다 — 문장 종류로 검사를 피할 수 없다.
    let e = vec![ev("근거1", 11, "신청 기간은 3월 2일부터 3월 31일까지")];
    let d = draft(
        r#"{"answer":"학부모님 안녕하십니까. 신청 기간은 3월 31일까지입니다. 기한은 4월 5일까지 넉넉합니다.",
            "claims":[{"text":"학부모님 안녕하십니까","sources":[],"kind":"style"},
                      {"text":"신청 기간은 3월 31일까지다","sources":["근거1"],"kind":"fact"},
                      {"text":"기한은 4월 5일까지 넉넉합니다","sources":[],"kind":"style"}],
            "insufficientEvidence":false}"#,
    );
    let v = verify(&d, &e);
    // 문체 주장은 뒷받침되지 않은 주장으로 세지 않는다
    assert!(v.unsupported_claims().is_empty(), "{:?}", v.claims);
    assert!(!v.nothing_supported());
    // 그러나 문체 문장에 숨긴 `4월 5일` 은 근거에 없다고 잡힌다
    // (날짜는 `4월`·`5일` 처럼 조각으로 뽑힌다)
    let missing: Vec<&str> = v.numbers.iter().filter(|n| !n.found).map(|n| n.raw.as_str()).collect();
    assert!(missing.contains(&"4월") && missing.contains(&"5일"), "{:?}", v.numbers);
    assert!(v.numbers.iter().any(|n| n.found && n.raw == "31일"), "{:?}", v.numbers);
}

#[test]
fn 주장에_적히지_않은_본문_문장을_찾는다() {
    // ★ 문서 작성에서 실제로 본 고장 — claims 에는 근거 문장을 베껴 넣고(검사 통과),
    // 본문에는 근거에 없는 축제 날짜와 장소를 지어 썼다.
    let claims = vec![
        crate::answer::parse::Claim {
            text: "[학교명]입니다".into(),
            sources: vec![],
            kind: ClaimKind::Style,
        },
        crate::answer::parse::Claim {
            text: "단위학교는 특정 종교교육과 관련이 있는 방과후학교 프로그램을 편성·운영할 수 없다".into(),
            sources: vec!["근거3".into()],
            kind: ClaimKind::Fact,
        },
    ];
    let body = "[학교명]입니다. 학교 축제는 10월 15일(월)에 학교 운동장에서 진행됩니다. 많은 참여 부탁드립니다.";
    let cov = uncovered_sentences(body, &claims);
    let u = &cov.uncovered;
    assert_eq!(cov.total, 3);
    assert!(!cov.nothing_covered(), "인사말은 주장에 있으므로 전부 비어 있지는 않다");
    assert!(u.iter().any(|s| s.contains("축제")), "{u:?}");
    assert!(u.iter().any(|s| s.contains("참여")), "{u:?}");
    assert!(!u.iter().any(|s| s.contains("학교명")), "주장에 있는 문장은 걸리지 않는다: {u:?}");
}

#[test]
fn 주장을_옮겨_쓴_본문은_걸리지_않는다() {
    let claims = vec![crate::answer::parse::Claim {
        text: "자유수강권은 소득을 기준으로 저소득층 학생을 우선 지원한다".into(),
        sources: vec!["근거1".into()],
        kind: ClaimKind::Fact,
    }];
    let body = "자유수강권은 소득을 기준으로 저소득층 학생을 우선 지원합니다.";
    assert!(uncovered_sentences(body, &claims).uncovered.is_empty());
}

#[test]
fn 본문이_전부_주장에_없으면_근거_없는_초안이다() {
    // 실제로 본 판 — 주장 6개는 근거 문장을 베껴 넣었고, 본문 네 문장은 하나도 거기 없었다
    let claims = vec![crate::answer::parse::Claim {
        text: "자유수강권은 소득을 기준으로 저소득층 학생을 우선 지원한다".into(),
        sources: vec!["근거1".into()],
        kind: ClaimKind::Fact,
    }];
    let body = "신청 기간은 2025년 2월 1일부터 2월 28일까지입니다. 자세한 사항은 학교 홈페이지를 참조해 주세요.";
    let cov = uncovered_sentences(body, &claims);
    assert_eq!(cov.total, 2);
    assert!(cov.nothing_covered(), "{cov:?}");
}
