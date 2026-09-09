//! 실제 업무자료 3종으로 검색 품질을 **숫자로** 잰다 (P4b).
//!
//!     cargo test --lib eval -- --nocapture
//!
//! 재는 것은 세 가지다.
//!   - 낱말(keyword) : FTS5 trigram + 조사 떼기 + 커버리지 + BM25
//!   - 뜻(semantic)  : 청크 벡터와 물음 벡터의 코사인
//!   - 섞기(hybrid)  : 위 둘을 순위로 섞기(RRF)
//!
//! **정답은 사람이 원문을 읽고 고른 문장이다.** `test/golden/questions.json` 에
//! 문장으로 적어 두고, `npm run golden:build` 가 그때의 청크 번호로 옮긴다.
//! 그래서 청크 나누는 기준을 손봐도 골든 셋은 그대로 쓸 수 있다.
//!
//! 뜻·섞기는 청크 벡터가 있어야 한다. 벡터는 Ollama 가 한 번 만들어
//! `test/golden/vectors/` 에 넣어 둔 것을 읽는다(`npm run golden:embed`).
//! 없으면 낱말만 재고 왜 못 쟀는지 말한다 — 조용히 건너뛰지 않는다.

use crate::db::schema::MIGRATIONS;
use crate::repo::search::{keyword_search, Request};
use crate::repo::{hybrid, vector};
use rusqlite::Connection;
use serde::Deserialize;
use std::collections::HashMap;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../test/golden");

// ── 자료 읽기 ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct Corpus {
    collections: Vec<Coll>,
    documents: Vec<Doc>,
}

#[derive(Deserialize)]
struct Coll {
    id: i64,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Doc {
    id: i64,
    collection_id: i64,
    title: String,
    filename: String,
    page_count: i64,
    chunks: Vec<Chunk>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Chunk {
    ord: i64,
    heading_path: String,
    text: String,
    text_norm: String,
    kind: String,
    page_start: i64,
    page_end: i64,
    spans: Vec<Span>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Span {
    page: i64,
    char_start: i64,
    char_end: i64,
}

#[derive(Deserialize)]
struct Golden {
    questions: Vec<Question>,
}

#[derive(Deserialize, Clone)]
struct Question {
    question_id: String,
    document_id: i64,
    collection_id: i64,
    question: String,
    question_type: String,
    expected_pages: Vec<i64>,
    expected_chunk_ids: Vec<i64>,
    #[allow(dead_code)]
    evidence: Vec<String>,
    kinds: Vec<String>,
}

/// 벡터 꾸러미. 청크 벡터와 물음 벡터를 함께 담는다.
struct Vectors {
    model: String,
    dim: usize,
    /// (문서 id, 청크 ord) -> 벡터
    chunks: HashMap<(i64, i64), Vec<f32>>,
    /// 물음 번호 -> 벡터
    questions: HashMap<String, Vec<f32>>,
}

#[derive(Deserialize)]
struct Manifest {
    model: String,
    dim: usize,
    /// [문서 id, 청크 ord] 를 담은 목록. 아래 .bin 의 앞쪽 순서와 같다
    chunks: Vec<[i64; 2]>,
    /// 물음 번호 목록. .bin 의 뒤쪽 순서와 같다
    questions: Vec<String>,
}

fn load_corpus() -> Corpus {
    let path = format!("{ROOT}/chunks.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("{path} 를 읽지 못했습니다 ({e}). `npm run golden:analyze` 를 먼저 돌리세요.")
    });
    serde_json::from_str(&raw).unwrap()
}

fn load_golden() -> Vec<Question> {
    let path = format!("{ROOT}/golden.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("{path} 를 읽지 못했습니다 ({e}). `npm run golden:build` 를 먼저 돌리세요.")
    });
    let g: Golden = serde_json::from_str(&raw).unwrap();
    g.questions
}

/// 만들어 둔 벡터 꾸러미를 **모두** 읽는다. 검색 모델을 견주려면 여러 벌이 필요하다.
fn load_all_vectors() -> Result<Vec<Vectors>, String> {
    let dir = format!("{ROOT}/vectors");
    let entries = std::fs::read_dir(&dir)
        .map_err(|_| format!("{dir} 가 없습니다. `npm run golden:embed` 로 벡터를 먼저 만드세요."))?;
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".json"))
        .collect();
    names.sort();
    if names.is_empty() {
        return Err(format!("{dir} 에 벡터가 없습니다. `npm run golden:embed` 를 돌리세요."));
    }
    let mut out = Vec::new();
    for n in &names {
        out.push(read_vectors(&dir, n)?);
    }
    Ok(out)
}

