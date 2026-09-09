//! 점수만 정해 온 청크 목록을 **화면에 보여 줄 꼴**로 채운다.
//!
//! 낱말 검색은 SQL 한 판으로 순위까지 정하지만, 의미 검색과 섞기(RRF)는
//! "어느 청크가 몇 점" 만 손에 들고 있다. 거기서 제목·쪽·문자 구간을 다시
//! 붙여 주는 자리가 여기다.
//!
//! **자료집 조건과 대체된 문서 걸러내기를 여기서도 한다.** 점수를 낸 쪽에서
//! 이미 걸렀더라도, 근거를 만드는 마지막 자리에서 한 번 더 본다 — 지난 문서가
//! 근거로 나가는 것이 가장 나쁜 고장이다.

use super::search::{spans_of, Hit};
use crate::error::AppResult;
use rusqlite::{Connection, ToSql};

/// `scored` 는 (청크 id, 점수). 앞에 있는 것이 위에 온다 — 여기서 다시
/// 정렬하지 않는다. 순위를 정하는 일은 부르는 쪽 몫이다.
pub fn fill(
    conn: &Connection,
    scored: &[(i64, f64)],
    collections: &[i64],
    limit: i64,
) -> AppResult<Vec<Hit>> {
    if scored.is_empty() {
        return Ok(vec![]);
    }

    let holes = scored.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let coll = if collections.is_empty() {
        String::new()
    } else {
        let h = collections.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        format!(" AND d.collection_id IN ({h})")
    };
    let sql = format!(
        "SELECT c.id, c.document_id, d.collection_id, d.title, c.ord, c.heading_path,
                c.text, c.kind, c.page_start, c.page_end
           FROM chunk c JOIN document d ON d.id = c.document_id
          WHERE c.id IN ({holes}) AND d.superseded_by IS NULL{coll}"
    );

    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    for (id, _) in scored {
        params.push(Box::new(*id));
    }
    for id in collections {
        params.push(Box::new(*id));
    }

    struct Loaded {
        chunk_id: i64,
        document_id: i64,
        collection_id: i64,
        doc_title: String,
        ord: i64,
        heading_path: Option<String>,
        text: String,
        kind: String,
        page_start: i64,
        page_end: i64,
    }

    let mut st = conn.prepare(&sql)?;
    let loaded = st
        .query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |r| {
                Ok(Loaded {
                    chunk_id: r.get(0)?,
                    document_id: r.get(1)?,
                    collection_id: r.get(2)?,
                    doc_title: r.get(3)?,
                    ord: r.get(4)?,
                    heading_path: r.get(5)?,
                    text: r.get(6)?,
                    kind: r.get(7)?,
                    page_start: r.get(8)?,
                    page_end: r.get(9)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;

    let mut hits = Vec::new();
    for (id, score) in scored {
        let Some(row) = loaded.iter().find(|l| l.chunk_id == *id) else {
            continue; // 걸러진 청크
        };
        hits.push(Hit {
            rank: hits.len() as i64 + 1,
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
            matched: 0,
            matched_terms: vec![],
            bm25: 0.0,
            score: *score,
        });
        if hits.len() as i64 >= limit.max(1) {
            break;
        }
    }
    Ok(hits)
}
