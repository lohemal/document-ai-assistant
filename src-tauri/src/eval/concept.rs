//! **핵심 개념 부재** 실험 (P5b) — 초점 낱말 규칙을 어디까지 넣을지 정한 자리.
//!
//!     cargo test --lib eval::concept -- --nocapture
//!
//! 낱말 고르기와 세 범위(자료집 전체 A · 넘긴 근거 B · 인용 청크 C)는 이제 제품
//! 코드 `answer::focus` 에 있고, 여기서는 그것을 그대로 불러 **규칙을 넣기 전과 뒤,
//! 그리고 넣지 않은 변형들**(끝 2개 · C 를 거부에 쓰는 것)을 나란히 잰다.
//! 결정의 근거가 된 표이므로 남겨 둔다 — 낱말 고르기를 손보면 이 표가 어떻게
//! 움직이는지 바로 볼 수 있다. 설계안 5-4 여덟.

use super::answer::{load_all, run_one, Ran};
use super::*;
use crate::answer::focus::{self, concepts, joined, present, Word};
use crate::answer::refuse::Decision;
use crate::repo::chunk;

/// 한 물음에서 본 것.
struct Seen {
    id: String,
    question: String,
    positive: bool,
    /// 답했고, 정답 근거를 인용했는가 (positive 만)
    right: bool,
    ran: Ran,
    /// 개념 낱말 (차례대로) 과 세 범위 존재 여부
    concepts: Vec<Word>,
    in_all: Vec<bool>,
    in_evidence: Vec<bool>,
    in_cited: Vec<bool>,
}

impl Seen {
    fn answered(&self) -> bool {
        matches!(self.ran.decision, Decision::Answer | Decision::Limited)
    }
    fn focus(&self, n: usize) -> impl Iterator<Item = usize> + '_ {
        let len = self.concepts.len();
        (len.saturating_sub(n)..len).rev()
    }
    fn focus_absent_all(&self, n: usize) -> bool {
        self.focus(n).any(|i| !self.in_all[i])
    }
    fn focus_absent_evidence(&self, n: usize) -> bool {
        self.focus(n).any(|i| self.in_all[i] && !self.in_evidence[i])
    }
    fn focus_absent_cited(&self, n: usize) -> bool {
        self.focus(n).any(|i| self.in_all[i] && !self.in_cited[i])
    }
}

struct Score {
    name: String,
    false_answer: usize,
    false_refusal: usize,
    kept_right: usize,
    right_total: usize,
    neg_total: usize,
    pos_total: usize,
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

fn score(name: &str, all: &[Seen], refuse: impl Fn(&Seen) -> bool) -> Score {
    let mut s = Score {
        name: name.to_string(),
        false_answer: 0,
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
            s.newly.push(format!(
                "{}{}",
                q.id,
                if q.positive { if q.right { "(정답이었음)" } else { "(엉뚱했음)" } } else { "✓" }
            ));
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

    let mut seen: Vec<Seen> = Vec::new();
    let mut look = |id: &str, question: &str, coll: i64, positive: bool, want: Vec<i64>| -> bool {
        let Some(ran) = run_one(&conn, &vs, id, question, coll) else { return false };
        let ev_text = joined(&ran.evidence_texts);
        let cited_texts: Vec<String> = ran
            .cited_ids
            .iter()
            .filter_map(|c| ran.evidence_ids.iter().position(|e| e == c))
            .map(|i| ran.evidence_texts[i].clone())
            .collect();
        let cited_text = joined(&cited_texts);

        let concepts = concepts(question);
        // 자료집 전체는 앱과 같은 길로 본다 (repo::chunk::any_contains)
        let in_all = concepts
            .iter()
            .map(|w| chunk::any_contains(&conn, &[coll], &focus::needles(w)).unwrap())
            .collect();
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
        println!("  {:<6} {:<7} {}", q.id, if q.answered() { "답함 ✗" } else { "거부 ✓" }, ws.join(" · "));
        println!("         {}", q.question);
    }
    println!("\n[답이 있는 물음 52개 가운데 ✗ 가 하나라도 있는 것]");
    for q in seen.iter().filter(|s| s.positive) {
        let ws: Vec<String> = (0..q.concepts.len()).map(|i| mark(q, i)).collect();
        if ws.iter().any(|w| w.contains('✗')) {
            println!("  {:<6} {}", q.id, ws.join(" · "));
        }
    }

    println!("\n[정책별 — P5 판단에 신호를 더했을 때]");
    score("P5 그대로", &seen, |_| false).print();
    for n in [1usize, 2] {
        score(&format!("A{n}. 초점 낱말(끝 {n}개) 자료집 전체에 없음 → 거부"), &seen, |q| q.focus_absent_all(n)).print();
        score(&format!("B{n}. 자료집엔 있는데 넘긴 근거에 없음 → 거부"), &seen, |q| q.focus_absent_evidence(n)).print();
        score(&format!("A{n}+B{n}. (앱이 쓰는 규칙은 n=1)"), &seen, |q| {
            q.focus_absent_all(n) || q.focus_absent_evidence(n)
        })
        .print();
        score(&format!("C{n}. 인용 청크에 없음 → 거부 (앱은 경고만)"), &seen, |q| q.focus_absent_cited(n)).print();
        score(&format!("A{n}+B{n}+C{n}."), &seen, |q| {
            q.focus_absent_all(n) || q.focus_absent_evidence(n) || q.focus_absent_cited(n)
        })
        .print();
    }
}