/// 벡터 한 벌을 읽는다. 없으면 왜 없는지 돌려준다.
fn load_vectors() -> Result<Vectors, String> {
    let dir = format!("{ROOT}/vectors");
    let entries = std::fs::read_dir(&dir)
        .map_err(|_| format!("{dir} 가 없습니다. `npm run golden:embed` 로 벡터를 먼저 만드세요."))?;
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".json"))
        .collect();
    names.sort();
    let name = names
        .first()
        .ok_or_else(|| format!("{dir} 에 벡터가 없습니다. `npm run golden:embed` 를 돌리세요."))?;

    read_vectors(&dir, name)
}

fn read_vectors(dir: &str, name: &str) -> Result<Vectors, String> {
    let manifest: Manifest = serde_json::from_str(
        &std::fs::read_to_string(format!("{dir}/{name}")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let bin = std::fs::read(format!("{dir}/{}", name.replace(".json", ".bin")))
        .map_err(|e| format!("벡터 파일을 읽지 못했습니다: {e}"))?;

    let need = (manifest.chunks.len() + manifest.questions.len()) * manifest.dim * 4;
    if bin.len() != need {
        return Err(format!(
            "벡터 파일 크기가 맞지 않습니다 ({} 바이트, {need} 바이트여야 함). 다시 만드세요.",
            bin.len()
        ));
    }

    let mut at = 0usize;
    let mut take = || {
        let end = at + manifest.dim * 4;
        let v: Vec<f32> = bin[at..end]
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        at = end;
        v
    };

    let mut chunks = HashMap::new();
    for key in &manifest.chunks {
        chunks.insert((key[0], key[1]), take());
    }
    let mut questions = HashMap::new();
    for id in &manifest.questions {
        questions.insert(id.clone(), take());
    }

    Ok(Vectors {
        model: manifest.model,
        dim: manifest.dim,
        chunks,
        questions,
    })
}

/// 골든 자료를 담은 DB 를 만든다. (문서 id, ord) -> chunk.id 짝도 돌려준다.
fn build(corpus: &Corpus, vectors: Option<&Vectors>) -> (Connection, HashMap<(i64, i64), i64>) {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for (_, sql) in MIGRATIONS {
        conn.execute_batch(sql).unwrap();
    }
    for c in &corpus.collections {
        conn.execute(
            "INSERT INTO collection(id, name, created_at) VALUES (?1, ?2, '2026-09-09')",
            rusqlite::params![c.id, c.name],
        )
        .unwrap();
    }
    let mut ids = HashMap::new();
    for d in &corpus.documents {
        conn.execute(
            "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size,
                                  page_count, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, 'ok', '2026-09-09')",
            rusqlite::params![d.id, d.collection_id, d.title, d.filename, d.title, d.page_count],
        )
        .unwrap();
        for ch in &d.chunks {
            conn.execute(
                "INSERT INTO chunk(document_id, ord, heading_path, text, text_norm, kind,
                                   page_start, page_end, hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    d.id, ch.ord, ch.heading_path, ch.text, ch.text_norm, ch.kind,
                    ch.page_start, ch.page_end,
                    crate::repo::chunk::hash_of(&ch.text_norm)
                ],
            )
            .unwrap();
            let cid = conn.last_insert_rowid();
            ids.insert((d.id, ch.ord), cid);
            for s in &ch.spans {
                conn.execute(
                    "INSERT INTO chunk_span(chunk_id, page, char_start, char_end)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![cid, s.page, s.char_start, s.char_end],
                )
                .unwrap();
            }
            if let Some(vs) = vectors {
                if let Some(v) = vs.chunks.get(&(d.id, ch.ord)) {
                    let hash = crate::repo::chunk::hash_of(&ch.text_norm);
                    vector::save(&conn, cid, &vs.model, &hash, v).unwrap();
                }
            }
        }
    }
    (conn, ids)
}

