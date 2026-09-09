//! 답변 품질을 **숫자로** 잰다 (P5).
//!
//!     cargo test --lib eval::answer -- --nocapture
//!
//! 재는 것은 "답을 잘 썼는가" 가 아니다. **없는 것을 지어냈는가** 다.
//! 그래서 답이 있는 물음 52개와 **답이 없는 물음 15개**를 함께 돌린다.
//!
//! | | 좋은 것 | 나쁜 것 |
//! |---|---|---|
//! | 답이 있는 물음 | 정답 근거를 인용해 답한다 | 거부한다 (False Refusal) |
//! | 답이 없는 물음 | 거부한다 | 답한다 (False Answer) |
//!
//! **거부율만 높은 것은 좋은 것이 아니다.** 둘을 함께 봐야 뜻이 있다.
//!
//! ## 답을 갈무리해 둔다
//!
//! 4B 모델이 이 PC(CPU)에서 한 물음에 20~40초 걸린다. 67문항이면 30분이다.
//! 그래서 모델이 내놓은 글을 `test/golden/answers/` 에 갈무리해 두고, 물음과
//! 근거가 그대로면 다시 부르지 않는다. 갈무리를 저장소에 넣어 두면 Ollama 가
//! 없는 곳(CI)에서도 같은 평가를 다시 돌릴 수 있다 — 벡터와 같은 방식이다.

use super::*;
use crate::ai::ollama;
use crate::answer::{context, parse, prompt, refuse, verify};
use crate::repo::hybrid;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// 재는 데 쓸 답변 모델. 카탈로그의 가벼운 쪽이다.
const MODEL: &str = "gemma3:4b";

/// 물음 핵심어 덮임을 어디서 자르면 좋을지 보려고 함께 재는 기준들.
/// **어느 하나도 아직 판단에 쓰지 않는다** — 수치를 보고 정한다.
const COV: [f64; 5] = [0.001, 0.2, 0.26, 0.34, 0.4];

#[derive(Debug, Deserialize)]
pub(super) struct Negative {
    pub(super) question_id: String,
    #[allow(dead_code)]
    pub(super) document_id: i64,
    pub(super) collection_id: i64,
    pub(super) question: String,
    /// 자료에 무엇이 없는지 (사람이 적어 둔 것)
    pub(super) absent: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoldenAll {
    pub(super) questions: Vec<Question>,
    #[serde(default)]
    pub(super) negatives: Vec<Negative>,
}

pub(super) fn load_all() -> GoldenAll {
    let path = format!("{ROOT}/golden.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{path} 를 읽지 못했습니다 ({e})"));
    serde_json::from_str(&raw).unwrap()
}

// ── 답 갈무리 ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cached {
    question_id: String,
    model: String,
    /// 물음+근거의 지문. 이것이 달라지면 갈무리를 버린다.
    prompt_hash: String,
    raw: String,
    llm_ms: u64,
    tokens: u64,
}

fn cache_path(qid: &str) -> String {
    let stem = MODEL.replace(':', "-");
    format!("{ROOT}/answers/{stem}/{qid}.json")
}

fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))[..16].to_string()
}

/// 갈무리해 둔 답이 있으면 그것을, 없으면 모델에게 묻는다.
///
/// 돌려주는 것: (모델이 내놓은 글, 걸린 시간, 새로 물었는가)
fn ask_model(qid: &str, user_prompt: &str) -> Result<(String, u64, bool), String> {
    let want = hash(user_prompt);
    let path = cache_path(qid);

    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(c) = serde_json::from_str::<Cached>(&text) {
            if c.prompt_hash == want && c.model == MODEL {
                return Ok((c.raw, c.llm_ms, false));
            }
        }
    }

    let out = ollama::chat_stream(
        MODEL,
        prompt::SYSTEM,
        user_prompt,
        Some(prompt::schema()),
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )?;

    let dir = std::path::Path::new(&path).parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&dir).ok();
    let c = Cached {
        question_id: qid.to_string(),
        model: MODEL.to_string(),
        prompt_hash: want,
        raw: out.text.clone(),
        llm_ms: out.elapsed_ms,
        tokens: out.tokens,
    };
    std::fs::write(&path, serde_json::to_string_pretty(&c).unwrap() + "\n").ok();
    Ok((out.text, out.elapsed_ms, true))
}

// ── 한 물음 돌리기 ───────────────────────────────────────────────────

