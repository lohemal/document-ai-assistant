//! 낱말로 찾기 (FTS5 trigram).
//!
//! **AI 모델이 하나도 없어도 도는 검색이다** (설계안 2-12). 뜻으로 찾기는
//! P4b 에서 이 위에 얹고, 두 결과를 RRF 로 합친다.
//!
//! 여기서 하는 일은 세 가지뿐이다.
//!   ① 사용자가 적은 말을 검색어로 바꾼다 (`domain::query`)
//!   ② FTS5 로 후보를 긁는다
//!   ③ 몇 낱말이 들어 있는지로 차례를 매긴다 (`domain::rank`)
//!
//! **이웃 청크를 붙이는 일은 여기서 하지 않는다.** 검색 성능을 따로 재려면
//! 검색과 문맥 확장이 섞이면 안 된다. 이웃은 `neighbors()` 로 따로 가져간다.

use crate::domain::{query, rank};
use crate::error::AppResult;
use crate::repo::chunk::Span;
use rusqlite::{Connection, ToSql};
use serde::Serialize;

/// 후보를 이만큼까지만 긁는다. 이 안에서 낱말 수로 차례를 다시 매긴다.
///
/// BM25 로 먼저 자르므로, 낱말이 골고루 든 청크가 이 밖에 있으면 놓친다.
/// 학교 자료 규모에서는 넉넉하다. 나중에 자료가 아주 많아지면 늘린다.
const CANDIDATE_CAP: i64 = 500;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    /// 1부터. 화면에 보여 주는 차례
    pub rank: i64,
    pub chunk_id: i64,
    pub document_id: i64,
    pub collection_id: i64,
    pub doc_title: String,
    pub ord: i64,
    pub heading_path: Option<String>,
    pub text: String,
    pub kind: String,
    pub page_start: i64,
    pub page_end: i64,
    pub spans: Vec<Span>,
    /// 찾는 낱말 가운데 몇 개가 들어 있었는가
    pub matched: i64,
    /// 어느 낱말이 걸렸는가
    pub matched_terms: Vec<String>,
    /// SQLite BM25. 작을수록 잘 맞는다. 개발용으로만 보여 준다
    pub bm25: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub hits: Vec<Hit>,
    /// 실제로 찾아 본 낱말들 (조사를 뗀 꼴 포함)
    pub terms: Vec<String>,
    /// 너무 짧아 색인으로 못 찾고 훑어서 찾은 낱말들
    pub short_terms: Vec<String>,
    /// 물음말이라 버린 낱말들
    pub dropped_terms: Vec<String>,
    /// 차례를 매기기 전 후보 수
    pub candidates: i64,
    pub elapsed_ms: i64,
    /// 찾을 낱말이 하나도 없었으면 왜 그런지
    pub note: Option<String>,
}

pub struct Request {
    pub text: String,
    /// 비어 있으면 모든 자료집에서 찾는다.
    /// 여러 개를 받도록 해 두어, 나중에 자료집 여러 개를 고를 수 있다.
    pub collection_ids: Vec<i64>,
    pub limit: i64,
}

struct Row {
    chunk_id: i64,
    document_id: i64,
    collection_id: i64,
    doc_title: String,
    ord: i64,
    heading_path: Option<String>,
    text: String,
    text_norm: String,
    kind: String,
    page_start: i64,
    page_end: i64,
    bm25: f64,
}

/// 자료집 조건을 SQL 조각으로 만든다. 빈 목록이면 조건이 없다.
fn collection_filter(ids: &[i64]) -> String {
    if ids.is_empty() {
        String::new()
    } else {
        let holes = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        format!(" AND d.collection_id IN ({holes})")
    }
}

const COLUMNS: &str = "c.id, c.document_id, d.collection_id, d.title, c.ord, c.heading_path,
                       c.text, c.text_norm, c.kind, c.page_start, c.page_end";

fn read_row(r: &rusqlite::Row, bm25_col: usize) -> rusqlite::Result<Row> {
    Ok(Row {
        chunk_id: r.get(0)?,
        document_id: r.get(1)?,
        collection_id: r.get(2)?,
        doc_title: r.get(3)?,
        ord: r.get(4)?,
        heading_path: r.get(5)?,
        text: r.get(6)?,
        text_norm: r.get(7)?,
        kind: r.get(8)?,
        page_start: r.get(9)?,
        page_end: r.get(10)?,
        bm25: r.get(bm25_col)?,
    })
}

