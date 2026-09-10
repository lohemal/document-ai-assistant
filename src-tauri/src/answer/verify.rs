//! 답변이 **정말 그 근거에서 나왔는지** 본다.
//!
//! 두 가지를 본다.
//!
//! ① 인용 — 모델이 적은 근거 이름이 정말 우리가 넘긴 근거인가. 없는 이름을
//!    적었다면 그 답은 근거에서 나온 것이 아니다.
//! ② 숫자 — 답에 든 금액·기한·횟수가 **인용된 근거 안에** 있는가.
//!    검색 결과 전체가 아니라 인용된 것만 본다 (요구사항 9).
//!
//! ## 이 검증이 보장하지 못하는 것 ★
//!
//! 숫자가 근거에 있다는 것은 **그 숫자가 그 자리에 쓰였다는 뜻이 아니다.**
//!
//!     근거:  A 지원금 250,000원 / B 지원금 500,000원
//!     답변:  "A 지원금은 500,000원입니다"   ← 숫자는 근거에 있다. 답은 틀렸다.
//!
//! 그래서 화면에 **"답변 검증 완료" 라고 쓰지 않는다.** 쓸 수 있는 말은
//! "답변에 사용된 주요 숫자가 인용 근거에서 확인되었습니다" 까지다.
//! 통과는 신뢰의 근거로 쓰지 않고, **실패만 경고로** 쓴다 (설계안 5-5).
//!
//! 주장마다 인용을 묶어 두었으므로(`claims[].sources`), 숫자도 **그 주장이
//! 인용한 근거 안에서** 찾는다. 답 전체를 한 덩어리로 보는 것보다 좁다.
//! 여기서 한 걸음 더 나아가려면 주장의 주어(A·B)까지 봐야 하는데, 그것은
//! v0.2 의 문맥 창 대조로 넘긴다.

use super::context::Evidence;
use super::numbers::{self, Num};
use super::parse::{ClaimKind, Draft};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCheck {
    pub source_id: String,
    /// 우리가 넘긴 근거인가
    pub known: bool,
    pub chunk_id: Option<i64>,
    pub document_id: Option<i64>,
    pub page: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberCheck {
    /// 답에 적혀 있던 그대로
    pub raw: String,
    /// 금액·날짜·횟수…
    pub kind: &'static str,
    /// 인용된 근거에서 같은 값을 찾았는가
    pub found: bool,
    /// 어느 근거에서 찾았는가
    pub source_id: Option<String>,
}

/// 주장 하나가 그 주장이 든 근거로 뒷받침되는가.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimCheck {
    pub text: String,
    pub sources: Vec<String>,
    /// 사실인가 해석인가 — 해석에는 뒷받침 검사를 걸지 않는다 (아래 참고)
    pub kind: ClaimKind,
    /// 주장의 낱말 가운데 인용한 근거에도 있는 것의 비율 (0~1)
    pub overlap: f64,
    /// 뒷받침된다고 볼 만한가
    pub supported: bool,
    /// 인용을 고쳐 붙였으면, 모델이 원래 적었던 이름
    pub repaired_from: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub claims: Vec<ClaimCheck>,
    pub sources: Vec<SourceCheck>,
    /// 우리가 넘기지 않은 근거 이름 (지어낸 인용)
    pub unknown_sources: Vec<String>,
    /// 인용이 하나도 없는 주장
    pub claims_without_source: Vec<String>,
    pub numbers: Vec<NumberCheck>,
    /// 인용이 온전한가 — 하나라도 지어냈으면 false
    pub citations_ok: bool,
    /// 내용이 실제로 있는 근거로 바로잡은 인용 ("근거3 → 근거5")
    pub repaired: Vec<String>,
    /// 답에 해석이 섞여 있는가
    pub has_interpretation: bool,
    /// 화면에 그대로 쓸 말
    pub citation_message: String,
    pub number_message: String,
}

