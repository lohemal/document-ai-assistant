//! 물음의 **초점 낱말** — 답의 성격을 정하는 낱말 — 이 어디에 있는지 본다 (P5b).
//!
//! P5 에서 지어내 답한 8건은 모두 같은 모습이었다. 근거를 제대로 옮겨 적고, 그것을
//! 물음에 대한 답으로 내놓는다. 물음이 "제재" 를 물었는데 근거는 "교육 횟수" 를
//! 말한다. 인용·숫자·주장 뒷받침 검사가 모두 통과한다.
//!
//! **비율은 보지 않는다.** (P5 의 `refuse::question_gap` 은 비율이었고, 그것으로
//! 자르면 맞는 답을 두 배로 잃었다.) 대신 물음 끝의 개념 낱말 **하나**가 어디에
//! 있는지를 세 범위에서 본다. 뜻이 다르므로 사유도 다르다.
//!
//!   A. 선택한 자료집 전체에 없다      → 자료에 없는 것을 물었다           → 답하지 않는다
//!   B. 자료집엔 있는데 넘긴 근거에 없다 → 검색이 못 가져왔다                → 답하지 않는다 (사유는 따로)
//!   C. 근거엔 있는데 인용한 청크에 없다 → 근거는 왔는데 답이 그것으로 답하지 않았다 → 답은 보이되 경고
//!
//! 초점은 **물음의 끝에서 고른다.** 한국어 물음은 묻는 것을 서술어 바로 앞에 둔다 —
//! "…어떤 **제재**를 받아?", "…**문항**이 몇 개야?", "…**수당**은 얼마야?". 앞쪽
//! 낱말(`형편이 어려운 아이들`)은 무엇에 대한 물음인지를 꾸미는 말이라 바꿔 써도
//! 답이 달라지지 않는다. 그래서 앞쪽이 자료에 없다고 거부하지 않는다.
//!
//! 골든 셋 67문항에서 A+B 는 **값을 치르지 않았다** — 잘못 거부 11.5% 와 정답 답변
//! 29/29 가 그대로인 채 지어낸 답 8건 가운데 4건(`제재`·`문항`·`최대`·`비율`)을
//! 막았다. 끝 2개로 넓히면(`주요 활동` 의 `주요`) 맞는 답을 버렸고, 인용 청크까지
//! 거부에 쓰면(`하루` 대 `1일`) 맞는 답을 버렸다. 그래서 A·B 만 거부, C 는 경고다.
//! 설계안 5-4 여덟 · `eval::concept`.
//!
//! ## 낱말 고르기 — 사전 없이
//!
//! 형태소 분석기도, 동의어 사전도 쓰지 않는다. 세 가지만 한다.
//!   ① 숫자·단위·물음말은 뺀다 (`5퍼센트까지`·`%야`·`얼마까지`·`몇`)
//!   ② **명사+하다/되다** 는 앞의 명사를 살린다 (`폐강해야`→`폐강`, `구성되는가`→`구성`) — 그 명사가 묻는 것이다
//!   ③ 토박이 동사 활용(`받아도`·`걸려`·`바뀌었어`)은 어미 꼬리와 짧은 활용형 목록으로 뺀다
//! 무엇을 골랐는지는 개발 정보에 그대로 보인다.

use crate::domain::query;
use serde::Serialize;

/// 물음에서 뽑은 개념 낱말 하나. 찾을 때 쓸 꼴(원래 꼴·조사 뗀 꼴)을 함께 든다.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Word {
    pub raw: String,
    pub forms: Vec<String>,
}

/// 물음의 낱말들, 물음에 나온 차례대로. 물음말·한 글자·중복은 뺀다 (낱말 검색과 같은 자).
fn words(question: &str) -> Vec<Word> {
    let p = query::parse(question);
    // parse 는 terms 와 short 를 따로 돌려주므로 물음에 나온 차례를 되찾는다
    let mut out: Vec<(usize, Word)> = Vec::new();
    let mut push = |raw: &str, forms: Vec<String>| {
        if query::is_question_word(raw) || out.iter().any(|(_, w)| w.raw == raw) {
            return;
        }
        let at = question.find(raw).unwrap_or(usize::MAX);
        out.push((at, Word { raw: raw.to_string(), forms }));
    };
    for t in &p.terms {
        push(&t.raw, t.forms.clone());
    }
    for s in &p.short {
        push(s, vec![s.clone()]);
    }
    out.sort_by_key(|(at, _)| *at);
    out.into_iter().map(|(_, w)| w).collect()
}

/// 숫자·단위 낱말인가 (`5퍼센트까지`, `30000원에서`, `2km`, `%야`, `개야`). 개념이 아니라 값이다.
fn numeric(w: &str) -> bool {
    let first = w.chars().next();
    first.is_some_and(|c| c.is_ascii_digit() || c == '%')
        // `개야`·`명이야` — 단위에 물음 꼬리가 붙은 것
        || (w.ends_with('야') && w.chars().count() <= 3 && !w.starts_with('명'))
}

