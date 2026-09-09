//! **물음–근거 적합성 판정** 실험 (P5b · 요구사항 6).
//!
//!     cargo test --lib eval::judge -- --nocapture
//!
//! 핵심 개념 부재 신호(`eval::concept`)로 못 잡는 것이 남는다 — 물음의 낱말이
//! 자료집에도, 근거에도, 인용한 청크에도 다 있는데 **근거가 그 물음에 답하지는
//! 않는** 경우다 ("담당 교사 수당" 을 물었는데 근거는 "강사 수당"). 이것은 낱말로는
//! 안 보이고 뜻으로만 보인다.
//!
//! 그래서 아주 짧은 판정을 따로 물어 본다. **답을 만들지 않는다.** 물음과 인용한
//! 근거만 주고 `answerable / insufficient` 하나만 받는다. CPU 에서 한 번 더 부르는
//! 값이 얼마인지(초)와 무엇을 더 잡는지(건)를 함께 잰다. 제품에 넣을지는 그 둘을
//! 보고 정한다.
//!
//! 판정도 갈무리한다 (`test/golden/answers/<모델>-judge/`). 물음·근거가 같으면
//! 다시 묻지 않는다.

use super::answer::{load_all, run_one};
use super::*;
use crate::ai::ollama;
use crate::answer::refuse::Decision;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

const MODEL: &str = "gemma3:4b";

const SYSTEM: &str = "\
너는 근거가 물음에 답하는지만 판단한다. 답을 쓰지 않는다.

- 근거가 물음이 묻는 바로 그것(예: 제재, 수당, 한도, 횟수, 기한, 대상)을 직접 말하면 answerable 은 true 다.
- 근거가 물음과 관련은 있지만 묻는 것 자체는 말하지 않으면 false 다.
  예) 물음 \"의무 시수를 안 채우면 어떤 제재?\" · 근거 \"유치원 연 1회, 초·중·고 연 2회\" → false (횟수는 있고 제재는 없다)
  예) 물음 \"담당 교사 수당은?\" · 근거 \"외부 강사 수당 1시간 6만원\" → false (강사 수당이지 교사 수당이 아니다)
- missing 에는 근거에 없는 것을 한 구절로 적는다. answerable 이 true 면 빈 글로 둔다.

정해진 JSON 하나만 내놓는다.";

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "answerable": { "type": "boolean" },
            "missing": { "type": "string" }
        },
        "required": ["answerable", "missing"]
    })
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cached {
    question_id: String,
    model: String,
    prompt_hash: String,
    raw: String,
    llm_ms: u64,
}

#[derive(Debug, Deserialize)]
struct Verdict {
    answerable: bool,
    #[serde(default)]
    missing: String,
}

fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))[..16].to_string()
}

fn ask(qid: &str, prompt: &str) -> Result<(String, u64, bool), String> {
    let stem = MODEL.replace(':', "-");
    let path = format!("{ROOT}/answers/{stem}-judge/{qid}.json");
    let want = hash(prompt);
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(c) = serde_json::from_str::<Cached>(&text) {
            if c.prompt_hash == want && c.model == MODEL {
                return Ok((c.raw, c.llm_ms, false));
            }
        }
    }
    let out = ollama::chat_stream(
        MODEL,
        SYSTEM,
        prompt,
        Some(schema()),
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )?;
    std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).ok();
    let c = Cached {
        question_id: qid.to_string(),
        model: MODEL.to_string(),
        prompt_hash: want,
        raw: out.text.clone(),
        llm_ms: out.elapsed_ms,
    };
    std::fs::write(&path, serde_json::to_string_pretty(&c).unwrap() + "\n").ok();
    Ok((out.text, out.elapsed_ms, true))
}

/// 판정에 넘길 근거 — **답이 인용한 청크만.** 인용이 없으면 넘긴 근거 전체.
/// 길이는 3,000자에서 자른다 — 판정은 짧아야 한다.
fn evidence_for(ran: &super::answer::Ran) -> String {
    let picked: Vec<&String> = if ran.cited_ids.is_empty() {
        ran.evidence_texts.iter().collect()
    } else {
        ran.cited_ids
            .iter()
            .filter_map(|c| ran.evidence_ids.iter().position(|e| e == c))
            .map(|i| &ran.evidence_texts[i])
            .collect()
    };
    let mut s = String::new();
    for (i, t) in picked.iter().enumerate() {
        s.push_str(&format!("[근거{}]\n{}\n\n", i + 1, t));
        if s.chars().count() > 3000 {
            break;
        }
    }
    s.chars().take(3000).collect()
}

