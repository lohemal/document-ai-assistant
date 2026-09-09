//! **핵심 개념 부재** 실험 (P5b).
//!
//!     cargo test --lib eval::concept -- --nocapture
//!
//! P5 에서 지어내 답한 8건은 모두 같은 모습이었다 — 근거를 제대로 옮겨 적고,
//! 그것을 물음에 대한 답으로 내놓는다. 물음이 "제재" 를 물었는데 근거는
//! "교육 횟수" 를 말한다. 인용·숫자·주장 뒷받침 검사가 모두 통과한다.
//!
//! 이 실험은 **비율을 보지 않는다** (P5 의 `question_gap` 은 비율이었고, 그것으로
//! 자르면 맞는 답을 두 배로 잃었다). 대신 물음의 **초점 낱말** — 답의 성격을
//! 정하는 낱말(`제재`·`수당`·`상한`·`문항`) — 이 어디에 있는지를 세 범위에서 본다.
//!
//!   A. **선택한 자료집 전체**       없으면 → 자료에 없는 물음일 가능성
//!   B. **LLM 에게 넘긴 근거**       A 에는 있는데 B 에 없으면 → 검색이 못 가져왔을 가능성
//!   C. **답이 실제로 인용한 청크**  A·B 에는 있는데 C 에 없으면 → 근거는 있으나 물음에 답하지 않았을 가능성
//!
//! 세 가지는 뜻이 다르므로 같은 거부 사유로 묶지 않는다.
//!
//! 초점 낱말은 **물음의 끝에서 고른다.** 한국어 물음은 묻는 것을 서술어 바로 앞에
//! 둔다 — "…어떤 **제재**를 받아?", "…**문항**이 몇 개야?", "…**수당**은 얼마야?".
//! 앞쪽 낱말(`형편이 어려운 아이들`)은 무엇에 대한 물음인지를 꾸미는 말이라
//! 바꿔 써도 답이 달라지지 않는다. 그래서 앞쪽이 자료에 없다고 거부하지 않는다.
//!
//! 제품 코드에는 아직 아무것도 넣지 않는다. 갈무리해 둔 67문항 답으로 먼저 잰다.

use super::answer::{load_all, run_one, Ran};
use super::*;
use crate::answer::refuse::Decision;
use crate::domain::query;

/// 물음에서 뽑은 낱말 하나. 원래 꼴과 조사를 뗀 꼴을 함께 든다.
#[derive(Debug, Clone)]
struct Word {
    raw: String,
    forms: Vec<String>,
}

