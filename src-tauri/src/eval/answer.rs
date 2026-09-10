//! 답변 품질을 **숫자로** 잰다 (P5 · P5b).
//!
//!     cargo test --lib eval::answer -- --nocapture
//!     DOCAID_EVAL_MODEL=qwen3:8b cargo test --lib eval::answer -- --nocapture
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
//! 표는 **두 판**을 나란히 찍는다 — P5 판단 그대로, 그리고 P5b 의 초점 낱말
//! 규칙(A1+B1)을 더한 것. 모델의 답은 한 번만 받아 두 판에 같이 쓰므로, 규칙의
//! 효과와 모델의 효과를 갈라 볼 수 있다.
//!
//! ## 답을 갈무리해 둔다
//!
//! 4B 모델이 이 PC(CPU)에서 한 물음에 20~110초 걸린다. 그래서 모델이 내놓은 글을
//! `test/golden/answers/<모델>/` 에 갈무리해 두고, 물음과 근거가 그대로면 다시
//! 부르지 않는다. 갈무리를 저장소에 넣어 두면 Ollama 가 없는 곳(CI)에서도 같은
//! 평가를 다시 돌릴 수 있다 — 벡터와 같은 방식이다.

use super::*;
use crate::ai::ollama;
use crate::answer::{context, focus, parse, prompt, refuse, verify};
use crate::repo::{chunk, hybrid};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// 재는 데 쓸 답변 모델. 기본은 카탈로그의 가벼운 쪽이다.
/// 큰 쪽으로 재려면 `DOCAID_EVAL_MODEL=qwen3:8b cargo test …`. 갈무리는 모델별로 나뉜다.
fn model() -> String {
    std::env::var("DOCAID_EVAL_MODEL").unwrap_or_else(|_| "gemma3:4b".to_string())
}

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
    let stem = model().replace(':', "-");
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
            if c.prompt_hash == want && c.model == model() {
                return Ok((c.raw, c.llm_ms, false));
            }
        }
    }

    let out = ollama::chat_stream(
        &model(),
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
        model: model(),
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
    /// P5 판단 (초점 낱말 규칙을 더하기 전)
    pub(super) decision: refuse::Decision,
    /// 근거로 넘어간 청크 id
    pub(super) evidence_ids: Vec<i64>,
    /// LLM 에게 실제로 넘긴 근거의 원문
    pub(super) evidence_texts: Vec<String>,
    /// 답변이 인용한 청크 id
    pub(super) cited_ids: Vec<i64>,
    pub(super) citations_ok: bool,
    pub(super) numbers_total: usize,
    pub(super) numbers_missing: usize,
    pub(super) llm_ms: u64,
    pub(super) fresh: bool,
    /// **주장 뒷받침 검사** 때문에 거부했는가.
    pub(super) refused_by_support: bool,
    /// 뒷받침되지 않은 주장 수
    pub(super) unsupported: usize,
    /// 초점 낱말 검사 (P5b). 앱과 같은 자로 잰다.
    pub(super) focus: focus::FocusCheck,
    /// 왜 그렇게 판단했는지 (첫 까닭)
    pub(super) reason: String,
    /// 형식을 못 읽었으면 그 까닭
    pub(super) parse_error: Option<String>,
    pub(super) answer: String,
}

impl Ran {
    /// 초점 낱말 규칙(A1+B1)까지 더한 판단. 앱은 이 경우 모델을 부르지 않는다.
    pub(super) fn decision_with_focus(&self) -> refuse::Decision {
        if self.focus.refusal().is_some() {
            refuse::Decision::Refuse
        } else {
            self.decision
        }
    }
}

