//! 설정 한 줄씩 읽고 쓰기.
//!
//! 값이 없을 때 **무엇이 기본인지 한자리에서** 정한다. 부르는 곳마다
//! `unwrap_or("bge-m3")` 를 쓰면, 기본값을 바꿀 때 한 군데를 빠뜨리게 된다.

use crate::error::AppResult;
use rusqlite::{params, Connection};

/// 검색에 쓸 모델의 카탈로그 id
pub const EMBED_MODEL: &str = "embed_model";
/// 작업 기록 보존 기간 (P6 에서 쓴다)
pub const RETENTION_DAYS: &str = "retention_days";

const DEFAULTS: &[(&str, &str)] = &[
    // P4b 골든 셋으로 정했다 (설계안 결정사항 15)
    (EMBED_MODEL, "embed-standard"),
    (RETENTION_DAYS, "30"),
];

pub fn get(conn: &Connection, key: &str) -> AppResult<String> {
    let found: Option<String> = conn
        .query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| r.get(0))
        .ok();
    Ok(found.unwrap_or_else(|| {
        DEFAULTS
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.to_string())
            .unwrap_or_default()
    }))
}

pub fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO setting(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = ?2",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::MIGRATIONS;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        for (_, sql) in MIGRATIONS {
            conn.execute_batch(sql).unwrap();
        }
        conn
    }

    #[test]
    fn 처음부터_기본_검색_모델이_정해져_있다() {
        let c = db();
        assert_eq!(get(&c, EMBED_MODEL).unwrap(), "embed-standard");
    }

    #[test]
    fn 바꾼_값이_남는다() {
        let c = db();
        set(&c, EMBED_MODEL, "embed-light").unwrap();
        assert_eq!(get(&c, EMBED_MODEL).unwrap(), "embed-light");
    }

    #[test]
    fn 모르는_열쇠는_빈_값이다() {
        let c = db();
        assert_eq!(get(&c, "없는설정").unwrap(), "");
    }
}
