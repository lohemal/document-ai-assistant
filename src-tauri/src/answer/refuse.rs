//! 근거가 모자라면 **답하지 않는다.**
//!
//! P4b 에서 코사인 점수 하나로는 "자료에 없다" 를 가릴 수 없다는 것이 수치로
//! 나왔다 — 정답 청크의 점수 분포와 오답 1등의 분포가 거의 그대로 겹쳤다
//! (0.505~0.792 대 0.466~0.768). 그래서 **점수로 자르지 않는다.**
//!
//! 대신 서로 다른 신호를 함께 본다. 하나하나는 약하지만, 겹치면 분명해진다.
//!
//! | 신호 | 뜻 | 판단 |
//! |---|---|---|
//! | 근거가 아예 없다 | 검색이 아무것도 못 찾았다 | 거부 |
//! | 답이 비어 있다 | 형식만 채우고 내용이 없다 | 거부 |
//! | 인용이 하나도 없다 | 근거에서 나온 답이 아니다 | 거부 |
//! | 인용을 다 지어냈다 | 넘기지 않은 근거 이름을 썼다 | 거부 |
//! | **주장이 하나도 뒷받침되지 않는다** | 있는 근거를 인용해 놓고 그 근거에 없는 말을 했다 | 거부 |
//! | 모델이 없다고 했다 | `insufficientEvidence` — 다른 것이 멀쩡하면 표시만 | 거부 또는 제한적 |
//! | 일부 주장에 인용이 없다 | 반쯤 지어냈을 수 있다 | 제한적 |
//! | 일부 주장이 뒷받침되지 않는다 | 근거에 없는 말이 섞였다 | 제한적 |
//! | 숫자를 근거에서 못 찾았다 | 값을 바꿔 썼을 수 있다 | 제한적 |
//!
//! 여기에 **"물음의 핵심어가 인용 근거에 없다"** 를 넣으려 했다. 재 보고 뺐다 —
//! 지어낸 답 하나를 막는 값이 맞는 답 둘이었다 (`question_gap` 에 수치를 적어 두었다).
//!
//! **"주장이 뒷받침되는가" 가 자료에 없는 것을 물었을 때의 마지막 방어선이다.**
//! 인용이 실재하는 것과 그 근거에 그 내용이 있는 것은 다른 이야기다. 자료에
//! 없는 것을 물으면 모델은 있는 근거를 인용해 놓고 그 근거에 없는 말을 한다 —
//! "명절 휴가비" 를 물으면 생일기념 3만원 근거를 인용해 명절 휴가비가 3만원
//! 이라고 답한다. 인용도 실재하고 숫자도 근거에 있으므로, 그 둘로는 못 잡는다.
//!
//! **거부율만 높은 것은 좋은 것이 아니다.** 답할 수 있는 물음을 거부하면
//! 사용자는 프로그램을 믿지 않게 된다. 그래서 신호를 셋으로 나눈다 —
//! 답한다 / 제한을 달아 답한다 / 답하지 않는다.


use super::parse::Draft;
use super::verify::{self, Verdict};
use crate::domain::query;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// 근거로 답했다
    Answer,
    /// 답하되 확인되지 않은 데가 있다
    Limited,
    /// 답할 근거를 찾지 못했다
    Refuse,
    /// 답변 모델이 없거나 AI 가 꺼져 있다 — **검색과 근거는 그대로 보여 준다**
    NoModel,
}

/// 근거를 찾지 못했을 때 하는 말. **사용자가 정해 준 문구다.**
pub const REFUSAL: &str = "등록된 자료에서는 이 질문에 답할 수 있는 근거를 충분히 찾지 못했습니다.";
/// 그래도 관련이 있어 보이는 것이 있을 때 덧붙이는 말
pub const RELATED: &str = "다만 다음 내용은 관련이 있습니다.";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Judgement {
    pub decision: Decision,
    /// 왜 그렇게 판단했는지 (화면과 기록에 그대로 쓴다)
    pub reasons: Vec<String>,
}

/// 물음의 핵심어가 인용한 근거에 얼마나 있는가 (`question_gap` 참고).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyWords {
    /// 물음에서 뽑은 핵심어 수
    pub total: usize,
    /// 그 가운데 인용 근거에 없는 것
    pub missing: Vec<String>,
}

