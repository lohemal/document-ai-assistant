//! 초점 낱말 시험. 골든 셋에서 실제로 본 물음들이다 — 여기서 틀리면 거부가 틀린다.

use super::*;

fn names(question: &str) -> Vec<String> {
    concepts(question).into_iter().map(|w| w.raw).collect()
}

fn last(question: &str) -> String {
    focus(question).map(|w| w.raw).unwrap_or_default()
}

#[test]
fn 물음_끝의_개념_낱말이_초점이다() {
    assert_eq!(last("정보통신윤리교육 의무 시수를 채우지 않으면 어떤 제재를 받아?"), "제재를");
    assert_eq!(last("미디어 이용습관 진단조사는 문항이 몇 개야?"), "문항이");
    assert_eq!(last("정보통신윤리교육 담당 교사에게 주는 수당은 얼마야?"), "수당은");
    assert_eq!(last("예산을 다음 연도로 이월할 때 이월 상한 비율은 몇 %야?"), "비율은");
}

#[test]
fn 숫자_물음말_꾸밈말은_개념이_아니다() {
    // `1인당`·`얼마까지`·`있어` 가 초점이 되면 거의 모든 물음이 거부된다
    assert_eq!(
        names("교직원 명절 휴가비는 1인당 얼마까지 집행할 수 있어?"),
        vec!["교직원", "명절", "휴가비는", "집행"]
    );
    assert_eq!(names("일비는 하루에 20,000원인가요?"), vec!["일비는", "하루에"]);
    assert_eq!(names("한 학교가 신청할 수 있는 예방교육 학급 수는 최대 몇 개야?"), vec!["학교가", "신청", "예방교육", "학급", "최대"]);
}

#[test]
fn 하다_되다_활용은_앞의_명사를_살린다() {
    // `폐강` 이 바로 묻는 것이다. 버리면 이 물음의 초점이 `수강생` 이 된다.
    assert_eq!(last("방과후학교 프로그램은 수강생이 몇 명 미만이면 폐강해야 해?"), "폐강");
    assert_eq!(last("수강료를 쓰고 남은 돈은 어떻게 처리해야 해?"), "처리");
    assert_eq!(last("방과후학교 수강료는 무엇으로 구성되는가?"), "구성");
}

#[test]
fn 두_글자_명사의_한을_어미로_보지_않는다() {
    // `상한`·`기한`·`위반` — `한` 을 하다 활용으로 잘라 먹으면 낱말이 사라진다
    assert!(names("이월 상한 비율은?").contains(&"상한".to_string()));
    assert_eq!(last("신청 기한은?"), "기한은");
}

#[test]
fn 토박이_동사_활용은_뺀다() {
    assert!(!names("강사가 학생한테 수강료를 직접 받아도 돼?").iter().any(|w| w.contains("받아")));
    assert!(!names("작년과 비교해서 예방교육 대상이 어떻게 바뀌었어?").iter().any(|w| w.contains("바뀌")));
    assert_eq!(last("초등학교 저학년한테 영어 방과후 수업을 해도 선행학습 금지에 안 걸려?"), "금지에");
}

#[test]
fn 바꿔쓴_앞말은_초점이_아니다() {
    // ★ 요구사항 5 — 원문에 `형편이 어려운` 이 없다고 거부하면 안 된다.
    // 앞쪽 낱말은 초점이 아니므로 자료집에 없어도 아무 일도 일어나지 않는다.
    let q = "형편이 어려운 아이들 방과후 수업비를 대신 내주는 제도는 어떻게 운영해?";
    assert_eq!(last(q), "운영");
    let c = FocusCheck::before_answer(q, |w| w.raw == "운영", &["자유수강권 운영 지침".to_string()]);
    assert_eq!(c.refusal(), None, "{c:?}");
}

#[test]
fn 자료집에_없으면_답하지_않는다_사유_a() {
    let c = FocusCheck::before_answer(
        "정보통신윤리교육 의무 시수를 채우지 않으면 어떤 제재를 받아?",
        |_| false,
        &["유치원 연 1회, 초·중·고 연 2회".to_string()],
    );
    assert!(!c.in_collection);
    let why = c.refusal().unwrap();
    assert!(why.contains("자료집에는") && why.contains("제재"), "{why}");
}

#[test]
fn 자료집엔_있는데_근거에_없으면_다른_사유로_답하지_않는다_사유_b() {
    let c = FocusCheck::before_answer(
        "예산을 다음 연도로 이월할 때 이월 상한 비율은 몇 %야?",
        |_| true,
        &["명시이월·사고이월·계속비이월".to_string()],
    );
    assert!(c.in_collection && !c.in_evidence);
    let why = c.refusal().unwrap();
    assert!(why.contains("검색이 놓쳤을") && why.contains("비율"), "{why}");
}

#[test]
fn 근거엔_있는데_인용_청크에_없으면_경고만_한다_사유_c() {
    let q = "교직원 명절 휴가비는 1인당 얼마까지 집행할 수 있어?";
    // 초점은 `집행` 이지만 근거·자료집에 다 있다고 치자 → 거부 아님
    let mut c = FocusCheck::before_answer(q, |_| true, &["예산 집행 기준".to_string()]);
    assert_eq!(c.refusal(), None);
    c.after_answer(q, &["생일기념 경비 1인당 3만원".to_string()]);
    assert_eq!(c.in_cited, Some(false));
    assert!(c.warning().unwrap().contains("인용한 근거에는 없습니다"), "{c:?}");
    // 거부 사유는 여전히 없다 — C 는 경고다
    assert_eq!(c.refusal(), None);
}

#[test]
fn 어미가_달라도_찾는다() {
    // 근거의 `이내로` 를 물음은 `이내에서` 로 쓴다 — 꼬리를 떼고 찾는다
    let w = Word { raw: "이내에서".into(), forms: vec!["이내에서".into(), "이내".into()] };
    assert!(present(&w, &joined(&["강사료의 5%이내로 정할 수 있다".to_string()])));
    // SQL 에 넘길 조각도 같은 꼴을 만든다
    let n = needles(&w);
    assert!(n.contains(&"이내".to_string()), "{n:?}");
}

#[test]
fn 개념을_못_고르면_아무_판단도_하지_않는다() {
    let c = FocusCheck::before_answer("얼마야?", |_| false, &[]);
    assert_eq!(c.word, None);
    assert_eq!(c.refusal(), None);
    assert_eq!(c.warning(), None);
}