fn answered(d: refuse::Decision) -> bool {
    matches!(d, refuse::Decision::Answer | refuse::Decision::Limited)
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
    let evidence_texts: Vec<String> = evidence.iter().map(|e| e.text.clone()).collect();

    // ②′ 초점 낱말 — 앱과 같은 자. 여기서는 모델에게도 **물어 둔다** — 규칙을
    // 뺀 판(P5 그대로)도 같은 답으로 함께 재기 위해서다.
    let mut fc = focus::FocusCheck::before_answer(
        question,
        |w| chunk::any_contains(conn, &[collection_id], &focus::needles(w)).unwrap_or(true),
        &evidence_texts,
    );

    if evidence.is_empty() {
        return Some(Ran {
            decision: refuse::Decision::Refuse,
            evidence_ids,
            evidence_texts,
            cited_ids: vec![],
            citations_ok: false,
            numbers_total: 0,
            numbers_missing: 0,
            llm_ms: 0,
            fresh: false,
            refused_by_support: false,
            unsupported: 0,
            focus: fc,
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
                evidence_texts,
                cited_ids: vec![],
                citations_ok: false,
                numbers_total: 0,
                numbers_missing: 0,
                llm_ms,
                fresh,
                refused_by_support: false,
                unsupported: 0,
                focus: fc,
                reason: String::new(),
                parse_error: Some(e),
                answer: String::new(),
            });
        }
    };

    // ⑥ 검증 ⑦ 판단
    let verdict = verify::verify(&draft, &evidence);
    // 모델의 insufficientEvidence 를 믿는지는 카탈로그가 안다 — 앱과 같은 자
    let trust = crate::ai::catalog::by_tag(&model()).is_some_and(|m| m.trusts_insufficient);
    let mut judgement = refuse::decide(&draft, &verdict, evidence.len(), found.hits.len(), trust);

    let cited_ids: Vec<i64> = verdict.sources.iter().filter_map(|s| s.chunk_id).collect();
    let cited_texts: Vec<String> = cited_ids
        .iter()
        .filter_map(|c| evidence.iter().find(|e| e.chunk_id == *c))
        .map(|e| e.text.clone())
        .collect();
    // C — 앱과 같이 경고만 (판단은 제한적으로 답함)
    fc.after_answer(question, &cited_texts);
    if let Some(w) = fc.warning() {
        if judgement.decision == refuse::Decision::Answer {
            judgement.decision = refuse::Decision::Limited;
        }
        judgement.reasons.push(w);
    }

    Some(Ran {
        refused_by_support: judgement.decision == refuse::Decision::Refuse
            && verdict.nothing_supported(),
        unsupported: verdict.unsupported_claims().len(),
        decision: judgement.decision,
        evidence_ids,
        evidence_texts,
        cited_ids,
        citations_ok: verdict.citations_ok,
        numbers_total: verdict.numbers.len(),
        numbers_missing: verdict.numbers_missing(),
        llm_ms,
        fresh,
        focus: fc,
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
    /// 주장 뒷받침 검사 때문에 거부한 것
    refused_by_support: usize,
    /// 초점 낱말 규칙(A1+B1) 때문에 거부한 것 (이 판에서 규칙을 켠 경우)
    refused_by_focus: usize,
    /// 인용한 근거에서 확인되지 않은 주장 수 (모두 더한 것)
    unsupported: usize,
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

/// 한 판을 셈하고 찍는다. `with_focus` 가 true 면 초점 낱말 규칙(A1+B1)을 더한 판단으로 센다.
struct Report {
    pos: Tally,
    neg: Tally,
}

fn report(
    title: &str,
    corpus: &Corpus,
    ids: &HashMap<(i64, i64), i64>,
    pos_runs: &[(&Question, Ran)],
    neg_runs: &[(&Negative, Ran)],
    with_focus: bool,
) -> Report {
    let decide = |r: &Ran| if with_focus { r.decision_with_focus() } else { r.decision };

    let mut pos = Tally::default();
    let mut by_doc: HashMap<i64, Tally> = HashMap::new();
    let mut by_type: HashMap<String, Tally> = HashMap::new();
    let mut false_refusals: Vec<(&Question, String)> = Vec::new();
    let mut wrong_answers: Vec<(&Question, String)> = Vec::new();

    for (q, r) in pos_runs {
        let want = wanted(q, ids);
        let hit = r.evidence_ids.iter().any(|id| want.contains(id));
        let cited_right = r.cited_ids.iter().any(|id| want.contains(id));
        let d = decide(r);
        let ans = answered(d);
        let by_focus = with_focus && answered(r.decision) && !ans;

        for t in [
            Some(&mut pos),
            by_doc.entry(q.document_id).or_default().into(),
            by_type.entry(q.question_type.clone()).or_default().into(),
        ]
        .into_iter()
        .flatten()
        {
            t.n += 1;
            // 규칙이 막은 물음은 앱에서 모델을 부르지 않는다 — 시간도 그렇게 센다
            t.ms += if by_focus { 0 } else { r.llm_ms };
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
            if by_focus {
                t.refused_by_focus += 1;
            }
            t.unsupported += r.unsupported;
            t.numbers_total += r.numbers_total;
            t.numbers_missing += r.numbers_missing;
            if r.parse_error.is_some() {
                t.parse_failed += 1;
            }
            if ans {
                if cited_right {
                    t.answered_right += 1;
                } else {
                    t.answered_wrong += 1;
                }
            } else {
                t.refused += 1;
            }
        }

        if !ans && hit {
            let why = if by_focus { r.focus.refusal().unwrap_or_default() } else { r.reason.clone() };
            false_refusals.push((q, why));
        }
        if ans && !cited_right {
            wrong_answers.push((q, r.answer.chars().take(60).collect()));
        }
    }

    let mut neg = Tally::default();
    let mut false_answers: Vec<(&Negative, String, String)> = Vec::new();
    let mut caught: Vec<(&Negative, String)> = Vec::new();

    for (q, r) in neg_runs {
        let d = decide(r);
        let ans = answered(d);
        let by_focus = with_focus && answered(r.decision) && !ans;
        neg.n += 1;
        neg.ms += if by_focus { 0 } else { r.llm_ms };
        neg.numbers_total += r.numbers_total;
        neg.numbers_missing += r.numbers_missing;
        if r.refused_by_support {
            neg.refused_by_support += 1;
        }
        if by_focus {
            neg.refused_by_focus += 1;
            caught.push((q, r.focus.refusal().unwrap_or_default()));
        }
        neg.unsupported += r.unsupported;
        if r.fresh {
            neg.fresh += 1;
        }
        if ans {
            neg.answered_wrong += 1;
            false_answers.push((q, r.answer.chars().take(70).collect(), r.reason.clone()));
        } else {
            neg.refused += 1;
        }
    }

    // ── 표 ──────────────────────────────────────────────────────────
    println!("\n══ {title} ══");
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
        pos.numbers_total, pos.numbers_missing
    );
    if with_focus {
        println!("  초점 낱말 규칙이 거부한 positive {}개", pos.refused_by_focus);
    }

    println!("\n[답이 없는 물음 {}개]", neg.n);
    println!("  올바르게 거부한 비율           {:>5.1}%  ({}/{})", neg.pct(neg.refused), neg.refused, neg.n);
    println!("  지어내 답한 비율(False Answer) {:>5.1}%  ({}/{})", neg.pct(neg.answered_wrong), neg.answered_wrong, neg.n);
    println!(
        "  답하면서 숫자를 들이댄 것        {}개 · 그 가운데 근거에 없는 숫자 {}개",
        neg.numbers_total, neg.numbers_missing
    );
    if with_focus {
        println!("  초점 낱말 규칙이 막은 negative {}개", neg.refused_by_focus);
        for (q, why) in &caught {
            println!("    {} {} ← {}", q.question_id, q.question, why);
        }
    }

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
        "\n  한 물음에 걸린 시간 평균 {:.1}초{}",
        total_ms as f64 / total_n as f64 / 1000.0,
        if with_focus { " (규칙이 막은 물음은 모델을 부르지 않으므로 0초로 센다)" } else { "" }
    );

    if !false_answers.is_empty() {
        println!("\n[지어낸 답 — 가장 나쁜 고장]");
        for (q, a, why) in &false_answers {
            println!(
                "  {} {}\n      없는 것: {}\n      답: {}{}",
                q.question_id,
                q.question,
                q.absent,
                a,
                if why.is_empty() { String::new() } else { format!("\n      판단: {why}") }
            );
        }
    }
    if !false_refusals.is_empty() {
        println!("\n[잘못 거부 — 근거는 넘어갔는데 답하지 않음]");
        for (q, why) in &false_refusals {
            println!("  {} {}\n      까닭: {}", q.question_id, q.question, why);
        }
    }
    if !wrong_answers.is_empty() {
        println!("\n[엉뚱한 근거로 답함]");
        for (q, a) in wrong_answers.iter().take(8) {
            println!("  {} {}\n      답: {}", q.question_id, q.question, a);
        }
    }

    Report { pos, neg }
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
        "\n── 답변 모델 {} · 검색 {} · 청크 {}개",
        model(),
        vs.model,
        corpus.documents.iter().map(|d| d.chunks.len()).sum::<usize>()
    );
    println!(
        "   답이 있는 물음 {}개 · 답이 없는 물음 {}개",
        all.questions.len(),
        all.negatives.len()
    );

    // 모델의 답은 한 번만 받는다 (갈무리를 먼저 본다)
    let mut pos_runs: Vec<(&Question, Ran)> = Vec::new();
    for q in &all.questions {
        let Some(r) = run_one(&conn, &vs, &q.question_id, &q.question, q.collection_id) else {
            println!(
                "\n[답변 품질] 재지 못했습니다 — 갈무리해 둔 답이 없고 Ollama 에 붙지도 못했습니다.\
                 \n                 Ollama 를 켜고 `ollama pull {}` 을 한 뒤 다시 돌리세요.",
                model()
            );
            return;
        };
        pos_runs.push((q, r));
    }
    let mut neg_runs: Vec<(&Negative, Ran)> = Vec::new();
    for q in &all.negatives {
        let Some(r) = run_one(&conn, &vs, &q.question_id, &q.question, q.collection_id) else {
            println!("\n[답변 품질] 답이 없는 물음을 재지 못했습니다 (모델에 붙지 못함).");
            return;
        };
        neg_runs.push((q, r));
    }
    println!(
        "   새로 물은 것 {}개",
        pos_runs.iter().filter(|(_, r)| r.fresh).count() + neg_runs.iter().filter(|(_, r)| r.fresh).count()
    );

    // 두 판 — 규칙 없이 / 규칙(A1+B1) 더해서
    let base = report("기존 P5 판단", &corpus, &ids, &pos_runs, &neg_runs, false);
    let with = report("초점 낱말 규칙(A1+B1) 더함 — 앱이 쓰는 판단", &corpus, &ids, &pos_runs, &neg_runs, true);

    println!(
        "\n══ 두 판 견주기 ══\n  지어내 답함  {:.1}% → {:.1}%\n  잘못 거부    {:.1}% → {:.1}%\n  정답 인용    {:.1}% → {:.1}%",
        base.neg.pct(base.neg.answered_wrong),
        with.neg.pct(with.neg.answered_wrong),
        base.pos.pct(base.pos.refused),
        with.pos.pct(with.pos.refused),
        base.pos.pct(base.pos.answered_right),
        with.pos.pct(with.pos.answered_right),
    );

    // 내려가면 안 되는 선. 실제로 잰 값보다 넉넉히 아래에 둔다.
    assert!(
        with.pos.pct(with.pos.evidence_hit) >= 60.0,
        "정답 근거가 근거로 넘어가는 비율이 {:.1}% 로 떨어졌습니다",
        with.pos.pct(with.pos.evidence_hit)
    );
    assert!(
        with.neg.pct(with.neg.refused) >= 40.0,
        "자료에 없는 물음을 거부하는 비율이 {:.1}% 로 떨어졌습니다",
        with.neg.pct(with.neg.refused)
    );
    // 규칙이 맞는 답을 버리기 시작하면 여기서 걸린다 — 골든 셋에서는 0건이었다
    assert!(
        with.pos.answered_right >= base.pos.answered_right.saturating_sub(1),
        "초점 낱말 규칙이 정답 답변을 {}건 버렸습니다",
        base.pos.answered_right - with.pos.answered_right
    );
}