impl Verdict {
    /// 근거로 뒷받침되지 않는 주장.
    ///
    /// **해석은 세지 않는다.** 해석은 근거의 말을 옮기는 것이 아니라 근거를
    /// 읽어 낸 말이므로("심의를 거쳐야 한다" → "따라서 가능하다고 볼 수 있다")
    /// 낱말이 겹치지 않는 것이 정상이다. 해석까지 세면 규정을 읽어 주는
    /// 답 — 이 프로그램이 하려는 일 가운데 하나 — 이 거부된다.
    /// 해석은 대신 화면에 "자료 해석이 포함된 답변입니다" 로 밝힌다.
    pub fn unsupported_claims(&self) -> Vec<&ClaimCheck> {
        self.claims
            .iter()
            .filter(|c| !c.supported && c.kind == ClaimKind::Fact)
            .collect()
    }

    /// 사실 주장이 있는데 **하나도** 뒷받침되지 않는가
    pub fn nothing_supported(&self) -> bool {
        let facts: Vec<&ClaimCheck> = self
            .claims
            .iter()
            .filter(|c| c.kind == ClaimKind::Fact)
            .collect();
        !facts.is_empty() && facts.iter().all(|c| !c.supported)
    }

    pub fn numbers_missing(&self) -> usize {
        self.numbers.iter().filter(|n| !n.found).count()
    }
    pub fn numbers_found(&self) -> usize {
        self.numbers.iter().filter(|n| n.found).count()
    }
}

/// 근거 이름을 견줄 꼴로 다듬는다.
///
/// **모델은 이름을 조금씩 다르게 쓴다.** 실제로 본 것들:
///
///     근거6   근거 6   [근거6]   [근거 6]   `근거6`
///
/// 글자를 그대로 견주면 `근거 6` 이 없는 근거로 잡혀 **답을 지어낸 것으로
/// 잘못 판단하고 거부한다.** 실제로 그렇게 거부한 일이 있었다. 빈칸과
/// 괄호를 지우고 견준다.
fn bare_source(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && !"[]()`\"'".contains(*c))
        .collect()
}

/// 글에서 두 글자 이상인 낱말을 뽑는다. 조사가 붙어 있어도 그대로 둔다 —
/// 견줄 때 부분 일치로 보므로 `수강료는` 은 `수강료` 를 담은 글에서 찾아진다.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 2)
        .map(|w| w.to_string())
        .collect()
}

/// **인용한 근거가 그 주장을 실제로 담고 있는가.**
///
/// 인용이 실재하는 것과, 그 근거에 그 내용이 있는 것은 다른 이야기다.
/// 모델은 있는 근거를 인용해 놓고 그 근거에 없는 말을 할 수 있다 —
/// 자료에 없는 것을 물었을 때 특히 그렇다("명절 휴가비" 를 물으면
/// 생일기념 3만원 근거를 인용해 명절 휴가비가 3만원이라고 답한다).
///
/// 그래서 주장의 낱말이 그 근거에 얼마나 들어 있는지를 본다.
///
/// 기준을 0.7 로 둔 까닭 — 위의 명절 휴가비 답은 겹침이 **0.6** 이다.
/// 다섯 낱말(명절·휴가비는·1인당·3만원·이하다) 가운데 뒤의 세 낱말은
/// 근거에 있고, 정작 무엇에 대한 말인지를 가리는 앞의 두 낱말이 없다.
/// 절반(0.5)으로 두면 이 답이 통과한다. 실제로 통과했다.
///
/// **이 값을 그대로 거부에 쓰지는 않는다.** 바꿔 말하기를 잘한 답도 겹침이
/// 낮아질 수 있어서, 신호 하나로만 쓴다 (`refuse` 참고).
const SUPPORT_MIN: f64 = 0.7;

fn overlap(claim_text: &str, texts: &[String]) -> f64 {
    let ws = words(claim_text);
    if ws.is_empty() {
        return 1.0;
    }
    if texts.is_empty() {
        return 0.0;
    }
    let joined: String = texts
        .iter()
        .map(|t| t.replace(char::is_whitespace, ""))
        .collect::<Vec<_>>()
        .join(" ");

    let found = ws.iter().filter(|w| appears(&joined, w)).count();
    found as f64 / ws.len() as f64
}