impl KeyWords {
    /// 인용 근거가 물음의 낱말을 얼마나 덮고 있는가 (0~1). 핵심어가 없으면 1.
    pub fn coverage(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        (self.total - self.missing.len().min(self.total)) as f64 / self.total as f64
    }
}

/// **물음의 핵심어 가운데 인용한 근거에 없는 낱말.**
///
/// 사용자가 정해 준 신호 가운데 "핵심어가 겹치지 않음" 이다. 자료에 없는 것을
/// 물었을 때 모델이 하는 마지막 짓을 이것으로 본다 — **근거를 제대로 옮겨
/// 적었는데 그 근거가 물음에 대한 답이 아닌 경우다.**
///
///     물음:  교직원 **명절 휴가비**는 1인당 얼마까지 집행할 수 있어?
///     근거:  소속 교직원의 생일시 소액(1인당 3만원 이하)의 상품권
///     답:    1인당 3만원 이하        ← 인용도 실재하고 그 근거에 있는 말이다
///
/// 인용 검사·숫자 검사·주장 뒷받침 검사가 모두 통과한다. 물음의 `명절`,
/// `휴가비` 가 근거에 없다는 것만이 남는 단서다.
///
/// 물음말(`얼마까지`)과 조사는 `domain::query` 가 걸러 준다 — 낱말 검색과
/// 같은 자를 쓴다.
///
/// ## 재 보고 판단에는 쓰지 않기로 했다 ★
///
/// 덮임(핵심어 가운데 근거에 있는 것의 비율)으로 자를 때 잘못 거부 /
/// 지어내 답함이 이렇게 움직였다 (골든 셋 52+15문항, gemma3:4b):
///
///     자르지 않음   11.5% / 53.3%
///     < 0.26        13.5% / 46.7%   ← 1건 잡고 1건 잃는다
///     < 0.34        23.1% / 46.7%
///     < 0.4         25.0% / 33.3%   ← 3건 잡고 7건 잃는다
///     < 0.5         32.7% / 26.7%
///
/// 지어낸 답 하나를 막는 값이 맞는 답 둘이다. **거부에는 쓸 수 없다.**
///
/// 화면 경고로도 쓸 수 없었다. 답한 46건 가운데 37건에 걸렸는데, 걸린 낱말이
/// 대부분 뜻 없는 것이었다 — 어미가 다른 것(`써도`·`주나요`·`받을`), 숫자를
/// 달리 적은 것(`30000원에서` 대 근거의 `30,000원`), 바꿔 쓴 말(`형편이`·
/// `어려운`). 거의 모든 답에 붙는 경고는 아무 뜻이 없다.
///
/// 낱말 대신 **이 자료집 어디에도 없는 낱말**을 보는 쪽이 맞을 것 같다
/// (`휴가비`·`제재`·`문항` 은 자료 전체에 없다). 그것은 낱말 검색 층에
/// 손이 필요해서 다음으로 넘긴다. 이 함수는 그 판단을 다시 재는 데 쓴다
/// (`eval::answer`).
pub fn question_gap(question: &str, texts: &[String]) -> KeyWords {
    if texts.is_empty() {
        return KeyWords::default();
    }
    let joined: String = texts
        .iter()
        .map(|t| t.replace(char::is_whitespace, ""))
        .collect::<Vec<_>>()
        .join(" ");

    let p = query::parse(question);
    let mut seen: Vec<String> = Vec::new();
    let mut out = KeyWords::default();
    let mut check = |raw: &str, forms: &[String]| {
        if query::is_question_word(raw) || seen.iter().any(|g| g == raw) {
            return;
        }
        seen.push(raw.to_string());
        out.total += 1;
        if !forms.iter().any(|f| verify::appears(&joined, f)) {
            out.missing.push(raw.to_string());
        }
    };
    for t in &p.terms {
        check(&t.raw, &t.forms);
    }
    for s in &p.short {
        check(s, std::slice::from_ref(s));
    }
    out
}

