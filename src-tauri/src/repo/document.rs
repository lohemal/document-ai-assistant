//! 문서와 쪽 읽고 쓰기.
//!
//! 텍스트 추출은 화면 쪽(pdf.js)에서 한다. 여기는 그 결과를 받아 담고,
//! 원본 PDF 사본을 지키는 일을 한다.

use crate::error::{AppError, AppResult};
use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: i64,
    pub collection_id: i64,
    pub title: String,
    pub filename: String,
    pub sha256: String,
    pub byte_size: i64,
    pub page_count: i64,
    /// ok | scanned | extract_failed | indexing
    pub status: String,
    /// none | partial | done
    pub embed_state: String,
    /// 어느 pdf.js 판으로 뽑았는가
    pub extractor: Option<String>,
    /// 글자를 못 건진 쪽 수
    pub blank_pages: i64,
    pub created_at: String,
}

/// 화면에서 넘어오는 쪽 하나
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageIn {
    pub page: i64,
    pub label: Option<String>,
    pub text: String,
    /// `[[문자시작, 문자끝, 항목번호], …]` 를 JSON 문자열로 받는다.
    /// 쪽마다 수천 개라서, 중첩 배열로 주고받는 것보다 이쪽이 가볍다.
    pub item_map: String,
    /// text | scanned | empty
    pub kind: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageOut {
    pub page: i64,
    pub label: Option<String>,
    pub text: String,
    pub item_map: String,
    pub is_scanned: bool,
}

const SELECT: &str = "
SELECT d.id, d.collection_id, d.title, d.filename, d.sha256, d.byte_size,
       d.page_count, d.status, d.embed_state, d.extractor,
       -- 글자 없는 쪽. 세는 규칙이 `pageKind`(src/lib/pdf/extract.ts) 와 **같아야**
       -- 한다. TRIM 만 하면 표 사이의 탭과 줄바꿈이 글자로 세어져, 등록 때
       -- '스캔본' 으로 판단한 쪽 수와 화면에 보이는 쪽 수가 어긋난다.
       -- (실제로 368쪽 지침에서 15와 12로 갈렸다.)
       (SELECT COUNT(*) FROM page p
         WHERE p.document_id = d.id
           AND LENGTH(REPLACE(REPLACE(REPLACE(REPLACE(p.text, ' ', ''),
                              CHAR(9), ''), CHAR(10), ''), CHAR(13), '')) < 50),
       d.created_at
  FROM document d
";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Document> {
    Ok(Document {
        id: r.get(0)?,
        collection_id: r.get(1)?,
        title: r.get(2)?,
        filename: r.get(3)?,
        sha256: r.get(4)?,
        byte_size: r.get(5)?,
        page_count: r.get(6)?,
        status: r.get(7)?,
        embed_state: r.get(8)?,
        extractor: r.get(9)?,
        blank_pages: r.get(10)?,
        created_at: r.get(11)?,
    })
}