/// 낱말 하나가 그 글에 있는가.
///
/// 꼬리를 한두 자 떼어 보고도 찾는다. 답은 근거를 그대로 옮기지 않고 어미를
/// 바꿔 쓰기 때문이다 — 근거의 `3만원이하` 를 답은 `3만원이다` 로 쓴다.
/// 떼어 보지 않으면 **맞는 답을 "근거에 없다" 고 잘못 판단한다.**
/// (낱말 검색의 조사 떼기와 같은 생각이다 — `domain::query::strip_particle`)
pub(crate) fn appears(joined: &str, w: &str) -> bool {
    if joined.contains(w) {
        return true;
    }
    let cs: Vec<char> = w.chars().collect();
    for cut in [1usize, 2] {
        if cs.len() > cut + 1 {
            let stem: String = cs[..cs.len() - cut].iter().collect();
            if joined.contains(&stem) {
                return true;
            }
        }
    }
    false
}

/// 주장에 딸린 근거들의 숫자를 모은다. 인용이 없으면 빈 목록.
fn nums_of<'a>(sources: &[String], evidence: &'a [Evidence]) -> Vec<(&'a Evidence, Vec<Num>)> {
    evidence
        .iter()
        .filter(|e| {
            sources
                .iter()
                .any(|s| bare_source(s) == bare_source(&e.source_id))
        })
        .map(|e| (e, numbers::extract(&e.text)))
        .collect()
}

/// 주장을 담고 있는 근거를 찾는다. 돌려주는 것은 (근거, 겹침).
fn best_match<'a>(claim: &str, evidence: &'a [Evidence]) -> Option<(&'a Evidence, f64)> {
    evidence
        .iter()
        .map(|e| (e, overlap(claim, std::slice::from_ref(&e.text))))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
}

