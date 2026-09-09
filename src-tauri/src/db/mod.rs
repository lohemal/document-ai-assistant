pub mod schema;

use crate::error::{AppError, AppResult};
use chrono::Local;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 지금 프로그램이 아는 자료구조 번호.
///
/// 손으로 적지 않고 마이그레이션 목록의 마지막에서 끌어온다. 손으로 맞추게
/// 두면 언젠가 어긋나고, 어긋나면 자료가 반만 올라간 채로 열린다.
pub const SCHEMA_VERSION: i64 = schema::MIGRATIONS[schema::MIGRATIONS.len() - 1].0;

/// 앱이 들고 다니는 연결 하나. rusqlite 연결은 여러 갈래에서 동시에 쓸 수
/// 없으므로 자물쇠로 감싼다. 이 프로그램에서 무거운 일은 임베딩과 LLM 이지
/// SQLite 가 아니라서, 연결 하나로 충분하다.
#[derive(Debug)]
pub struct Db(Mutex<Connection>);

impl Db {
    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let guard = self
            .0
            .lock()
            .map_err(|_| AppError::msg("자료를 읽는 중 문제가 생겼습니다. 프로그램을 다시 켜 주세요."))?;
        f(&guard)
    }

    pub fn with_mut<T>(&self, f: impl FnOnce(&mut Connection) -> AppResult<T>) -> AppResult<T> {
        let mut guard = self
            .0
            .lock()
            .map_err(|_| AppError::msg("자료를 읽는 중 문제가 생겼습니다. 프로그램을 다시 켜 주세요."))?;
        f(&mut guard)
    }
}

pub fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join("data.db")
}

pub fn files_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("files")
}

fn backups_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

/// 자료 폴더를 열고, 필요하면 자료구조를 올린다.
pub fn open(data_dir: &Path) -> AppResult<Db> {
    std::fs::create_dir_all(files_dir(data_dir))?;
    let mut conn = Connection::open(db_path(data_dir))?;

    // WAL 은 앱이 갑자기 꺼져도 자료를 지키는 데 유리하다.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    // ON DELETE CASCADE / SET NULL 이 실제로 동작하려면 켜야 한다.
    conn.pragma_update(None, "foreign_keys", "ON")?;

    migrate(&mut conn, data_dir)?;
    Ok(Db(Mutex::new(conn)))
}