pub(super) struct Ran {
    pub(super) decision: refuse::Decision,
    /// 근거로 넘어간 청크 id
    pub(super) evidence_ids: Vec<i64>,
    /// LLM 에게 실제로 넘긴 근거의 원문 (핵심 개념 실험용 — `eval::concept`)
    pub(super) evidence_texts: Vec<String>,
    /// 답변이 인용한 청크 id
    pub(super) cited_ids: Vec<i64>,
    pub(super) citations_ok: bool,
    pub(super) numbers_total: usize,
    pub(super) numbers_missing: usize,
    pub(super) llm_ms: u64,
    pub(super) fresh: bool,
    /// **주장 뒷받침 검사** 때문에 거부했는가.
    /// 이 신호를 빼면 어떻게 되는지 함께 재려고 담아 둔다.
    pub(super) refused_by_support: bool,
    /// 뒷받침되지 않은 주장 수
    pub(super) unsupported: usize,
    /// **아직 판단에 쓰지 않는 신호** — 물음의 핵심어를 인용 근거가 얼마나 덮는가.
    /// 넣을지 말지를 이 수치로 정한다 (`refuse::question_gap`).
    pub(super) gap: refuse::KeyWords,
    /// 왜 그렇게 판단했는지 (첫 까닭)
    pub(super) reason: String,
    /// 형식을 못 읽었으면 그 까닭
    pub(super) parse_error: Option<String>,
    pub(super) answer: String,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_one(
    conn: &Connection,
    vs: &Vectors,
    qid: &str,
    question: &str,
    collection_id: i64,
) -> Option<Ran> {
    // ① 찾기 — 앱과 같은 길(섞기)
    let qv = vs
        .questions
        .get(qid)
        .unwrap_or_else(|| panic!("{qid} 의 물음 벡터가 없습니다. npm run golden:embed 를 다시 돌리세요."));
    let found = hybrid::hybrid_search(
        conn,
        &hybrid::Hybrid {
            text: question,
            query_vec: qv,
            model: &vs.model,
            collection_ids: vec![collection_id],
            limit: (context::DEFAULT_PLAN.top_k * 2) as i64,
            depth: hybrid::DEFAULT_DEPTH,
        },
    )
    .unwrap();

    // ② 근거 고르기
    let evidence = context::build(conn, &found.hits, context::DEFAULT_PLAN).unwrap();
    let evidence_ids: Vec<i64> = evidence.iter().map(|e| e.chunk_id).collect();

    if evidence.is_empty() {
        return Some(Ran {
            decision: refuse::Decision::Refuse,
            evidence_ids,
            evidence_texts: evidence.iter().map(|e| e.text.clone()).collect(),
            cited_ids: vec![],
            citations_ok: false,
            numbers_total: 0,
            numbers_missing: 0,
            llm_ms: 0,
            fresh: false,
            refused_by_support: false,
            gap: refuse::KeyWords::default(),
            unsupported: 0,
            reason: String::new(),
            parse_error: None,
            answer: String::new(),
        });
    }

    // ③④ 물음 만들고 답 받기 (갈무리를 먼저 본다)
    let user_prompt = prompt::user(question, &context::render(&evidence));
    let (raw, llm_ms, fresh) = match ask_model(qid, &user_prompt) {
        Ok(v) => v,
        Err(e) => {
            // 갈무리해 둔 답도 없고 모델에도 붙지 못했다.
            // **조용히 통과시키지 않는다** — 부르는 쪽이 왜 못 쟀는지 말한다.
            log::warn!("{qid}: {e}");
            return None;
        }
    };

    // ⑤ 읽기
    let draft = match parse::parse(&raw) {
        Ok(d) => d,
        Err(e) => {
            return Some(Ran {
                decision: refuse::Decision::Refuse,
                evidence_ids,
                evidence_texts: evidence.iter().map(|e| e.text.clone()).collect(),
                cited_ids: vec![],
                citations_ok: false,
                numbers_total: 0,
                numbers_missing: 0,
                llm_ms,
                fresh,
                refused_by_support: false,
                gap: refuse::KeyWords::default(),
                unsupported: 0,
                reason: String::new(),
                parse_error: Some(e),
                answer: String::new(),
            });
        }
    };

    // ⑥ 검증 ⑦ 판단
    let verdict = verify::verify(&draft, &evidence);
    let judgement = refuse::decide(&draft, &verdict, evidence.len(), found.hits.len());

    let cited_ids: Vec<i64> = verdict
        .sources
        .iter()
        .filter_map(|s| s.chunk_id)
        .collect();

    // 아직 판단에 넣지 않은 신호를 함께 담는다 — 넣을지는 수치를 보고 정한다
    let cited_texts: Vec<String> = verdict
        .sources
        .iter()
        .filter_map(|s| s.chunk_id)
        .filter_map(|id| evidence.iter().find(|e| e.chunk_id == id))
        .map(|e| e.text.clone())
        .collect();

    Some(Ran {
        refused_by_support: judgement.decision == refuse::Decision::Refuse
            && verdict.nothing_supported(),
        unsupported: verdict.unsupported_claims().len(),
        gap: refuse::question_gap(question, &cited_texts),
        decision: judgement.decision,
        evidence_ids,
        evidence_texts: evidence.iter().map(|e| e.text.clone()).collect(),
        cited_ids,
        citations_ok: verdict.citations_ok,
        numbers_total: verdict.numbers.len(),
        numbers_missing: verdict.numbers_missing(),
        llm_ms,
        fresh,
        reason: judgement.reasons.first().cloned().unwrap_or_default(),
        parse_error: None,
        answer: draft.answer,
    })
}

// ── 셈 ──────────────────────────────────────────────────────────────

#[derive(Default)]
struct Tally {
    n: usize,
    /// 정답 청크가 근거에 들어왔다
    evidence_hit: usize,
    /// 답했고, 정답 청크를 인용했다
    answered_right: usize,
    /// 답했지만 정답 청크를 인용하지 않았다
    answered_wrong: usize,
    /// 거부했다
    refused: usize,
    /// 형식을 못 읽었다
    parse_failed: usize,
    citations_ok: usize,
    /// 주장 뒷받침 검사를 **빼면** 답이 되는 것 (지금은 거부한 것)
    refused_by_support: usize,
    /// 인용한 근거에서 확인되지 않은 주장 수 (모두 더한 것)
    unsupported: usize,
    /// 답한 것 가운데 물음 핵심어 덮임이 각 기준보다 낮은 것 (아직 안 쓰는 신호).
    /// 기준을 어디로 두면 지어낸 답만 걸리는지 보려고 여러 값을 함께 센다.
    cov_below: [usize; COV.len()],
    numbers_total: usize,
    numbers_missing: usize,
    ms: u64,
    fresh: usize,
}

impl Tally {
    fn pct(&self, k: usize) -> f64 {
        if self.n == 0 { 0.0 } else { k as f64 * 100.0 / self.n as f64 }
    }
}

#[test]
fn 답변_품질을_실제_자료로_잰다() {
    let Ok(vs) = load_vectors() else {
        println!("\n[답변 품질] 벡터가 없어 재지 못했습니다. npm run golden:embed 를 먼저 돌리세요.");
        return;
    };
    let corpus = load_corpus();
    let (conn, ids) = build(&corpus, Some(&vs));
    let all = load_all();

    println!(
        "\n── 답변 모델 {MODEL} · 검색 {} · 청크 {}개",
        vs.model,
        corpus.documents.iter().map(|d| d.chunks.len()).sum::<usize>()
    );
    println!(
        "   답이 있는 물음 {}개 · 답이 없는 물음 {}개",
        all.questions.len(),
        all.negatives.len()
    );

    // ── 답이 있는 물음 ──────────────────────────────────────────────
    let mut pos = Tally::default();
    let mut by_doc: HashMap<i64, Tally> = HashMap::new();
    let mut by_type: HashMap<String, Tally> = HashMap::new();
    let mut false_refusals: Vec<(&Question, String)> = Vec::new();
    let mut wrong_answers: Vec<(&Question, String)> = Vec::new();

    for q in &all.questions {
        let want = wanted(q, &ids);
        let Some(r) = run_one(&conn, &vs, &q.question_id, &q.question, q.collection_id) else {
            println!(
                "
[답변 품질] 재지 못했습니다 — 갈무리해 둔 답이 없고 Ollama 에 붙지도 못했습니다.
\n                 Ollama 를 켜고 `ollama pull {MODEL}` 을 한 뒤 다시 돌리세요."
            );
            return;
        };

        let hit = r.evidence_ids.iter().any(|id| want.contains(id));
        let cited_right = r.cited_ids.iter().any(|id| want.contains(id));
        let answered = matches!(
            r.decision,
            refuse::Decision::Answer | refuse::Decision::Limited
        );

        for t in [
            Some(&mut pos),
            by_doc.entry(q.document_id).or_default().into(),
            by_type.entry(q.question_type.clone()).or_default().into(),
        ]
        .into_iter()
        .flatten()
        {
            t.n += 1;
            t.ms += r.llm_ms;
            if r.fresh {
                t.fresh += 1;
            }
            if hit {
                t.evidence_hit += 1;
            }
            if r.citations_ok {
                t.citations_ok += 1;
            }
            if r.refused_by_support {
                t.refused_by_support += 1;
            }
            if answered {
                for (k, min) in COV.iter().enumerate() {
                    if r.gap.coverage() < *min {
                        t.cov_below[k] += 1;
                    }
                }
            }
            t.unsupported += r.unsupported;
            t.numbers_total += r.numbers_total;
            t.numbers_missing += r.numbers_missing;
            if r.parse_error.is_some() {
                t.parse_failed += 1;
            }
            if answered {
                if cited_right {
                    t.answered_right += 1;
                } else {
                    t.answered_wrong += 1;
                }
            } else {
                t.refused += 1;
            }
        }

        if !answered && hit {
            false_refusals.push((q, r.reason.clone()));
        }
        if answered && !cited_right {
            wrong_answers.push((q, r.answer.chars().take(60).collect()));
        }
    }

    // ── 답이 없는 물음 ──────────────────────────────────────────────
    let mut neg = Tally::default();
    let mut false_answers: Vec<(&Negative, String)> = Vec::new();

    for q in &all.negatives {
        let Some(r) = run_one(&conn, &vs, &q.question_id, &q.question, q.collection_id) else {
            println!("
[답변 품질] 답이 없는 물음을 재지 못했습니다 (모델에 붙지 못함).");
            return;
        };
        let answered = matches!(
            r.decision,
            refuse::Decision::Answer | refuse::Decision::Limited
        );
        neg.n += 1;
        neg.ms += r.llm_ms;
        // **숫자를 지어낸 답이 가장 위험하다.** 사람이 그대로 옮겨 쓰기 때문이다.
        // 값 없이 두루뭉술하게 넘어간 답과는 해가 다르다.
        neg.numbers_total += r.numbers_total;
        neg.numbers_missing += r.numbers_missing;
        if r.refused_by_support {
            neg.refused_by_support += 1;
        }
        if answered {
            for (k, min) in COV.iter().enumerate() {
                if r.gap.coverage() < *min {
                    neg.cov_below[k] += 1;
                }
            }
        }
        neg.unsupported += r.unsupported;
        if r.fresh {
            neg.fresh += 1;
        }
        if answered {
            neg.answered_wrong += 1;
            false_answers.push((q, r.answer.chars().take(70).collect()));
        } else {
            neg.refused += 1;
        }
    }

    // ── 표 ──────────────────────────────────────────────────────────
    println!("\n[답이 있는 물음 {}개]", pos.n);
    println!("  정답 근거를 근거로 넘긴 비율   {:>5.1}%  ({}/{})", pos.pct(pos.evidence_hit), pos.evidence_hit, pos.n);
    println!("  정답 근거를 인용해 답한 비율   {:>5.1}%  ({}/{})", pos.pct(pos.answered_right), pos.answered_right, pos.n);
    println!("  엉뚱한 근거로 답한 비율        {:>5.1}%  ({}/{})", pos.pct(pos.answered_wrong), pos.answered_wrong, pos.n);
    println!("  잘못 거부한 비율(False Refusal) {:>4.1}%  ({}/{})", pos.pct(pos.refused), pos.refused, pos.n);
    println!("  인용이 온전한 비율             {:>5.1}%  ({}/{})", pos.pct(pos.citations_ok), pos.citations_ok, pos.n);
    println!("  형식을 못 읽은 답             {:>5.1}%  ({}/{})", pos.pct(pos.parse_failed), pos.parse_failed, pos.n);
    println!("  인용한 근거에서 확인되지 않은 주장 {}개", pos.unsupported);
    println!(
        "  숫자 확인                      {}개 가운데 {}개 확인 안 됨",
        pos.numbers_total,
        pos.numbers_missing
    );
    println!(
        "  ── 주장 뒷받침 검사를 빼면 ─ 잘못 거부 {:.1}% → {:.1}%  (이 검사가 {}건을 거부했다)",
        pos.pct(pos.refused),
        pos.pct(pos.refused.saturating_sub(pos.refused_by_support)),
        pos.refused_by_support
    );
    print!("  ── 물음 핵심어 덮임으로 자르면 ─ 잘못 거부 {:.1}%", pos.pct(pos.refused));
    for (k, min) in COV.iter().enumerate() {
        print!(" · <{min:.2} → {:.1}%", pos.pct(pos.refused + pos.cov_below[k]));
    }
    println!();

    println!("\n[답이 없는 물음 {}개]", neg.n);
    println!("  올바르게 거부한 비율           {:>5.1}%  ({}/{})", neg.pct(neg.refused), neg.refused, neg.n);
    println!("  지어내 답한 비율(False Answer) {:>5.1}%  ({}/{})", neg.pct(neg.answered_wrong), neg.answered_wrong, neg.n);
    println!(
        "  답하면서 숫자를 들이댄 것        {}개 · 그 가운데 근거에 없는 숫자 {}개",
        neg.numbers_total, neg.numbers_missing
    );
    println!(
        "  ── 주장 뒷받침 검사를 빼면 ─ 지어내 답함 {:.1}% → {:.1}%  (이 검사가 {}건을 잡았다)",
        neg.pct(neg.answered_wrong),
        neg.pct(neg.answered_wrong + neg.refused_by_support),
        neg.refused_by_support
    );
    print!("  ── 물음 핵심어 덮임으로 자르면 ─ 지어내 답함 {:.1}%", neg.pct(neg.answered_wrong));
    for (k, min) in COV.iter().enumerate() {
        print!(
            " · <{min:.2} → {:.1}%",
            neg.pct(neg.answered_wrong.saturating_sub(neg.cov_below[k]))
        );
    }
    println!();

    println!("\n  자료별 (근거 도달 / 정답 인용 / 거부)");
    for d in &corpus.documents {
        if let Some(t) = by_doc.get(&d.id) {
            println!(
                "    {:<12} {:>5.1}% {:>5.1}% {:>5.1}%   ({}문항)",
                d.title.chars().take(11).collect::<String>(),
                t.pct(t.evidence_hit),
                t.pct(t.answered_right),
                t.pct(t.refused),
                t.n
            );
        }
    }
    println!("\n  물음 유형별 (근거 도달 / 정답 인용 / 거부)");
    for (code, name) in TYPE_NAME {
        if let Some(t) = by_type.get(code) {
            println!(
                "    {name:<10} {:>5.1}% {:>5.1}% {:>5.1}%   ({}문항)",
                t.pct(t.evidence_hit),
                t.pct(t.answered_right),
                t.pct(t.refused),
                t.n
            );
        }
    }

    let total_ms = pos.ms + neg.ms;
    let total_n = pos.n + neg.n;
    println!(
        "\n  한 물음에 걸린 시간 평균 {:.1}초 (새로 물은 것 {}개)",
        total_ms as f64 / total_n as f64 / 1000.0,
        pos.fresh + neg.fresh
    );

    if !false_answers.is_empty() {
        println!("\n[지어낸 답 — 가장 나쁜 고장]");
        for (q, a) in &false_answers {
            println!("  {} {}\n      없는 것: {}\n      답: {}", q.question_id, q.question, q.absent, a);
        }
    }
    if !false_refusals.is_empty() {
        println!("\n[잘못 거부 — 근거는 넘어갔는데 답하지 않음]");
        for (q, why) in &false_refusals {
            println!("  {} {}
      까닭: {}", q.question_id, q.question, why);
        }
    }
    if !wrong_answers.is_empty() {
        println!("\n[엉뚱한 근거로 답함]");
        for (q, a) in wrong_answers.iter().take(8) {
            println!("  {} {}\n      답: {}", q.question_id, q.question, a);
        }
    }

    // 내려가면 안 되는 선. 실제로 잰 값보다 넉넉히 아래에 둔다.
    assert!(
        pos.pct(pos.evidence_hit) >= 60.0,
        "정답 근거가 근거로 넘어가는 비율이 {:.1}% 로 떨어졌습니다",
        pos.pct(pos.evidence_hit)
    );
    assert!(
        neg.pct(neg.refused) >= 40.0,
        "자료에 없는 물음을 거부하는 비율이 {:.1}% 로 떨어졌습니다",
        neg.pct(neg.refused)
    );
}
