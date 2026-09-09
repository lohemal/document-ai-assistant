//! 자료집 읽고 쓰기.

use crate::error::{AppError, AppResult};
use chrono::Local;
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: i64,
    pub name: String,
    /// 이 자료집에 든 문서 수
    pub doc_count: i64,
    /// 그 중 임베딩이 아직 없는 문서 수.
    /// AI 모델을 나중에 설치했을 때 "채울 것이 남았다"고 알려 주는 데 쓴다.
    pub embed_pending: i64,
    /// 이 자료집을 색인할 때 쓴 임베딩 모델. 아직 없으면 None.
    pub embed_model: Option<String>,
    pub created_at: String,
}

const SELECT: &str = "
SELECT c.id,
       c.name,
       (SELECT COUNT(*) FROM document d WHERE d.collection_id = c.id),
       -- 벡터가 청크 수만큼 있지 않은 문서 수.
       --
       -- 어느 모델로 만든 벡터인지는 여기서 보지 않는다. 그건 지금 고른
       -- 모델을 알아야 하는 일이라 repo::embed_index 가 맡는다. 여기 값은
       -- 채울 것이 남았는지만 거칠게 말한다.
       (SELECT COUNT(*) FROM document d
         WHERE d.collection_id = c.id
           AND (SELECT COUNT(*) FROM chunk ch WHERE ch.document_id = d.id)
               > (SELECT COUNT(*) FROM chunk ch JOIN embedding e ON e.chunk_id = ch.id
                   WHERE ch.document_id = d.id)),
       c.embed_model,
       c.created_at
  FROM collection c
";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Collection> {
    Ok(Collection {
        id: r.get(0)?,
        name: r.get(1)?,
        doc_count: r.get(2)?,
        embed_pending: r.get(3)?,
        embed_model: r.get(4)?,
        created_at: r.get(5)?,
    })
}