/// 자주 쓰는 동사·형용사 활용형 (맨 낱말로 나올 때). 두 글자라서 꼬리로는 못 가르는 것들.
/// 동의어 사전이 아니다 — "이건 명사가 아니다" 를 가리는 최소한의 자다.
const VERB_FORMS: &[&str] = &[
    "주는", "쓰는", "있는", "없는", "받는", "사는", "하는", "되는", "오는", "가는", "보는",
    "써도", "사도", "해도", "줘도", "봐도", "받아", "받을", "걸려", "해야", "하면", "되면",
    "남은", "쓰고", "끝난", "열려", "나면", "맡는", "이어", "주나", "올리", "채우", "안에",
    "해", "돼", "써", "줘", "봐", "사",
];

/// 낱말을 개념 낱말로 다듬는다. 명사가 아니면 None.
///
/// `폐강해야`·`처리해야`·`구성되는가` 처럼 **명사 + 하다/되다** 활용은 앞의 명사를
/// 살린다 — 그 명사(`폐강`·`처리`·`구성`)가 바로 물음이 묻는 것이기 때문이다.
/// 토박이 동사(`바뀌었어`·`걸려`·`받아도`)는 하/되 가 없어 여기서 걸러진다.
fn noun_of(w: &Word) -> Option<String> {
    let raw = w.raw.as_str();
    let n = raw.chars().count();
    if VERB_FORMS.contains(&raw) {
        return None;
    }

    // 하다/되다 활용 — 앞의 명사를 살린다.
    // 세 글자 이상에만 — `상한`·`기한`·`위반` 같은 두 글자 명사의 `한` 을 어미로 보면 안 된다.
    const HA: &[&str] = &[
        "하나요", "하는가", "합니까", "할까요", "해야", "하면", "하는", "해도", "해줘", "하지",
        "하고", "해서", "할", "한", "해", "돼야", "되나요", "되는가", "됩니까", "되면", "되는",
        "되고", "되지", "될", "된", "돼", "되나",
    ];
    if n >= 3 {
        for t in HA {
            let tn = t.chars().count();
            if raw.ends_with(t) && n - tn >= 2 {
                return Some(raw.chars().take(n - tn).collect());
            }
        }
    }

    // 그 밖의 활용 어미 꼬리 (세 글자 이상인 낱말에만 — `제도`·`한도` 같은 두 글자 명사를 지키려고)
    const TAILS: &[&str] = &[
        "인가요", "일까요", "습니까", "으면", "않으면", "있는", "없는", "받는", "있어", "없어",
        "고서", "이면", "라면", "려면", "다면", "지만", "어와", "어서", "았어", "었어", "나요",
        "우지", "치지", "르지", "하지", "되지", "이지",
        "줘", "야", "죠", "니", "고", "면", "며", "려", "어", "요",
        "쓸", "줄", "볼", "올", "낼", "운", "게", "히",
    ];
    if n >= 3 && TAILS.iter().any(|t| raw.ends_with(t) && n > t.chars().count()) {
        return None;
    }

    // 조사를 떼고, 뗀 꼴이 동사 활용형이면 명사가 아니다 (`받아도` → `받아`)
    if let Some(stem) = query::strip_particle(raw) {
        if VERB_FORMS.contains(&stem.as_str()) {
            return None;
        }
    }
    Some(raw.to_string())
}

/// 물음말·꾸밈말로 흔한 낱말. 개념이 아니다.
fn filler(w: &Word) -> bool {
    const FILLERS: &[&str] = &[
        "어떤", "어느", "무슨", "몇", "어떻게", "언제", "어디", "누구", "누가", "왜",
        "너무", "많이", "대신", "다른", "같은", "이런", "그런", "모든", "각", "매", "다음",
        "가능", "관련", "경우", "때", "것", "수", "등", "및", "또는", "그리고", "직접", "한",
    ];
    if FILLERS.contains(&w.raw.as_str()) || w.forms.iter().any(|f| FILLERS.contains(&f.as_str())) {
        return true;
    }
    // `수는`·`것이` — 한 글자 꾸밈말에 조사가 붙은 것 (조사 떼기는 두 글자를 남겨야 떼므로 여기서 본다)
    let cs: Vec<char> = w.raw.chars().collect();
    cs.len() == 2 && FILLERS.contains(&cs[0].to_string().as_str())
}