// ── 지표 ─────────────────────────────────────────────────────────────

#[derive(Default, Clone)]
struct Score {
    n: usize,
    hit1: usize,
    hit5: usize,
    hit10: usize,
    rr: f64,
    ms: f64,
}

impl Score {
    fn add(&mut self, rank: Option<usize>, ms: f64) {
        self.n += 1;
        self.ms += ms;
        if let Some(r) = rank {
            if r == 1 {
                self.hit1 += 1;
            }
            if r <= 5 {
                self.hit5 += 1;
            }
            if r <= 10 {
                self.hit10 += 1;
            }
            self.rr += 1.0 / r as f64;
        }
    }
    fn pct(&self, k: usize) -> f64 {
        if self.n == 0 { 0.0 } else { k as f64 * 100.0 / self.n as f64 }
    }
    fn p1(&self) -> f64 { self.pct(self.hit1) }
    fn r5(&self) -> f64 { self.pct(self.hit5) }
    fn r10(&self) -> f64 { self.pct(self.hit10) }
    fn mrr(&self) -> f64 { if self.n == 0 { 0.0 } else { self.rr / self.n as f64 } }
    fn avg_ms(&self) -> f64 { if self.n == 0 { 0.0 } else { self.ms / self.n as f64 } }
    fn line(&self, label: &str) -> String {
        format!(
            "{label:<12} P@1 {:>5.1}%  R@5 {:>5.1}%  R@10 {:>5.1}%  MRR {:>5.3}  ({}문항, 평균 {:.1}ms)",
            self.p1(), self.r5(), self.r10(), self.mrr(), self.n, self.avg_ms()
        )
    }
}

/// 정답이 몇 등에 나왔는가. 정답 청크가 여럿이면 **가장 위에 나온 것**을 본다.
fn first_rank(order: &[i64], want: &[i64]) -> Option<usize> {
    order.iter().position(|id| want.contains(id)).map(|i| i + 1)
}

// ── 검색 한 판 ───────────────────────────────────────────────────────

const TOP: i64 = 10;

struct Run {
    order: Vec<i64>,
    ms: f64,
}

fn run_keyword(conn: &Connection, q: &Question, collections: Vec<i64>) -> Run {
    let t = std::time::Instant::now();
    let res = keyword_search(
        conn,
        &Request { text: q.question.clone(), collection_ids: collections, limit: TOP },
    )
    .unwrap();
    Run {
        order: res.hits.iter().map(|h| h.chunk_id).collect(),
        ms: t.elapsed().as_secs_f64() * 1000.0,
    }
}

fn run_semantic(conn: &Connection, v: &[f32], model: &str, collections: Vec<i64>) -> Run {
    let t = std::time::Instant::now();
    let (scored, _) = vector::nearest(conn, v, model, &collections, TOP as usize).unwrap();
    Run {
        order: scored.iter().map(|(id, _)| *id).collect(),
        ms: t.elapsed().as_secs_f64() * 1000.0,
    }
}

fn run_hybrid(conn: &Connection, q: &Question, v: &[f32], model: &str, collections: Vec<i64>, depth: i64) -> Run {
    let t = std::time::Instant::now();
    let res = hybrid::hybrid_search(
        conn,
        &hybrid::Hybrid {
            text: &q.question,
            query_vec: v,
            model,
            collection_ids: collections,
            limit: TOP,
            depth,
        },
    )
    .unwrap();
    Run {
        order: res.hits.iter().map(|h| h.chunk_id).collect(),
        ms: t.elapsed().as_secs_f64() * 1000.0,
    }
}

/// 골든 셋의 정답 청크 번호(문서 안 ord)를 이 DB 의 chunk.id 로 옮긴다
fn wanted(q: &Question, ids: &HashMap<(i64, i64), i64>) -> Vec<i64> {
    q.expected_chunk_ids
        .iter()
        .filter_map(|o| ids.get(&(q.document_id, *o)).copied())
        .collect()
}

const TYPE_NAME: [(&str, &str); 6] = [
    ("A", "직접 표현"),
    ("B", "조사 변화"),
    ("C", "바꿔쓰기"),
    ("D", "숫자 표기"),
    ("E", "조건·규정"),
    ("F", "구조·요약"),
];

#[cfg(test)]
#[path = "run.rs"]
mod run;