pub fn verify(draft: &Draft, evidence: &[Evidence]) -> Verdict {
    // ── ① 주장마다, 그 주장이 든 근거가 그 말을 담고 있는가 ──────────
    //
    // 담고 있지 않으면 **다른 근거에서 찾아 인용을 고쳐 붙인다.**
    //
    // 작은 모델은 근거를 제대로 옮겨 적고도 번호를 잘못 붙인다. 실제로
    // gemma3:4b 는 `유치원: 연 1회` 를 그대로 옮겨 놓고 근거 번호를 다른 것으로
    // 적었다. 그때 거부해 버리면 **맞는 답을 버리고, 사용자에게는 아무것도
    // 남지 않는다.** 우리는 어느 근거에 그 말이 있는지 찾을 수 있으므로,
    // 찾아서 고쳐 붙이고 **고쳤다는 사실을 화면에 밝힌다.**
    //
    // 어디에도 없으면 고치지 않는다 — 그게 지어낸 말이다.
    let mut claim_checks: Vec<ClaimCheck> = Vec::new();
    let mut repaired: Vec<String> = Vec::new();

    for c in &draft.claims {
        let named: Vec<&Evidence> = evidence
            .iter()
            .filter(|e| {
                c.sources
                    .iter()
                    .any(|s| bare_source(s) == bare_source(&e.source_id))
            })
            .collect();
        let texts: Vec<String> = named.iter().map(|e| e.text.clone()).collect();
        let mine = overlap(&c.text, &texts);

        if mine >= SUPPORT_MIN {
            claim_checks.push(ClaimCheck {
                text: c.text.clone(),
                kind: c.kind,
                sources: named.iter().map(|e| e.source_id.clone()).collect(),
                overlap: mine,
                supported: true,
                repaired_from: None,
            });
            continue;
        }

        // 이 주장을 담고 있는 근거가 따로 있는가
        match best_match(&c.text, evidence) {
            Some((e, best)) if best >= SUPPORT_MIN => {
                repaired.push(format!(
                    "{} → {}",
                    if c.sources.is_empty() {
                        "인용 없음".to_string()
                    } else {
                        c.sources.join(", ")
                    },
                    e.source_id
                ));
                claim_checks.push(ClaimCheck {
                    text: c.text.clone(),
                    kind: c.kind,
                    sources: vec![e.source_id.clone()],
                    overlap: best,
                    supported: true,
                    repaired_from: Some(c.sources.join(", ")),
                });
            }
            _ => claim_checks.push(ClaimCheck {
                text: c.text.clone(),
                kind: c.kind,
                sources: named.iter().map(|e| e.source_id.clone()).collect(),
                overlap: mine,
                supported: false,
                repaired_from: None,
            }),
        }
    }

    // ── ② 인용 — 고친 것을 반영한 최종 근거 목록 ─────────────────────
    let mut effective: Vec<String> = Vec::new();
    for c in &claim_checks {
        for s in &c.sources {
            if !effective.contains(s) {
                effective.push(s.clone());
            }
        }
    }

    // 모델이 적었는데 자료에도 없고 고쳐 붙일 수도 없었던 이름 = 지어낸 인용
    let mut unknown: Vec<String> = Vec::new();
    for id in draft.cited() {
        let real = evidence
            .iter()
            .any(|e| bare_source(&e.source_id) == bare_source(&id));
        let used = effective.iter().any(|s| bare_source(s) == bare_source(&id));
        if !real && !used && !unknown.contains(&id) {
            unknown.push(id);
        }
    }

    let sources: Vec<SourceCheck> = effective
        .iter()
        .map(|id| {
            let e = evidence
                .iter()
                .find(|e| bare_source(&e.source_id) == bare_source(id));
            SourceCheck {
                source_id: id.clone(),
                known: e.is_some(),
                chunk_id: e.map(|e| e.chunk_id),
                document_id: e.map(|e| e.document_id),
                page: e.map(|e| e.page_start),
            }
        })
        .collect();

    // 문체(style) 주장은 근거가 없어도 되는 말이다 — 인사말에 인용이 없다고 탓하지 않는다
    let claims_without_source: Vec<String> = claim_checks
        .iter()
        .filter(|c| c.sources.is_empty() && c.kind != ClaimKind::Style)
        .map(|c| c.text.clone())
        .collect();

    let citations_ok = unknown.is_empty() && !sources.is_empty();

    // 지어낸 인용이 있으면 그 말을 가장 먼저 한다. 없는 이름을 적었는데
    // "인용된 근거가 없습니다" 라고만 하면 **무엇이 잘못됐는지가 가려진다.**
    let citation_message = if !unknown.is_empty() {
        format!(
            "답변이 자료에 없는 근거({})를 들었습니다. 이 답변은 믿을 수 없습니다.",
            unknown.join(", ")
        )
    } else if sources.is_empty() {
        "답변에 인용된 근거가 없습니다.".to_string()
    } else if !repaired.is_empty() {
        format!(
            "답변이 든 근거 {}개를 자료에서 확인했습니다. 다만 인용 번호 {}건은 \
             내용이 실제로 있는 근거로 바로잡았습니다 ({}).",
            sources.len(),
            repaired.len(),
            repaired.join(" · ")
        )
    } else if !claims_without_source.is_empty() {
        format!(
            "근거 {}개가 확인되었습니다. 다만 인용이 붙지 않은 주장이 {}개 있습니다.",
            sources.len(),
            claims_without_source.len()
        )
    } else {
        format!("답변이 든 근거 {}개가 모두 자료에서 확인되었습니다.", sources.len())
    };

    // ── ③ 숫자 — **고쳐 붙인 근거를 기준으로** 본다 ─────────────────
    let mut checks: Vec<NumberCheck> = Vec::new();
    let mut seen: Vec<(String, f64)> = Vec::new();

    let mut check_in = |num: &Num, pool: &[(&Evidence, Vec<Num>)], out: &mut Vec<NumberCheck>| {
        let key = (num.raw.clone(), num.value);
        if seen.contains(&key) {
            return;
        }
        seen.push(key);
        let hit = pool
            .iter()
            .find(|(_, nums)| numbers::contains(nums, num))
            .map(|(e, _)| e.source_id.clone());
        out.push(NumberCheck {
            raw: num.raw.clone(),
            kind: num.kind.label(),
            found: hit.is_some(),
            source_id: hit,
        });
    };

    if claim_checks.is_empty() {
        let pool = nums_of(&effective, evidence);
        for n in draft.numbers() {
            check_in(&n, &pool, &mut checks);
        }
    } else {
        for c in &claim_checks {
            let pool = nums_of(&c.sources, evidence);
            for n in numbers::extract(&c.text) {
                check_in(&n, &pool, &mut checks);
            }
        }
        // 주장에는 없고 답변 글에만 있는 숫자도 본다 (요약하며 더 쓴 것)
        let pool = nums_of(&effective, evidence);
        for n in draft.numbers() {
            check_in(&n, &pool, &mut checks);
        }
    }

    let missing = checks.iter().filter(|c| !c.found).count();
    let number_message = if checks.is_empty() {
        "답변에 확인할 숫자가 없습니다.".to_string()
    } else if missing == 0 {
        // **여기서 "검증 완료" 라고 쓰지 않는다.** 확인한 것은 값의 있음뿐이다.
        "답변에 사용된 주요 숫자가 인용 근거에서 확인되었습니다.".to_string()
    } else {
        let list: Vec<&str> = checks
            .iter()
            .filter(|c| !c.found)
            .map(|c| c.raw.as_str())
            .collect();
        format!(
            "답변의 숫자 가운데 {}개를 인용 근거에서 확인하지 못했습니다: {}. \
             원문을 직접 확인해 주세요.",
            missing,
            list.join(", ")
        )
    };

    Verdict {
        claims: claim_checks,
        sources,
        unknown_sources: unknown,
        claims_without_source,
        numbers: checks,
        citations_ok,
        repaired,
        has_interpretation: draft.has_interpretation()
            || draft.claims.iter().any(|c| c.kind == ClaimKind::Interpretation),
        citation_message,
        number_message,
    }
}