/// 물음의 낱말들 (물음에 나온 차례대로). 물음말·한 글자·중복은 뺀다.
fn words(question: &str) -> Vec<Word> {
    let p = query::parse(question);
    // parse 는 terms 와 short 를 따로 돌려주므로, 물음에 나온 차례를 되찾는다
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

/// 글 뭉치를 빈칸 없이 이어 붙인다 — 낱말이 줄바꿈으로 갈라져 있어도 찾도록.
fn joined(texts: &[String]) -> String {
    texts
        .iter()
        .map(|t| t.replace(char::is_whitespace, ""))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 낱말이 글에 있는가. 조사를 뗀 꼴과 꼬리 한두 자를 뗀 꼴까지 본다 (`verify::appears`).
fn present(w: &Word, text: &str) -> bool {
    w.forms.iter().any(|f| crate::answer::verify::appears(text, f))
}

/// 숫자·단위 낱말인가 (`5퍼센트까지`, `30000원에서`, `2km`, `%야`, `개야`). 개념이 아니라 값이다.
fn numeric(w: &str) -> bool {
    let first = w.chars().next();
    first.is_some_and(|c| c.is_ascii_digit() || c == '%')
        // `개야`·`명이야` — 단위에 물음 꼬리가 붙은 것
        || (w.ends_with('야') && w.chars().count() <= 3 && !w.starts_with('명'))
}

/// **명사로 볼 수 있는가.**
///
/// 형태소 분석기를 쓰지 않는다. 대신 두 가지만 본다.
///
/// ① 조사가 붙어 있었으면 명사구다 — `제재를`·`문항이`·`수당은`·`돈은`.
///    동사는 격조사를 받지 않는다. (`주는`·`써도` 의 `는`·`도` 는 조사가 아니라 어미지만
///    `domain::query` 는 남는 글자가 두 자 미만이면 떼지 않으므로 여기 걸리지 않는다.)
/// ② 조사가 없는 맨 낱말은, **활용 어미 꼬리**가 아니고 **자주 쓰는 동사 활용형
///    목록**에도 없으면 명사로 본다 — `최대`·`강의`·`이월`·`담당`·`명절`.
///
/// 아래 목록은 동의어 사전이 아니다. 이 실험의 67문항에 나온 활용형을 걸러 내는
/// 최소한의 자이고, 무엇을 걸러 냈는지 화면에 그대로 찍어 확인한다.
/// 낱말을 **개념 낱말**로 다듬는다. 명사가 아니면 None.
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

    // 하다/되다 활용 — 앞의 명사를 살린다
    const HA: &[&str] = &[
        "하나요", "하는가", "합니까", "할까요", "해야", "하면", "하는", "해도", "해줘", "하지",
        "하고", "해서", "할", "한", "해", "돼야", "되나요", "되는가", "됩니까", "되면", "되는",
        "되고", "되지", "될", "된", "돼", "되나",
    ];
    // 세 글자 이상에만 — `상한`·`기한`·`위반` 같은 두 글자 명사의 `한` 을 어미로 보면 안 된다
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

/// 자주 쓰는 동사·형용사 활용형 (맨 낱말로 나올 때). 두 글자라서 꼬리로는 못 가르는 것들.
const VERB_FORMS: &[&str] = &[
    "주는", "쓰는", "있는", "없는", "받는", "사는", "하는", "되는", "오는", "가는", "보는",
    "써도", "사도", "해도", "줘도", "봐도", "받아", "받을", "걸려", "해야", "하면", "되면",
    "남은", "쓰고", "끝난", "열려", "나면", "맡는", "이어", "주나", "올리", "채우", "안에",
    "해", "돼", "써", "줘", "봐", "사",
];

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
fn concepts(question: &str) -> Vec<Word> {
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

/// 한 물음에서 본 것.
struct Seen {
    id: String,
    question: String,
    /// 답이 있어야 하는 물음인가
    positive: bool,
    /// 답했고, 정답 근거를 인용했는가 (positive 만)
    right: bool,
    ran: Ran,
    /// 개념 낱말 (차례대로)
    concepts: Vec<Word>,
    /// 자료집 전체에 있는가 / 넘긴 근거에 있는가 / 인용한 청크에 있는가 (개념 낱말과 같은 차례)
    in_all: Vec<bool>,
    in_evidence: Vec<bool>,
    in_cited: Vec<bool>,
}

impl Seen {
    fn answered(&self) -> bool {
        matches!(self.ran.decision, Decision::Answer | Decision::Limited)
    }
    /// 끝에서 n 개의 개념 낱말 (초점)
    fn focus(&self, n: usize) -> impl Iterator<Item = usize> + '_ {
        let len = self.concepts.len();
        (len.saturating_sub(n)..len).rev()
    }
    fn names(&self, idx: impl Iterator<Item = usize>) -> String {
        let v: Vec<&str> = idx.map(|i| self.concepts[i].raw.as_str()).collect();
        if v.is_empty() { "—".to_string() } else { v.join(", ") }
    }
    /// 초점 낱말 가운데 자료집 전체에 없는 것이 있는가
    fn focus_absent_all(&self, n: usize) -> bool {
        self.focus(n).any(|i| !self.in_all[i])
    }
    /// 초점 낱말 가운데 자료집엔 있는데 인용한 청크에 없는 것이 있는가
    fn focus_absent_cited(&self, n: usize) -> bool {
        self.focus(n).any(|i| self.in_all[i] && !self.in_cited[i])
    }
    /// 초점 낱말 가운데 자료집엔 있는데 넘긴 근거에 없는 것이 있는가 (검색이 놓쳤을 가능성)
    fn focus_absent_evidence(&self, n: usize) -> bool {
        self.focus(n).any(|i| self.in_all[i] && !self.in_evidence[i])
    }
}

/// 정책 하나를 67문항에 적용한 결과.
struct Score {
    name: String,
    false_answer: usize,
    correct_refusal: usize,
    false_refusal: usize,
    /// P5 가 정답 근거를 인용해 답한 것 가운데 그대로 남은 것
    kept_right: usize,
    right_total: usize,
    neg_total: usize,
    pos_total: usize,
    /// 이 정책이 새로 거부한 물음
    newly: Vec<String>,
}

impl Score {
    fn print(&self) {
        println!(
            "  {:<40} 지어내 답함 {:>2}/{} ({:>5.1}%) · 잘못 거부 {:>2}/{} ({:>5.1}%) · 정상 답변 유지 {:>2}/{} ({:>5.1}%)",
            self.name,
            self.false_answer,
            self.neg_total,
            self.false_answer as f64 * 100.0 / self.neg_total as f64,
            self.false_refusal,
            self.pos_total,
            self.false_refusal as f64 * 100.0 / self.pos_total as f64,
            self.kept_right,
            self.right_total,
            if self.right_total == 0 { 0.0 } else { self.kept_right as f64 * 100.0 / self.right_total as f64 },
        );
        if !self.newly.is_empty() {
            println!("  {:<40}   새로 거부: {}", "", self.newly.join(" "));
        }
    }
}

/// `refuse(seen)` 이 true 면 그 물음을 (P5 판단에 더해) 거부한다고 보고 센다.
fn score(name: &str, all: &[Seen], refuse: impl Fn(&Seen) -> bool) -> Score {
    let mut s = Score {
        name: name.to_string(),
        false_answer: 0,
        correct_refusal: 0,
        false_refusal: 0,
        kept_right: 0,
        right_total: 0,
        neg_total: 0,
        pos_total: 0,
        newly: Vec::new(),
    };
    for q in all {
        let was = q.answered();
        let ans = was && !refuse(q);
        if was && !ans {
            s.newly.push(format!("{}{}", q.id, if q.positive { if q.right { "(정답이었음)" } else { "(엉뚱했음)" } } else { "✓" }));
        }
        if q.positive {
            s.pos_total += 1;
            if q.right {
                s.right_total += 1;
                if ans {
                    s.kept_right += 1;
                }
            }
            if !ans {
                s.false_refusal += 1;
            }
        } else {
            s.neg_total += 1;
            if ans {
                s.false_answer += 1;
            } else {
                s.correct_refusal += 1;
            }
        }
    }
    s
}

#[test]
fn 핵심_개념_부재를_잰다() {
    let Ok(vs) = load_vectors() else {
        println!("\n[핵심 개념] 벡터가 없어 재지 못했습니다. npm run golden:embed 를 먼저 돌리세요.");
        return;
    };
    let corpus = load_corpus();
    let (conn, ids) = build(&corpus, Some(&vs));
    let all = load_all();

    // 자료집(= 문서 하나)별 전체 글
    let mut coll_text: HashMap<i64, String> = HashMap::new();
    for d in &corpus.documents {
        let texts: Vec<String> = d.chunks.iter().map(|c| c.text_norm.clone()).collect();
        let t = coll_text.entry(d.collection_id).or_default();
        t.push(' ');
        t.push_str(&joined(&texts));
    }

    let mut seen: Vec<Seen> = Vec::new();

    let mut look = |id: &str, question: &str, coll: i64, positive: bool, want: Vec<i64>| -> bool {
        let Some(ran) = run_one(&conn, &vs, id, question, coll) else {
            return false;
        };
        let all_text = &coll_text[&coll];
        let ev_text = joined(&ran.evidence_texts);
        let cited_texts: Vec<String> = ran
            .cited_ids
            .iter()
            .filter_map(|c| ran.evidence_ids.iter().position(|e| e == c))
            .map(|i| ran.evidence_texts[i].clone())
            .collect();
        let cited_text = joined(&cited_texts);

        let concepts = concepts(question);
        let in_all = concepts.iter().map(|w| present(w, all_text)).collect();
        let in_evidence = concepts.iter().map(|w| present(w, &ev_text)).collect();
        let in_cited = concepts.iter().map(|w| present(w, &cited_text)).collect();

        let answered = matches!(ran.decision, Decision::Answer | Decision::Limited);
        let right = positive && answered && ran.cited_ids.iter().any(|c| want.contains(c));
        seen.push(Seen {
            id: id.to_string(),
            question: question.to_string(),
            positive,
            right,
            ran,
            concepts,
            in_all,
            in_evidence,
            in_cited,
        });
        true
    };

    for q in &all.questions {
        let want = wanted(q, &ids);
        if !look(&q.question_id, &q.question, q.collection_id, true, want) {
            println!("\n[핵심 개념] 재지 못했습니다 — 갈무리해 둔 답이 없고 Ollama 에 붙지도 못했습니다.");
            return;
        }
    }
    for n in &all.negatives {
        if !look(&n.question_id, &n.question, n.collection_id, false, vec![]) {
            println!("\n[핵심 개념] 답이 없는 물음을 재지 못했습니다.");
            return;
        }
    }

    // ── 물음마다 ─────────────────────────────────────────────────────
    let mark = |q: &Seen, i: usize| -> String {
        format!(
            "{}{}",
            q.concepts[i].raw,
            match (q.in_all[i], q.in_evidence[i], q.in_cited[i]) {
                (false, _, _) => "[자료집✗]",
                (true, false, _) => "[근거✗]",
                (true, true, false) => "[인용✗]",
                _ => "",
            }
        )
    };

    println!("\n[답이 없는 물음 15개] 개념 낱말(차례대로) — ✗ 표시가 없으면 어디에나 있다");
    for q in seen.iter().filter(|s| !s.positive) {
        let ws: Vec<String> = (0..q.concepts.len()).map(|i| mark(q, i)).collect();
        println!(
            "  {:<6} {:<7} {}",
            q.id,
            if q.answered() { "답함 ✗" } else { "거부 ✓" },
            ws.join(" · ")
        );
        println!("         {}", q.question);
    }

    println!("\n[답이 있는 물음 52개] 개념 낱말(차례대로) — 걸러 낸 것이 맞는지 눈으로 본다");
    for q in seen.iter().filter(|s| s.positive) {
        let ws: Vec<String> = (0..q.concepts.len()).map(|i| mark(q, i)).collect();
        println!("  {:<6} {}", q.id, ws.join(" · "));
    }

    println!("\n[답이 있는 물음 52개 가운데 초점 낱말(끝 2개)에 ✗ 가 있는 것]");
    for q in seen.iter().filter(|s| s.positive) {
        let flagged: Vec<String> = q
            .focus(2)
            .filter(|&i| !q.in_all[i] || !q.in_evidence[i] || !q.in_cited[i])
            .map(|i| mark(q, i))
            .collect();
        if flagged.is_empty() {
            continue;
        }
        println!(
            "  {:<6} {:<10} {:<32} — {}",
            q.id,
            if !q.answered() { "거부" } else if q.right { "정답 인용" } else { "엉뚱한 근거" },
            flagged.join(" · "),
            q.question
        );
    }

    // ── 정책 견주기 ──────────────────────────────────────────────────
    println!("\n[정책별 — P5 판단에 신호를 더했을 때]");
    score("P5 그대로", &seen, |_| false).print();
    for n in [1usize, 2] {
        score(&format!("A{n}. 초점 낱말(끝 {n}개) 자료집 전체에 없음 → 거부"), &seen, |q| q.focus_absent_all(n)).print();
        score(&format!("C{n}. 초점 낱말(끝 {n}개) 인용 청크에 없음 → 거부"), &seen, |q| q.focus_absent_cited(n)).print();
        score(&format!("B{n}. 초점 낱말 자료집엔 있는데 넘긴 근거에 없음 → 거부"), &seen, |q| q.focus_absent_evidence(n)).print();
        score(&format!("A{n}+B{n}. 자료집 부재 또는 근거 부재 → 거부 (사유는 따로)"), &seen, |q| {
            q.focus_absent_all(n) || q.focus_absent_evidence(n)
        })
        .print();
        score(&format!("A{n}+B{n}+C{n}. 인용 청크 부재까지 → 거부"), &seen, |q| {
            q.focus_absent_all(n) || q.focus_absent_evidence(n) || q.focus_absent_cited(n)
        })
        .print();
    }
    score("A1+B1+C2. 인용 청크는 끝 2개까지 봄", &seen, |q| {
        q.focus_absent_all(1) || q.focus_absent_evidence(1) || q.focus_absent_cited(2)
    })
    .print();
    score("모든 개념 낱말 자료집에 없음 → 거부 (비교용)", &seen, |q| q.in_all.iter().any(|b| !b)).print();

    let _ = |q: &Seen| q.names(q.focus(2));
}
