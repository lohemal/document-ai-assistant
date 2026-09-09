//! 모델 답 읽기 시험.
//!
//! 여기 든 어긋난 꼴들은 **작은 모델이 실제로 하는 짓**이다.

use super::*;

const GOOD: &str = r#"{
  "answer": "저소득층 학생을 우선 지원합니다.",
  "claims": [
    {"text": "저소득층 학생을 우선 지원한다", "sources": ["근거1"], "kind": "fact"}
  ],
  "insufficientEvidence": false,
  "confidence": "high",
  "interpretationNotes": []
}"#;

#[test]
fn 제대로_온_것을_읽는다() {
    let d = parse(GOOD).unwrap();
    assert_eq!(d.answer, "저소득층 학생을 우선 지원합니다.");
    assert_eq!(d.claims.len(), 1);
    assert_eq!(d.claims[0].sources, vec!["근거1"]);
    assert_eq!(d.claims[0].kind, ClaimKind::Fact);
    assert!(!d.insufficient_evidence);
    assert!(!d.has_interpretation());
}

#[test]
fn 울타리를_걷어_낸다() {
    let raw = format!("```json\n{GOOD}\n```");
    assert_eq!(parse(&raw).unwrap().claims.len(), 1);
}

#[test]
fn 앞뒤에_말을_붙여도_읽는다() {
    let raw = format!("알겠습니다. 아래와 같습니다:\n{GOOD}\n도움이 되었으면 좋겠습니다.");
    assert_eq!(parse(&raw).unwrap().answer, "저소득층 학생을 우선 지원합니다.");
}

#[test]
fn 답_안에_중괄호가_있어도_끝을_찾는다() {
    let raw = r#"{"answer": "이런 표기 {가} 있어도", "claims": [], "insufficientEvidence": false}"#;
    assert_eq!(parse(raw).unwrap().answer, "이런 표기 {가} 있어도");
}

#[test]
fn 없는_자리는_기본값으로_둔다() {
    // claims 나 confidence 를 빼먹는 일이 잦다
    let raw = r#"{"answer": "짧은 답"}"#;
    let d = parse(raw).unwrap();
    assert_eq!(d.answer, "짧은 답");
    assert!(d.claims.is_empty());
    assert!(!d.insufficient_evidence);
    assert!(d.confidence.is_none());
}

#[test]
fn 답이_아예_없으면_실패다() {
    let e = parse(r#"{"claims": []}"#).unwrap_err();
    assert!(e.contains("읽지 못했습니다"), "{e}");
}

#[test]
fn json_이_아니면_실패다() {
    let e = parse("근거1에 따르면 저소득층 학생입니다.").unwrap_err();
    assert!(e.contains("정해진 형식"), "{e}");
    // 받은 글을 함께 보여 줘야 무엇이 잘못됐는지 알 수 있다
    assert!(e.contains("근거1에 따르면"), "{e}");
}

#[test]
fn 근거가_없다고_말한_것을_알아본다() {
    let raw = r#"{"answer": "", "claims": [], "insufficientEvidence": true}"#;
    let d = parse(raw).unwrap();
    assert!(d.insufficient_evidence);
    assert!(d.answer.is_empty());
}

#[test]
fn 해석이_섞인_것을_알아본다() {
    let raw = r#"{"answer": "가능해 보입니다.",
      "claims": [
        {"text": "심의를 거쳐야 한다", "sources": ["근거1"], "kind": "fact"},
        {"text": "이 경우 가능하다고 볼 수 있다", "sources": ["근거1"], "kind": "interpretation"}
      ],
      "insufficientEvidence": false}"#;
    let d = parse(raw).unwrap();
    assert!(d.has_interpretation());
    assert_eq!(d.claims[1].kind, ClaimKind::Interpretation);
}

#[test]
fn 인용을_겹치지_않게_모은다() {
    let raw = r#"{"answer": "a",
      "claims": [
        {"text": "x", "sources": ["근거1", "근거2"], "kind": "fact"},
        {"text": "y", "sources": ["근거2", " 근거3 "], "kind": "fact"}
      ],
      "insufficientEvidence": false}"#;
    let d = parse(raw).unwrap();
    assert_eq!(d.cited(), vec!["근거1", "근거2", "근거3"]);
}

#[test]
fn 답에서_숫자를_뽑는다() {
    let raw = r#"{"answer": "1인당 50,000원 이내입니다.", "claims": [], "insufficientEvidence": false}"#;
    let d = parse(raw).unwrap();
    let nums = d.numbers();
    assert!(nums.iter().any(|n| n.value == 50000.0), "{nums:?}");
}

#[test]
fn 모르는_자리가_와도_버티다() {
    // 모델이 제멋대로 자리를 더 붙일 때가 있다
    let raw = r#"{"answer": "a", "claims": [], "insufficientEvidence": false, "따로붙인것": 1}"#;
    assert!(parse(raw).is_ok());
}

#[test]
fn 길어서_잘린_답을_알아본다() {
    // ★ 실제로 나온 고장이다. 모델이 길게 쓰다 토큰 한도에 걸려 JSON 이
    // 닫히지 않았다. 그때 "형식이 아니다" 라고만 하면 사용자가 할 일을 모른다.
    let cut = r#"{ "answer": "근거 6, 7, 8을 종합하여 답변합니다. 명절 휴가비는"#;
    let e = parse(cut).unwrap_err();
    assert!(e.contains("잘렸습니다"), "{e}");
    assert!(e.contains("좁혀"), "무엇을 하면 되는지 적혀 있지 않습니다: {e}");
}

#[test]
fn 답_자리에_자리_이름을_적어_놓으면_빈_답으로_본다() {
    // ★ gemma3:4b 에서 실제로 나온 답이다. 그대로 두면 화면에 답변으로
    // `insufficientEvidence` 라는 글자가 뜬다.
    let raw = r#"{"answer":"insufficientEvidence",
      "claims":[{"text":"학교가 달리 정할 수 있다","sources":["근거1"],"kind":"fact"}],
      "insufficientEvidence":true}"#;
    let d = parse(raw).unwrap();
    assert!(d.answer.is_empty(), "답이 비워지지 않았습니다: {:?}", d.answer);
    assert!(d.insufficient_evidence);
    // 주장은 그대로 남는다 — 모델이 무엇을 보고 그랬는지는 남겨 둔다
    assert_eq!(d.claims.len(), 1);
}

#[test]
fn 사람_말로_된_답은_그대로_둔다() {
    // 위 규칙이 멀쩡한 답을 지우면 안 된다
    let raw = r#"{"answer":"근거가 없습니다.","claims":[],"insufficientEvidence":true}"#;
    assert_eq!(parse(raw).unwrap().answer, "근거가 없습니다.");
}