/// **본문 문장 가운데 어느 주장에도 적히지 않은 것** (문서 작성, P7).
///
/// 문서 초안에서 실제로 본 고장: 모델이 `claims` 에는 근거 문장을 그대로 베껴 넣고
/// (그래서 뒷받침 검사는 다 통과한다), `answer` 본문에는 근거에 없는 날짜와 장소를
/// 지어 썼다("학교 축제는 10월 15일(월)에 학교 운동장에서 진행됩니다"). 주장만 보면
/// 본문의 거짓이 보이지 않는다. 그래서 **본문의 문장마다 그 문장을 적은 주장이 있는지**
/// 본다 — 문체(style) 주장이든 사실 주장이든, 문장의 낱말이 절반 넘게 겹치는 주장이
/// 하나는 있어야 한다. 없으면 그 문장은 근거 없이 쓴 것이다.
///
/// 규정 해석(P5)의 짧은 답("네", "7인")에는 걸지 않는다 — 부르는 쪽이 문서 작성일 때만 쓴다.
pub fn uncovered_sentences(answer: &str, claims: &[super::parse::Claim]) -> Coverage {
    let claim_texts: Vec<String> = claims.iter().map(|c| c.text.clone()).collect();
    let sentences: Vec<String> = answer
        .split(|c: char| matches!(c, '.' | '!' | '?' | '\n'))
        .map(|s| s.trim().trim_end_matches(['다', '요']).to_string())
        .filter(|s| words(s).len() >= 2)
        .collect();
    let uncovered = sentences
        .iter()
        .filter(|s| {
            // 어느 주장과도 절반 넘게 겹치지 않으면 근거 없이 쓴 문장이다
            !claim_texts
                .iter()
                .any(|t| overlap(s, std::slice::from_ref(t)) >= SUPPORT_MIN)
        })
        .cloned()
        .collect();
    Coverage { total: sentences.len(), uncovered }
}

/// 본문 문장이 주장에 얼마나 적혔는가.
#[derive(Debug, Clone, Default)]
pub struct Coverage {
    /// 본문 문장 수 (두 낱말 이상인 것)
    pub total: usize,
    /// 어느 주장에도 적히지 않은 문장
    pub uncovered: Vec<String>,
}

impl Coverage {
    /// 본문이 있는데 **한 문장도** 주장에 적히지 않았다 — 근거 없이 쓴 초안이다
    pub fn nothing_covered(&self) -> bool {
        self.total > 0 && self.uncovered.len() == self.total
    }
}

#[cfg(test)]
#[path = "verify_tests.rs"]
mod tests;