pub fn list(conn: &Connection, collection_id: i64) -> AppResult<Vec<Document>> {
    let mut st = conn.prepare(&format!(
        "{SELECT} WHERE d.collection_id = ?1 AND d.superseded_by IS NULL \
         ORDER BY d.created_at DESC"
    ))?;
    let items = st
        .query_map(params![collection_id], row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Document> {
    conn.query_row(&format!("{SELECT} WHERE d.id = ?1"), params![id], row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::msg("그 자료를 찾지 못했습니다."),
            other => other.into(),
        })
}

/// 같은 자료집에 내용이 똑같은 파일이 이미 있는가 (해시로 본다)
pub fn find_same(conn: &Connection, collection_id: i64, sha256: &str) -> AppResult<Option<Document>> {
    let found = conn
        .query_row(
            &format!("{SELECT} WHERE d.collection_id = ?1 AND d.sha256 = ?2 AND d.superseded_by IS NULL"),
            params![collection_id, sha256],
            row,
        )
        .optional()?;
    Ok(found)
}

/// 이름은 같은데 내용이 다른 문서 — 개정본으로 이어 붙일 대상
pub fn find_previous(
    conn: &Connection,
    collection_id: i64,
    filename: &str,
) -> AppResult<Option<Document>> {
    let found = conn
        .query_row(
            &format!(
                "{SELECT} WHERE d.collection_id = ?1 AND d.filename = ?2 \
                 AND d.superseded_by IS NULL ORDER BY d.created_at DESC LIMIT 1"
            ),
            params![collection_id, filename],
            row,
        )
        .optional()?;
    Ok(found)
}

/// 자리를 먼저 잡는다. 이 시점에는 아직 글자가 없다(`status = 'indexing'`).
pub fn begin(
    conn: &Connection,
    collection_id: i64,
    title: &str,
    filename: &str,
    sha256: &str,
    byte_size: i64,
    source_path: Option<&str>,
) -> AppResult<i64> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::msg("자료 이름이 비어 있습니다."));
    }
    let now = Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO document(collection_id, title, filename, sha256, byte_size,
                              source_path, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'indexing', ?7)",
        params![collection_id, title, filename, sha256, byte_size, source_path, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 뽑은 쪽들을 담는다. 한 묶음씩 여러 번 부를 수 있다.
pub fn save_pages(conn: &mut Connection, document_id: i64, pages: &[PageIn]) -> AppResult<()> {
    let tx = conn.transaction()?;
    {
        let mut st = tx.prepare(
            "INSERT OR REPLACE INTO page(document_id, page, label, text, is_scanned, item_map)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for p in pages {
            st.execute(params![
                document_id,
                p.page,
                p.label,
                p.text,
                if p.kind == "scanned" { 1 } else { 0 },
                p.item_map,
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// 다 끝났다고 표를 붙인다. 여기서부터 이 문서가 검색에 쓰인다.
pub fn finish(
    conn: &Connection,
    document_id: i64,
    page_count: i64,
    status: &str,
    extractor: &str,
) -> AppResult<()> {
    if !matches!(status, "ok" | "scanned" | "extract_failed") {
        return Err(AppError::msg(format!("알 수 없는 자료 상태입니다: {status}")));
    }
    conn.execute(
        "UPDATE document SET page_count = ?1, status = ?2, extractor = ?3 WHERE id = ?4",
        params![page_count, status, extractor, document_id],
    )?;
    Ok(())
}

/// 이름이 같은 옛 문서를 이 문서의 앞선 판으로 이어 붙인다.
///
/// 지우지 않는 까닭은 과거 작업 기록이 그 문서의 원문을 가리키고 있을 수 있기
/// 때문이다 (설계안 6장). 검색에서만 빠진다.
pub fn supersede(conn: &Connection, old_id: i64, new_id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE document SET superseded_by = ?1 WHERE id = ?2",
        params![new_id, old_id],
    )?;
    conn.execute(
        "UPDATE document SET revision_of = ?1 WHERE id = ?2",
        params![old_id, new_id],
    )?;
    Ok(())
}

pub fn pages(conn: &Connection, document_id: i64) -> AppResult<Vec<PageOut>> {
    let mut st = conn.prepare(
        "SELECT page, label, text, item_map, is_scanned FROM page
          WHERE document_id = ?1 ORDER BY page",
    )?;
    let items = st
        .query_map(params![document_id], |r| {
            Ok(PageOut {
                page: r.get(0)?,
                label: r.get(1)?,
                text: r.get(2)?,
                item_map: r.get(3)?,
                is_scanned: r.get::<_, i64>(4)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub fn page(conn: &Connection, document_id: i64, page: i64) -> AppResult<PageOut> {
    conn.query_row(
        "SELECT page, label, text, item_map, is_scanned FROM page
          WHERE document_id = ?1 AND page = ?2",
        params![document_id, page],
        |r| {
            Ok(PageOut {
                page: r.get(0)?,
                label: r.get(1)?,
                text: r.get(2)?,
                item_map: r.get(3)?,
                is_scanned: r.get::<_, i64>(4)? != 0,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::msg("그 쪽을 찾지 못했습니다."),
        other => other.into(),
    })
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    let n = conn.execute("DELETE FROM document WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(AppError::msg("그 자료를 찾지 못했습니다."));
    }
    Ok(())
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
            "INSERT INTO collection(id, name, created_at) VALUES (1, '늘봄학교', '2026-09-09')",
            [],
        )
        .unwrap();
        conn
    }

    fn page_in(page: i64, text: &str, map: &str, kind: &str) -> PageIn {
        PageIn {
            page,
            label: None,
            text: text.into(),
            item_map: map.into(),
            kind: kind.into(),
        }
    }

    #[test]
    fn 등록하고_쪽을_담고_끝낸다() {
        let mut c = db();
        let id = begin(&c, 1, "운영지침", "guide.pdf", "abc123", 1000, Some("D:/a.pdf")).unwrap();
        assert_eq!(get(&c, id).unwrap().status, "indexing");

        save_pages(
            &mut c,
            id,
            &[
                page_in(1, "첫째 쪽입니다.", "[[0,7,0]]", "text"),
                page_in(2, "둘째 쪽입니다.", "[[0,7,3]]", "text"),
            ],
        )
        .unwrap();
        finish(&c, id, 2, "ok", "pdfjs-6.3.289").unwrap();

        let d = get(&c, id).unwrap();
        assert_eq!(d.status, "ok");
        assert_eq!(d.page_count, 2);
        assert_eq!(d.extractor.as_deref(), Some("pdfjs-6.3.289"));
        assert_eq!(pages(&c, id).unwrap().len(), 2);
    }

    #[test]
    fn 문자_위치_지도가_그대로_돌아온다() {
        // 이 값이 상하면 형광펜이 엉뚱한 곳을 가리킨다
        let mut c = db();
        let id = begin(&c, 1, "d", "a.pdf", "h", 1, None).unwrap();
        let map = "[[0,4,0],[5,9,2],[10,18,7]]";
        save_pages(&mut c, id, &[page_in(1, "가나다라 마바사아 자차카타파하아", map, "text")]).unwrap();
        assert_eq!(page(&c, id, 1).unwrap().item_map, map);
    }

    #[test]
    fn 같은_쪽을_다시_담으면_덮어쓴다() {
        // 등록을 중간에 멈췄다 다시 하면 같은 쪽이 두 번 올 수 있다
        let mut c = db();
        let id = begin(&c, 1, "d", "a.pdf", "h", 1, None).unwrap();
        save_pages(&mut c, id, &[page_in(1, "처음", "[]", "text")]).unwrap();
        save_pages(&mut c, id, &[page_in(1, "다시", "[]", "text")]).unwrap();
        let ps = pages(&c, id).unwrap();
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].text, "다시");
    }

    #[test]
    fn 알_수_없는_상태는_거절한다() {
        let c = db();
        let id = begin(&c, 1, "d", "a.pdf", "h", 1, None).unwrap();
        assert!(finish(&c, id, 1, "말도안되는상태", "x").is_err());
    }

    #[test]
    fn 내용이_같은_파일을_알아본다() {
        let c = db();
        let id = begin(&c, 1, "지침", "a.pdf", "같은해시", 10, None).unwrap();
        finish(&c, id, 1, "ok", "x").unwrap();
        assert!(find_same(&c, 1, "같은해시").unwrap().is_some());
        assert!(find_same(&c, 1, "다른해시").unwrap().is_none());
        // 다른 자료집이면 남남이다
        assert!(find_same(&c, 2, "같은해시").unwrap().is_none());
    }

    #[test]
    fn 바뀐_파일은_지우지_않고_개정본으로_잇는다() {
        // 과거 작업 기록이 옛 문서의 원문을 가리킬 수 있으므로 지우면 안 된다
        let c = db();
        let old = begin(&c, 1, "지침 2025", "guide.pdf", "옛해시", 10, None).unwrap();
        finish(&c, old, 1, "ok", "x").unwrap();
        let new = begin(&c, 1, "지침 2026", "guide.pdf", "새해시", 12, None).unwrap();
        finish(&c, new, 1, "ok", "x").unwrap();
        supersede(&c, old, new).unwrap();

        // 목록에는 새것만 보인다
        let visible = list(&c, 1).unwrap();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, new);

        // 옛것도 여전히 열린다
        assert_eq!(get(&c, old).unwrap().title, "지침 2025");
    }

    #[test]
    fn 글자를_못_건진_쪽을_센다() {
        let mut c = db();
        let id = begin(&c, 1, "d", "a.pdf", "h", 1, None).unwrap();
        save_pages(
            &mut c,
            id,
            &[
                page_in(1, &"가".repeat(80), "[]", "text"),
                page_in(2, "", "[]", "scanned"),
                page_in(3, "짧다", "[]", "empty"),
                // 표 사이의 탭·줄바꿈은 글자가 아니다. 이 쪽은 '글자 없는 쪽'
                // 으로 세어야 한다 — 등록 때의 판단과 화면의 숫자를 맞춘다.
                // 글자는 40자뿐인데 탭·줄바꿈까지 세면 80자가 된다
                page_in(4, &"칸\t칸\n".repeat(20), "[]", "empty"),
            ],
        )
        .unwrap();
        assert_eq!(get(&c, id).unwrap().blank_pages, 3);
    }

    #[test]
    fn 자료를_지우면_쪽도_함께_지워진다() {
        let mut c = db();
        let id = begin(&c, 1, "d", "a.pdf", "h", 1, None).unwrap();
        save_pages(&mut c, id, &[page_in(1, "가", "[]", "text")]).unwrap();
        delete(&c, id).unwrap();
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM page", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
