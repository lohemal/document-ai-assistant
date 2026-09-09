//! 청크와 그 위치 읽고 쓰기.
//!
//! 청크 하나는 "이 문서의 이 쪽 이 글자부터 이 글자까지" 를 가리킨다.
//! 그 가리키는 값이 곧 형광펜이 칠할 자리다.

use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub page: i64,
    /// 그 쪽 텍스트 안의 시작 위치
    pub char_start: i64,
    pub char_end: i64,
}

/// 화면에서 넘어오는 청크
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkIn {
    pub ord: i64,
    pub heading_path: String,
    pub text: String,
    pub text_norm: String,
    /// text | table
    pub kind: String,
    pub page_start: i64,
    pub page_end: i64,
    pub spans: Vec<Span>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub id: i64,
    pub document_id: i64,
    pub ord: i64,
    pub heading_path: Option<String>,
    pub text: String,
    pub kind: String,
    pub page_start: i64,
    pub page_end: i64,
    pub spans: Vec<Span>,
}

/// 이 문서의 청크를 통째로 갈아 끼운다.
///
/// 더하지 않고 갈아 끼우는 까닭은, 나누는 규칙이 바뀌면 옛 청크가 남아
/// 같은 내용이 두 번 검색되기 때문이다.
pub fn replace_all(
    conn: &mut Connection,
    document_id: i64,
    chunks: &[ChunkIn],
) -> AppResult<usize> {
    let tx = conn.transaction()?;
    // chunk_span 과 embedding 은 chunk 를 따라 함께 지워진다 (ON DELETE CASCADE).
    // 임베딩이 사라지는 것은 맞다 — 글이 달라졌으니 다시 만들어야 한다.
    tx.execute("DELETE FROM chunk WHERE document_id = ?1", params![document_id])?;

    {
        let mut ins = tx.prepare(
            "INSERT INTO chunk(document_id, ord, heading_path, text, text_norm, kind,
                               page_start, page_end)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        let mut ins_span = tx.prepare(
            "INSERT INTO chunk_span(chunk_id, page, char_start, char_end) VALUES (?1, ?2, ?3, ?4)",
        )?;

        for c in chunks {
            if !matches!(c.kind.as_str(), "text" | "table") {
                return Err(AppError::msg(format!("알 수 없는 청크 종류입니다: {}", c.kind)));
            }
            if c.spans.is_empty() {
                return Err(AppError::msg(format!(
                    "청크 {}: 가리키는 자리가 없습니다. 이러면 근거를 보여 줄 수 없습니다.",
                    c.ord
                )));
            }
            ins.execute(params![
                document_id,
                c.ord,
                if c.heading_path.is_empty() { None } else { Some(&c.heading_path) },
                c.text,
                c.text_norm,
                c.kind,
                c.page_start,
                c.page_end,
            ])?;
            let chunk_id = tx.last_insert_rowid();
            for s in &c.spans {
                ins_span.execute(params![chunk_id, s.page, s.char_start, s.char_end])?;
            }
        }
    }

    tx.commit()?;
    Ok(chunks.len())
}