#[test]
fn 물음_근거_적합성을_모델에게_묻는다() {
    let Ok(vs) = load_vectors() else {
        println!("\n[적합성 판정] 벡터가 없어 재지 못했습니다.");
        return;
    };
    let corpus = load_corpus();
    let (conn, ids) = build(&corpus, Some(&vs));
    let all = load_all();

    struct Row {
        id: String,
        positive: bool,
        answered: bool,
        right: bool,
        answerable: Option<bool>,
        missing: String,
        ms: u64,
        fresh: bool,
        answer: String,
    }
    let mut rows: Vec<Row> = Vec::new();

    let mut look = |id: &str, question: &str, coll: i64, positive: bool, want: Vec<i64>| -> bool {
        let Some(ran) = run_one(&conn, &vs, id, question, coll) else { return false };
        let answered = matches!(ran.decision, Decision::Answer | Decision::Limited);
        let right = positive && answered && ran.cited_ids.iter().any(|c| want.contains(c));
        // 판정은 **P5 가 답한 것**에만 건다 — 거부한 것은 판정할 것이 없다
        let (answerable, missing, ms, fresh) = if answered {
            let prompt = format!("[물음]\n{}\n\n[근거]\n{}", question, evidence_for(&ran));
            match ask(id, &prompt) {
                Ok((raw, ms, fresh)) => match serde_json::from_str::<Verdict>(&raw) {
                    Ok(v) => (Some(v.answerable), v.missing, ms, fresh),
                    Err(_) => (None, format!("형식 못 읽음: {}", raw.chars().take(60).collect::<String>()), ms, fresh),
                },
                Err(e) => {
                    println!("  {id}: 모델에 붙지 못했습니다 — {e}");
                    return false;
                }
            }
        } else {
            (None, String::new(), 0, false)
        };
        rows.push(Row {
            id: id.to_string(),
            positive,
            answered,
            right,
            answerable,
            missing,
            ms,
            fresh,
            answer: ran.answer.chars().take(50).collect(),
        });
        true
    };

    for q in &all.questions {
        let want = wanted(q, &ids);
        if !look(&q.question_id, &q.question, q.collection_id, true, want) {
            println!("\n[적합성 판정] 재지 못했습니다 — Ollama 를 켜고 다시 돌리세요.");
            return;
        }
    }
    for n in &all.negatives {
        if !look(&n.question_id, &n.question, n.collection_id, false, vec![]) {
            println!("\n[적합성 판정] 답이 없는 물음을 재지 못했습니다.");
            return;
        }
    }

    // ── 표 ──────────────────────────────────────────────────────────
    let judged: Vec<&Row> = rows.iter().filter(|r| r.answered).collect();
    let fresh = judged.iter().filter(|r| r.fresh).count();
    let ms: u64 = judged.iter().map(|r| r.ms).sum();
    println!(
        "\n── 적합성 판정 {MODEL} · 판정한 답 {}개 (새로 물은 것 {}개) · 한 번에 평균 {:.1}초",
        judged.len(),
        fresh,
        if judged.is_empty() { 0.0 } else { ms as f64 / judged.len() as f64 / 1000.0 }
    );

    println!("\n[답이 없는 물음인데 P5 가 답한 것 — 판정이 잡는가]");
    for r in judged.iter().filter(|r| !r.positive) {
        println!(
            "  {:<6} {:<12} {:<28} 답: {}",
            r.id,
            match r.answerable { Some(true) => "answerable ✗", Some(false) => "insufficient ✓", None => "판정 못 읽음" },
            r.missing.chars().take(26).collect::<String>(),
            r.answer
        );
    }
    println!("\n[답이 있는 물음에 P5 가 답한 것 — 판정이 잘못 막는가]");
    for r in judged.iter().filter(|r| r.positive && r.answerable != Some(true)) {
        println!(
            "  {:<6} {:<10} {:<12} {:<28} 답: {}",
            r.id,
            if r.right { "정답 인용" } else { "엉뚱한 근거" },
            match r.answerable { Some(false) => "insufficient ✗", None => "판정 못 읽음", _ => "" },
            r.missing.chars().take(26).collect::<String>(),
            r.answer
        );
    }

    let neg_caught = judged.iter().filter(|r| !r.positive && r.answerable == Some(false)).count();
    let neg_ans = judged.iter().filter(|r| !r.positive).count();
    let pos_blocked = judged.iter().filter(|r| r.positive && r.answerable == Some(false)).count();
    let pos_right_blocked = judged.iter().filter(|r| r.positive && r.right && r.answerable == Some(false)).count();
    let pos_right = judged.iter().filter(|r| r.positive && r.right).count();
    let pos_ans = judged.iter().filter(|r| r.positive).count();
    println!(
        "\n  지어낸 답 {}개 가운데 판정이 막은 것 {}개 · 답한 positive {}개 가운데 막은 것 {}개 (그중 정답이었던 것 {}/{})",
        neg_ans, neg_caught, pos_ans, pos_blocked, pos_right_blocked, pos_right
    );
}