/// `evidence_count` 는 LLM 에게 넘긴 근거 수, `hit_count` 는 검색이 찾은 수.
pub fn decide(
    draft: &Draft,
    verdict: &Verdict,
    evidence_count: usize,
    hit_count: usize,
) -> Judgement {
    let mut reasons: Vec<String> = Vec::new();

    // ── 답하지 않아야 하는 경우 ──────────────────────────────────────
    if evidence_count == 0 {
        reasons.push(if hit_count == 0 {
            "검색에서 관련 있는 자료를 찾지 못했습니다.".to_string()
        } else {
            "근거로 쓸 만한 자료를 고르지 못했습니다.".to_string()
        });
        return Judgement { decision: Decision::Refuse, reasons };
    }
    if draft.answer.trim().is_empty() {
        reasons.push("AI 가 답을 내놓지 못했습니다.".to_string());
        return Judgement { decision: Decision::Refuse, reasons };
    }
    let cited = draft.cited();

    // 모델이 "근거가 부족하다" 고 표시했을 때.
    //
    // **그 표시만으로 바로 거부하지는 않는다.** 작은 모델은 답을 제대로 쓰고
    // 인용까지 붙여 놓고도 이 값을 true 로 두는 일이 잦다(gemma3:4b 에서 실제로
    // 그랬다). 그때 거부해 버리면 **맞는 답을 버리게 된다.**
    //
    // 그래서 표시와 실제를 함께 본다 — 답이 있고, 인용이 있고, 그 인용이 실재하면
    // 답을 보여 주되 **모델이 스스로 못 미더워했다는 사실을 함께 적는다.**
    // 그 밖에는 표시를 그대로 따른다.
    if draft.insufficient_evidence {
        let substantive =
            !cited.is_empty() && verdict.unknown_sources.is_empty();
        if !substantive {
            reasons.push("AI 가 근거만으로는 답할 수 없다고 판단했습니다.".to_string());
            return Judgement { decision: Decision::Refuse, reasons };
        }
        reasons.push(
            "AI 가 근거가 넉넉하지 않다고 표시했습니다. 답과 근거를 직접 확인해 주세요."
                .to_string(),
        );
    }
    if cited.is_empty() {
        reasons.push("답변에 인용된 근거가 하나도 없습니다.".to_string());
        return Judgement { decision: Decision::Refuse, reasons };
    }
    if verdict.unknown_sources.len() == cited.len() {
        reasons.push(format!(
            "답변이 든 근거({})가 자료에 없습니다.",
            verdict.unknown_sources.join(", ")
        ));
        return Judgement { decision: Decision::Refuse, reasons };
    }

    // 인용은 실재하지만 **그 근거에 그 말이 없는** 경우.
    // 자료에 없는 것을 물었을 때 모델이 하는 짓이다 — 있는 근거를 인용해 놓고
    // 그 근거에 없는 말을 한다. 하나도 뒷받침되지 않으면 답이라고 볼 수 없다.
    if verdict.nothing_supported() {
        reasons.push(
            "답변의 주장이 인용한 근거에서 확인되지 않습니다. 근거에 없는 내용일 수 있습니다."
                .to_string(),
        );
        return Judgement { decision: Decision::Refuse, reasons };
    }

    // ── 답하되 제한을 다는 경우 ──────────────────────────────────────
    if !verdict.unknown_sources.is_empty() {
        reasons.push(format!(
            "일부 인용({})이 자료에 없습니다.",
            verdict.unknown_sources.join(", ")
        ));
    }
    if !verdict.claims_without_source.is_empty() {
        reasons.push(format!(
            "인용이 붙지 않은 주장이 {}개 있습니다.",
            verdict.claims_without_source.len()
        ));
    }
    let weak = verdict.unsupported_claims();
    if !weak.is_empty() {
        reasons.push(format!(
            "주장 {}개가 인용한 근거에서 확인되지 않습니다.",
            weak.len()
        ));
    }
    if verdict.numbers_missing() > 0 {
        reasons.push(format!(
            "숫자 {}개를 인용 근거에서 확인하지 못했습니다.",
            verdict.numbers_missing()
        ));
    }

    // 여기에 "물음의 핵심어가 인용 근거에 없다" 를 넣으려 했다. **재 보고 뺐다.**
    // 까닭은 `question_gap` 에 적어 두었다.

    if reasons.is_empty() {
        Judgement { decision: Decision::Answer, reasons }
    } else {
        Judgement { decision: Decision::Limited, reasons }
    }
}

#[cfg(test)]
#[path = "refuse_tests.rs"]
mod tests;