fn spans_for(conn: &Connection, chunk_ids: &[i64]) -> AppResult<Vec<(i64, Span)>> {
    if chunk_ids.is_empty() {
        return Ok(vec![]);
    }
    let holes = chunk_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT chunk_id, page, char_start, char_end FROM chunk_span
          WHERE chunk_id IN ({holes}) ORDER BY chunk_id, page, char_start"
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st
        .query_map(rusqlite::params_from_iter(chunk_ids.iter()), |r| {
            Ok((
                r.get::<_, i64>(0)?,
                Span {
                    page: r.get(1)?,
                    char_start: r.get(2)?,
                    char_end: r.get(3)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn attach_spans(conn: &Connection, mut chunks: Vec<Chunk>) -> AppResult<Vec<Chunk>> {
    let ids: Vec<i64> = chunks.iter().map(|c| c.id).collect();
    for (chunk_id, span) in spans_for(conn, &ids)? {
        if let Some(c) = chunks.iter_mut().find(|c| c.id == chunk_id) {
            c.spans.push(span);
        }
    }
    Ok(chunks)
}

fn bare(r: &rusqlite::Row) -> rusqlite::Result<Chunk> {
    Ok(Chunk {
        id: r.get(0)?,
        document_id: r.get(1)?,
        ord: r.get(2)?,
        heading_path: r.get(3)?,
        text: r.get(4)?,
        kind: r.get(5)?,
        page_start: r.get(6)?,
        page_end: r.get(7)?,
        spans: vec![],
    })
}

const SELECT: &str = "SELECT id, document_id, ord, heading_path, text, kind, page_start, page_end
                        FROM chunk";

pub fn list(conn: &Connection, document_id: i64) -> AppResult<Vec<Chunk>> {
    let mut st = conn.prepare(&format!("{SELECT} WHERE document_id = ?1 ORDER BY ord"))?;
    let chunks = st
        .query_map(params![document_id], bare)?
        .collect::<Result<Vec<_>, _>>()?;
    attach_spans(conn, chunks)
}

pub fn get(conn: &Connection, chunk_id: i64) -> AppResult<Chunk> {
    let chunk = conn
        .query_row(&format!("{SELECT} WHERE id = ?1"), params![chunk_id], bare)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::msg("그 청크를 찾지 못했습니다."),
            other => AppError::from(other),
        })?;
    Ok(attach_spans(conn, vec![chunk])?.remove(0))
}

pub fn count(conn: &Connection, document_id: i64) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM chunk WHERE document_id = ?1",
        params![document_id],
        |r| r.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        for (_, sql) in crate::db::schema::MIGRATIONS {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute(
            "INSERT INTO collection(id, name, created_at) VALUES (1, 'c', '2026-09-09')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size, created_at)
             VALUES (1, 1, 'd', 'a.pdf', 'x', 1, '2026-09-09')",
            [],
        )
        .unwrap();
        conn
    }

    fn chunk(ord: i64, text: &str, spans: Vec<Span>) -> ChunkIn {
        let page_start = spans.first().map(|s| s.page).unwrap_or(1);
        let page_end = spans.last().map(|s| s.page).unwrap_or(1);
        ChunkIn {
            ord,
            heading_path: "제1장 총칙".into(),
            text: text.into(),
            text_norm: text.into(),
            kind: "text".into(),
            page_start,
            page_end,
            spans,
        }
    }

    fn span(page: i64, a: i64, b: i64) -> Span {
        Span { page, char_start: a, char_end: b }
    }

    #[test]
    fn 담고_그대로_돌려받는다() {
        let mut c = db();
        replace_all(
            &mut c,
            1,
            &[
                chunk(0, "첫 조각", vec![span(1, 0, 12)]),
                chunk(1, "둘째 조각", vec![span(1, 12, 40), span(2, 0, 30)]),
            ],
        )
        .unwrap();

        let got = list(&c, 1).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].text, "첫 조각");
        assert_eq!(got[1].spans.len(), 2, "쪽을 걸친 청크는 구간이 둘이어야 한다");
        assert_eq!(got[1].spans[0].page, 1);
        assert_eq!(got[1].spans[1].page, 2);
        assert_eq!(got[1].page_start, 1);
        assert_eq!(got[1].page_end, 2);
        assert_eq!(got[0].heading_path.as_deref(), Some("제1장 총칙"));
    }

    #[test]
    fn 다시_나누면_옛_청크가_남지_않는다() {
        let mut c = db();
        replace_all(&mut c, 1, &[chunk(0, "옛것", vec![span(1, 0, 3)])]).unwrap();
        replace_all(&mut c, 1, &[chunk(0, "새것", vec![span(1, 0, 3)])]).unwrap();
        let got = list(&c, 1).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "새것");
        // 구간도 함께 정리되어야 한다
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM chunk_span", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn 가리키는_자리가_없는_청크는_거절한다() {
        // 자리가 없으면 근거를 보여 줄 수 없다. 담기 전에 막는다.
        let mut c = db();
        let bad = chunk(0, "어디에도 없는 글", vec![]);
        assert!(replace_all(&mut c, 1, &[bad]).is_err());
    }

    #[test]
    fn 거절된_뒤에는_아무것도_담기지_않는다() {
        let mut c = db();
        let ok = chunk(0, "괜찮은 것", vec![span(1, 0, 5)]);
        let bad = chunk(1, "자리 없는 것", vec![]);
        assert!(replace_all(&mut c, 1, &[ok, bad]).is_err());
        assert_eq!(count(&c, 1).unwrap(), 0, "한 건이라도 틀리면 통째로 물린다");
    }

    #[test]
    fn 낱말_색인에도_들어간다() {
        // 청크를 담으면 FTS5 색인이 따라와야 한다 (P4a 검색의 밑바탕)
        let mut c = db();
        replace_all(
            &mut c,
            1,
            &[chunk(0, "이용권을 교재비로 쓸 수 있다", vec![span(1, 0, 15)])],
        )
        .unwrap();
        let hit: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM chunk_fts WHERE chunk_fts MATCH '교재비'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hit, 1);
    }

    #[test]
    fn 문서를_지우면_청크와_구간도_사라진다() {
        let mut c = db();
        replace_all(&mut c, 1, &[chunk(0, "글", vec![span(1, 0, 1)])]).unwrap();
        c.execute("DELETE FROM document WHERE id = 1", []).unwrap();
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM chunk_span", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