/// 색인으로 찾는다 (세 글자 이상인 낱말)
fn by_index(
    conn: &Connection,
    expr: &str,
    collections: &[i64],
) -> AppResult<Vec<Row>> {
    let sql = format!(
        "SELECT {COLUMNS}, m.bm
           FROM (SELECT rowid AS rid, bm25(chunk_fts) AS bm
                   FROM chunk_fts WHERE chunk_fts MATCH ?1
                  ORDER BY bm LIMIT ?2) m
           JOIN chunk c ON c.id = m.rid
           JOIN document d ON d.id = c.document_id
          WHERE d.superseded_by IS NULL{}",
        collection_filter(collections)
    );

    let mut params: Vec<Box<dyn ToSql>> = vec![Box::new(expr.to_string()), Box::new(CANDIDATE_CAP)];
    for id in collections {
        params.push(Box::new(*id));
    }

    let mut st = conn.prepare(&sql)?;
    let rows = st
        .query_map(rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())), |r| {
            read_row(r, 11)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 색인으로 못 찾는 짧은 낱말은 훑어서 찾는다.
///
/// trigram 은 세 글자 아래를 색인하지 못한다. 그렇다고 "교재" 를 찾을 수
/// 없다고 하면 사용자는 프로그램이 고장 났다고 여긴다. 느리지만 훑는다.
fn by_scan(conn: &Connection, needles: &[String], collections: &[i64]) -> AppResult<Vec<Row>> {
    if needles.is_empty() {
        return Ok(vec![]);
    }
    let likes = needles
        .iter()
        .map(|_| "c.text_norm LIKE ?")
        .collect::<Vec<_>>()
        .join(" OR ");
    let sql = format!(
        "SELECT {COLUMNS}, 0.0
           FROM chunk c JOIN document d ON d.id = c.document_id
          WHERE d.superseded_by IS NULL AND ({likes}){}
          LIMIT {CANDIDATE_CAP}",
        collection_filter(collections)
    );

    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    for n in needles {
        params.push(Box::new(format!("%{}%", n.replace('%', "\\%"))));
    }
    for id in collections {
        params.push(Box::new(*id));
    }

    let mut st = conn.prepare(&sql)?;
    let rows = st
        .query_map(rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())), |r| {
            read_row(r, 11)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn spans_of(conn: &Connection, chunk_id: i64) -> AppResult<Vec<Span>> {
    let mut st = conn.prepare(
        "SELECT page, char_start, char_end FROM chunk_span
          WHERE chunk_id = ?1 ORDER BY page, char_start",
    )?;
    let rows = st
        .query_map([chunk_id], |r| {
            Ok(Span {
                page: r.get(0)?,
                char_start: r.get(1)?,
                char_end: r.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn keyword_search(conn: &Connection, req: &Request) -> AppResult<SearchResult> {
    let started = std::time::Instant::now();
    let parsed = query::parse(&req.text);

    let mut note = None;
    if parsed.terms.is_empty() && parsed.short.is_empty() {
        note = Some(if parsed.dropped.is_empty() {
            "찾을 낱말이 없습니다.".to_string()
        } else {
            format!(
                "'{}' 같은 물음말만 있어서 찾을 낱말이 없습니다. 핵심 낱말을 넣어 보세요.",
                parsed.dropped.join(", ")
            )
        });
    }

    // 색인으로 한 번, 짧은 낱말은 훑어서 한 번. 겹치는 것은 합친다.
    let mut rows: Vec<Row> = match query::fts_expression(&parsed) {
        Some(expr) => by_index(conn, &expr, &req.collection_ids)?,
        None => vec![],
    };
    let scanned = by_scan(conn, &parsed.short, &req.collection_ids)?;
    for s in scanned {
        if !rows.iter().any(|r| r.chunk_id == s.chunk_id) {
            rows.push(s);
        }
    }

    let candidates = rows.len() as i64;

    // 짧은 낱말도 "들어 있는가" 셈에는 넣는다
    let mut terms = parsed.terms.clone();
    for s in &parsed.short {
        terms.push(query::Term {
            raw: s.clone(),
            forms: vec![s.clone()],
        });
    }

    let cands: Vec<rank::Candidate> = rows
        .iter()
        .map(|r| rank::Candidate {
            chunk_id: r.chunk_id,
            text_norm: r.text_norm.clone(),
            bm25: r.bm25,
        })
        .collect();

    let ranked = rank::rank(&cands, &terms);

    let mut hits = Vec::new();
    for (i, rk) in ranked.iter().take(req.limit.max(1) as usize).enumerate() {
        let row = rows.iter().find(|r| r.chunk_id == rk.chunk_id).unwrap();
        hits.push(Hit {
            rank: i as i64 + 1,
            chunk_id: row.chunk_id,
            document_id: row.document_id,
            collection_id: row.collection_id,
            doc_title: row.doc_title.clone(),
            ord: row.ord,
            heading_path: row.heading_path.clone(),
            text: row.text.clone(),
            kind: row.kind.clone(),
            page_start: row.page_start,
            page_end: row.page_end,
            spans: spans_of(conn, row.chunk_id)?,
            matched: rk.matched as i64,
            matched_terms: rk.matched_terms.clone(),
            bm25: rk.bm25,
        });
    }

    Ok(SearchResult {
        hits,
        terms: parsed
            .terms
            .iter()
            .flat_map(|t| t.forms.clone())
            .collect(),
        short_terms: parsed.short.clone(),
        dropped_terms: parsed.dropped.clone(),
        candidates,
        elapsed_ms: started.elapsed().as_millis() as i64,
        note,
    })
}

/// 검색이 고른 청크의 이웃을 가져온다.
///
/// **검색 순위와는 아무 상관이 없다.** 검색은 청크 하나를 고르고, 문맥을
/// 넓히는 일은 그 뒤에 따로 한다. 섞으면 나중에 검색 품질을 따로 잴 수 없다.
/// P5 에서 근거를 LLM 에 넘길 때 쓴다.
pub fn neighbors(conn: &Connection, chunk_id: i64, radius: i64) -> AppResult<Vec<i64>> {
    let (doc, ord): (i64, i64) = conn.query_row(
        "SELECT document_id, ord FROM chunk WHERE id = ?1",
        [chunk_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut st = conn.prepare(
        "SELECT id FROM chunk WHERE document_id = ?1 AND ord BETWEEN ?2 AND ?3 ORDER BY ord",
    )?;
    let ids = st
        .query_map([doc, ord - radius, ord + radius], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