fn user_version(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// 자료구조를 올린다.
///
/// - 새 자료(버전 0)면 백업 없이 만든다. 지킬 자료가 없다.
/// - 이미 쓰던 자료면 **손대기 전에 통째로 백업**한다.
/// - 자료가 프로그램보다 새것이면 **아무것도 하지 않고 멈춘다.** 옛 프로그램이
///   새 자료를 건드리면 조용히 망가진다.
fn migrate(conn: &mut Connection, data_dir: &Path) -> AppResult<()> {
    let from = user_version(conn)?;

    if from > SCHEMA_VERSION {
        return Err(AppError::msg(format!(
            "이 자료는 더 새로운 버전의 프로그램에서 만들어졌습니다 \
             (자료 {from}, 프로그램 {SCHEMA_VERSION}). \
             프로그램을 최신 버전으로 업데이트한 뒤 다시 열어 주세요."
        )));
    }
    if from == SCHEMA_VERSION {
        return Ok(());
    }

    // 이미 쓰던 자료를 올리는 것이라면 먼저 백업한다.
    if from > 0 {
        let saved = backup(conn, data_dir, from)?;
        log::info!("자료구조를 올리기 전에 백업했습니다: {}", saved.display());
    }

    for (version, sql) in schema::MIGRATIONS {
        if *version <= from {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        // PRAGMA 는 바인딩을 받지 않는다
        tx.execute_batch(&format!("PRAGMA user_version = {version}"))?;
        tx.commit()?;
        log::info!("자료구조를 {version} 로 올렸습니다.");
    }

    Ok(())
}

/// 자료를 파일 하나로 복사한다. 마이그레이션 직전과 사용자가 백업을 누를 때 쓴다.
pub fn backup(conn: &Connection, data_dir: &Path, version: i64) -> AppResult<PathBuf> {
    let dir = backups_dir(data_dir);
    std::fs::create_dir_all(&dir)?;

    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("pre-migration-v{version}-{stamp}.db"));

    let mut dest = Connection::open(&path)?;
    let backup = rusqlite::backup::Backup::new(conn, &mut dest)?;
    backup.run_to_completion(200, std::time::Duration::from_millis(50), None)?;

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 시험용으로 메모리가 아니라 임시 폴더를 쓴다.
    /// 마이그레이션과 백업이 파일을 다루기 때문이다.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "docaid-test-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn 새_자료를_만들면_최신_버전이_된다() {
        let dir = temp_dir("fresh");
        let db = open(&dir).unwrap();
        let v = db.with(|c| user_version(c)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 두_번_열어도_망가지지_않는다() {
        let dir = temp_dir("twice");
        {
            let db = open(&dir).unwrap();
            db.with(|c| {
                c.execute(
                    "INSERT INTO collection(name, created_at) VALUES ('늘봄학교', '2026-09-09')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        }
        let db = open(&dir).unwrap();
        let n: i64 = db
            .with(|c| Ok(c.query_row("SELECT COUNT(*) FROM collection", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(n, 1, "다시 열었을 때 자료가 남아 있어야 한다");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 자료가_프로그램보다_새것이면_열지_않는다() {
        let dir = temp_dir("newer");
        {
            let conn = Connection::open(db_path(&dir)).unwrap();
            conn.execute_batch(&format!("PRAGMA user_version = {}", SCHEMA_VERSION + 5))
                .unwrap();
        }
        let err = open(&dir).unwrap_err().to_string();
        assert!(
            err.contains("더 새로운 버전"),
            "옛 프로그램이 새 자료를 건드리면 안 된다: {err}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 자료집을_지우면_문서도_같이_지워진다() {
        // ON DELETE CASCADE 가 실제로 켜져 있는지 본다.
        // foreign_keys PRAGMA 를 빠뜨리면 조용히 고아 자료가 남는다.
        let dir = temp_dir("cascade");
        let db = open(&dir).unwrap();
        db.with(|c| {
            c.execute(
                "INSERT INTO collection(id, name, created_at) VALUES (1, '초3 지원금', '2026-09-09')",
                [],
            )?;
            c.execute(
                "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size, created_at)
                 VALUES (1, 1, '운영지침', 'a.pdf', 'deadbeef', 100, '2026-09-09')",
                [],
            )?;
            c.execute("DELETE FROM collection WHERE id = 1", [])?;
            let n: i64 = c.query_row("SELECT COUNT(*) FROM document", [], |r| r.get(0))?;
            assert_eq!(n, 0, "자료집을 지우면 그 안의 문서도 지워져야 한다");
            Ok(())
        })
        .unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 옛_자료를_올려도_담긴_것이_그대로_남는다() {
        // 설계안 10-5. 업데이트로 자료가 상하지 않는다는 것을 실제로 확인한다.
        let dir = temp_dir("migrate");

        // v1 만 적용한 옛 자료를 만든다
        {
            let conn = Connection::open(db_path(&dir)).unwrap();
            conn.pragma_update(None, "foreign_keys", "ON").unwrap();
            let (v, sql) = schema::MIGRATIONS[0];
            conn.execute_batch(sql).unwrap();
            conn.execute_batch(&format!("PRAGMA user_version = {v}")).unwrap();
            conn.execute(
                "INSERT INTO collection(id, name, embed_model, embed_dim, created_at)
                 VALUES (1, '늘봄학교', 'bge-m3', 1024, '2026-09-01')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size, status, created_at)
                 VALUES (1, 1, '운영지침', 'a.pdf', 'abc', 100, 'ok', '2026-09-01')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO chunk(id, document_id, ord, text, text_norm, page_start, page_end)
                 VALUES (1, 1, 0, '교재비로 쓸 수 있다', '교재비로 쓸 수 있다', 3, 3)",
                [],
            )
            .unwrap();
            // 임베딩은 다시 만드는 값이 비싸다. 절대 잃으면 안 된다.
            conn.execute(
                "INSERT INTO embedding(chunk_id, model, dim, vec) VALUES (1, 'bge-m3', 2, ?1)",
                [&[0u8, 1, 2, 3, 4, 5, 6, 7][..]],
            )
            .unwrap();
        }

        // 지금 프로그램으로 연다 -> v2 로 올라간다
        let db = open(&dir).unwrap();
        assert_eq!(db.with(|c| user_version(c)).unwrap(), SCHEMA_VERSION);

        db.with(|c| {
            let name: String =
                c.query_row("SELECT name FROM collection WHERE id = 1", [], |r| r.get(0))?;
            assert_eq!(name, "늘봄학교");

            let vec: Vec<u8> =
                c.query_row("SELECT vec FROM embedding WHERE chunk_id = 1", [], |r| r.get(0))?;
            assert_eq!(vec, vec![0u8, 1, 2, 3, 4, 5, 6, 7], "임베딩이 그대로 있어야 한다");

            // v2 에서 더한 칸은 기본값으로 채워져 있다
            let map: String = c.query_row(
                "SELECT COALESCE((SELECT item_map FROM page LIMIT 1), '[]')",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(map, "[]");

            // 낱말 색인도 살아 있다
            let hit: i64 = c.query_row(
                "SELECT COUNT(*) FROM chunk_fts WHERE chunk_fts MATCH '교재비'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(hit, 1);
            Ok(())
        })
        .unwrap();

        // 손대기 전에 백업을 떠 두었는가
        let backups: Vec<_> = std::fs::read_dir(dir.join("backups"))
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(backups.len(), 1, "올리기 전에 백업이 하나 있어야 한다");
        assert!(backups[0].file_name().to_string_lossy().starts_with("pre-migration-v1-"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 낱말_검색_색인이_한국어_조사를_넘어간다() {
        // trigram 토크나이저가 실제로 빌드에 들어 있는지, 그리고 조사가 붙어도
        // 걸리는지를 함께 본다. 이게 안 되면 "AI 없이 되는 검색"이 성립하지 않는다.
        let dir = temp_dir("fts");
        let db = open(&dir).unwrap();
        db.with(|c| {
            c.execute(
                "INSERT INTO collection(id, name, created_at) VALUES (1, 'c', '2026-09-09')",
                [],
            )?;
            c.execute(
                "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size, created_at)
                 VALUES (1, 1, 'd', 'a.pdf', 'x', 1, '2026-09-09')",
                [],
            )?;
            c.execute(
                "INSERT INTO chunk(id, document_id, ord, text, text_norm, page_start, page_end)
                 VALUES (1, 1, 0, '이용권을 교재비로 쓸 수 있다', '이용권을 교재비로 쓸 수 있다', 3, 3)",
                [],
            )?;

            // 조사가 없는 낱말로 조사가 붙은 본문을 찾을 수 있어야 한다
            let hit: i64 = c.query_row(
                "SELECT COUNT(*) FROM chunk_fts WHERE chunk_fts MATCH '이용권'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(hit, 1, "trigram 색인이 조사를 넘어가지 못했다");

            // 지우면 색인에서도 빠져야 한다
            c.execute("DELETE FROM chunk WHERE id = 1", [])?;
            let hit: i64 = c.query_row(
                "SELECT COUNT(*) FROM chunk_fts WHERE chunk_fts MATCH '이용권'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(hit, 0, "청크를 지웠는데 색인에 남아 있다");
            Ok(())
        })
        .unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }
}