pub fn list(conn: &Connection) -> AppResult<Vec<Collection>> {
    let mut st = conn.prepare(&format!("{SELECT} ORDER BY c.name COLLATE NOCASE"))?;
    let items = st.query_map([], row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Collection> {
    conn.query_row(&format!("{SELECT} WHERE c.id = ?1"), params![id], row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::msg("그 자료집을 찾지 못했습니다."),
            other => other.into(),
        })
}

/// 이름을 다듬는다. 앞뒤 공백을 떼고, 연속 공백을 하나로 줄인다.
fn tidy(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn check_name(name: &str) -> AppResult<String> {
    let name = tidy(name);
    if name.is_empty() {
        return Err(AppError::msg("자료집 이름을 입력해 주세요."));
    }
    if name.chars().count() > 40 {
        return Err(AppError::msg("자료집 이름은 40자까지 쓸 수 있습니다."));
    }
    Ok(name)
}

pub fn create(conn: &Connection, name: &str) -> AppResult<i64> {
    let name = check_name(name)?;
    let now = Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO collection(name, created_at) VALUES (?1, ?2)",
        params![name, now],
    )
    .map_err(|e| dup_name(e, &name))?;
    Ok(conn.last_insert_rowid())
}

pub fn rename(conn: &Connection, id: i64, name: &str) -> AppResult<()> {
    let name = check_name(name)?;
    let n = conn
        .execute(
            "UPDATE collection SET name = ?1 WHERE id = ?2",
            params![name, id],
        )
        .map_err(|e| dup_name(e, &name))?;
    if n == 0 {
        return Err(AppError::msg("그 자료집을 찾지 못했습니다."));
    }
    Ok(())
}

/// 자료집을 지운다. 안에 든 문서·청크·임베딩도 함께 지워진다(ON DELETE CASCADE).
///
/// 지워진 문서들의 id 를 돌려준다 — 호출한 쪽에서 `files/<id>.pdf` 를 지우라는
/// 뜻이다. 파일 삭제를 여기서 하지 않는 것은, DB 트랜잭션이 되돌아갈 수 있는데
/// 파일 삭제는 되돌아가지 않기 때문이다.
pub fn delete(conn: &Connection, id: i64) -> AppResult<Vec<i64>> {
    let doc_ids: Vec<i64> = {
        let mut st = conn.prepare("SELECT id FROM document WHERE collection_id = ?1")?;
        let ids = st
            .query_map(params![id], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids
    };

    let n = conn.execute("DELETE FROM collection WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(AppError::msg("그 자료집을 찾지 못했습니다."));
    }
    Ok(doc_ids)
}

/// 이름이 겹칠 때는 SQLite 의 영문 오류 대신 읽을 수 있는 안내를 준다.
fn dup_name(e: rusqlite::Error, name: &str) -> AppError {
    let is_dup = matches!(
        &e,
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation
    );
    if is_dup {
        AppError::msg(format!("'{name}' 이라는 자료집이 이미 있습니다."))
    } else {
        e.into()
    }
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
        conn
    }

    #[test]
    fn 만들고_이름을_바꾸고_지운다() {
        let c = db();
        let id = create(&c, "늘봄학교").unwrap();
        assert_eq!(get(&c, id).unwrap().name, "늘봄학교");

        rename(&c, id, "늘봄학교 운영").unwrap();
        assert_eq!(get(&c, id).unwrap().name, "늘봄학교 운영");

        delete(&c, id).unwrap();
        assert!(get(&c, id).is_err());
    }

    #[test]
    fn 이름의_앞뒤_공백과_연속_공백을_다듬는다() {
        let c = db();
        let id = create(&c, "  초3   지원금  ").unwrap();
        assert_eq!(get(&c, id).unwrap().name, "초3 지원금");
    }

    #[test]
    fn 빈_이름은_거절한다() {
        let c = db();
        assert!(create(&c, "   ").is_err());
    }

    #[test]
    fn 같은_이름은_거절하고_이유를_알려_준다() {
        let c = db();
        create(&c, "자유수강권").unwrap();
        let err = create(&c, "자유수강권").unwrap_err().to_string();
        assert!(err.contains("이미 있습니다"), "안내가 읽을 만해야 한다: {err}");
    }

    #[test]
    fn 다듬은_뒤에_이름이_겹치는_것도_잡는다() {
        let c = db();
        create(&c, "자유수강권").unwrap();
        assert!(create(&c, " 자유수강권 ").is_err());
    }

    #[test]
    fn 지울_때_안에_든_문서_id_를_돌려준다() {
        let c = db();
        let cid = create(&c, "초3 지원금").unwrap();
        for i in 1..=3 {
            c.execute(
                "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size, created_at)
                 VALUES (?1, ?2, 'd', 'a.pdf', 'x', 1, '2026-09-09')",
                params![i, cid],
            )
            .unwrap();
        }
        let mut ids = delete(&c, cid).unwrap();
        ids.sort();
        assert_eq!(ids, vec![1, 2, 3], "지운 문서의 파일도 치울 수 있어야 한다");
    }

    #[test]
    fn 벡터가_덜_만들어진_문서_수를_센다() {
        // P4c 부터는 문서에 적힌 상태가 아니라 **실제 벡터 수**로 센다.
        // 상태는 모델을 바꾼 순간 거짓이 되기 때문이다.
        let c = db();
        let cid = create(&c, "늘봄").unwrap();
        for (id, name) in [(1, "a"), (2, "b")] {
            c.execute(
                "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size, created_at)
                 VALUES (?1, ?2, ?3, ?3, ?3, 1, '2026-09-09')",
                params![id, cid, name],
            )
            .unwrap();
            c.execute(
                "INSERT INTO chunk(id, document_id, ord, text, text_norm, kind,
                                   page_start, page_end, hash)
                 VALUES (?1, ?1, 0, '글', '글', 'text', 1, 1, 'h')",
                params![id],
            )
            .unwrap();
        }
        // 첫 문서만 벡터가 있다
        c.execute(
            "INSERT INTO embedding(chunk_id, model, dim, vec, chunk_hash, created_at)
             VALUES (1, 'bge-m3', 2, X'0000', 'h', '2026-09-09')",
            [],
        )
        .unwrap();

        let got = get(&c, cid).unwrap();
        assert_eq!(got.doc_count, 2);
        assert_eq!(got.embed_pending, 1);
    }
}