/// 물음의 **개념 낱말**만, 물음에 나온 차례대로 — 숫자도, 서술어도, 꾸밈말도 아닌 것.
pub fn concepts(question: &str) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::new();
    for w in words(question) {
        if numeric(&w.raw) || filler(&w) {
            continue;
        }
        let Some(noun) = noun_of(&w) else { continue };
        // 다듬은 낱말로 찾을 꼴을 다시 만든다 — 조사를 뗀 꼴도 함께
        let mut forms = vec![noun.clone()];
        if let Some(stem) = query::strip_particle(&noun) {
            forms.push(stem);
        }
        // 같은 개념이 두 번 나오면(`이월할 때 이월 상한`) 한 번만
        let key = forms.last().cloned().unwrap_or_default();
        if out.iter().any(|o| o.forms.last().is_some_and(|k| *k == key)) {
            continue;
        }
        out.push(Word { raw: noun, forms });
    }
    out
}

/// 물음의 초점 — 끝의 개념 낱말 하나.
pub fn focus(question: &str) -> Option<Word> {
    concepts(question).pop()
}

/// 글 뭉치를 빈칸 없이 이어 붙인다 — 낱말이 줄바꿈으로 갈라져 있어도 찾도록.
pub fn joined(texts: &[String]) -> String {
    texts
        .iter()
        .map(|t| t.replace(char::is_whitespace, ""))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 낱말이 글에 있는가. 조사를 뗀 꼴과 꼬리 한두 자를 뗀 꼴까지 본다 (`verify::appears`).
pub fn present(w: &Word, joined_text: &str) -> bool {
    w.forms.iter().any(|f| super::verify::appears(joined_text, f))
}

/// 자료집 전체에서 찾을 때 SQL 에 넘길 조각들 — `appears` 와 같은 꼴을 만든다
/// (원래 꼴, 조사 뗀 꼴, 각각의 꼬리 한두 자 뗀 꼴). 빈칸을 지운 글에서 찾는다.
pub fn needles(w: &Word) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for f in &w.forms {
        let cs: Vec<char> = f.chars().collect();
        let mut push = |s: String| {
            if !out.contains(&s) {
                out.push(s);
            }
        };
        push(f.clone());
        for cut in [1usize, 2] {
            if cs.len() > cut + 1 {
                push(cs[..cs.len() - cut].iter().collect());
            }
        }
    }
    out
}

/// 초점 낱말을 세 범위에서 찾은 결과. 화면과 판단에 그대로 쓴다.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusCheck {
    /// 초점 낱말 (없으면 물음에서 개념을 못 골랐다 — 아무 판단도 하지 않는다)
    pub word: Option<String>,
    /// 선택한 자료집 전체에 있는가
    pub in_collection: bool,
    /// LLM 에 넘긴 근거에 있는가
    pub in_evidence: bool,
    /// 답이 인용한 청크에 있는가 (답을 받은 뒤에야 안다)
    pub in_cited: Option<bool>,
}

impl FocusCheck {
    /// 근거를 고른 뒤, 모델을 부르기 **전에** 본다. `in_collection` 은 부르는 쪽이
    /// 자료집 전체를 훑어 준다 (`repo::chunk::any_contains`).
    pub fn before_answer(question: &str, in_collection: impl FnOnce(&Word) -> bool, evidence_texts: &[String]) -> Self {
        let Some(w) = focus(question) else {
            return Self::default();
        };
        let in_collection = in_collection(&w);
        let in_evidence = in_collection && present(&w, &joined(evidence_texts));
        // 화면에는 조사를 뗀 꼴로 — "'제재를' 에 관한 내용" 이 아니라 "'제재' 에 관한 내용"
        let shown = w.forms.last().cloned().unwrap_or(w.raw);
        Self {
            word: Some(shown),
            in_collection,
            in_evidence,
            in_cited: None,
        }
    }

    /// 답을 받은 뒤, 인용한 청크의 원문으로 C 를 채운다.
    pub fn after_answer(&mut self, question: &str, cited_texts: &[String]) {
        if self.word.is_none() || !self.in_evidence {
            return;
        }
        if let Some(w) = focus(question) {
            self.in_cited = Some(present(&w, &joined(cited_texts)));
        }
    }

    /// 모델을 부르지 않고 답하지 않아야 하는가. 사유는 A·B 가 다르다.
    pub fn refusal(&self) -> Option<String> {
        let w = self.word.as_ref()?;
        if !self.in_collection {
            return Some(format!(
                "이 자료집에는 '{w}' 에 관한 내용이 없습니다."
            ));
        }
        if !self.in_evidence {
            return Some(format!(
                "자료집에 '{w}' 이(가) 나오지만 찾아온 근거에는 없습니다. 검색이 놓쳤을 수 있으니 '{w}' 으로 낱말 검색을 해 보세요."
            ));
        }
        None
    }

    /// 답은 보이되 화면에 적을 경고 (C).
    pub fn warning(&self) -> Option<String> {
        let w = self.word.as_ref()?;
        match self.in_cited {
            Some(false) => Some(format!(
                "물음의 '{w}' 이(가) 답변이 인용한 근거에는 없습니다. 근거가 물음과 다른 내용일 수 있습니다."
            )),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod tests;
