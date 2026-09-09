//! 섞어 찾기를 시험한다.
//!
//! 가장 중요한 것은 **색인이 안 된 문서가 검색에서 빠지지 않는가** 다
//! (요구사항 8). 자료집에 다섯 개를 넣고 셋만 색인한 사용자가, 나머지 둘을
//! 검색에서 잃으면 그건 자료를 잃은 것과 같다.

use super::*;
use crate::db::schema::MIGRATIONS;
use crate::repo::chunk::hash_of;
use crate::repo::vector;

const MODEL: &str = "시험모델";

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for (_, sql) in MIGRATIONS {
        conn.execute_batch(sql).unwrap();
    }
    conn.execute(
        "INSERT INTO collection(id, name, created_at) VALUES (1, '자료집', '2026-09-09')",
        [],
    )
    .unwrap();
    conn
}

/// 문서를 만들고 청크를 넣는다. 돌려주는 것은 청크 id 들.
fn doc(conn: &Connection, id: i64, title: &str, texts: &[&str]) -> Vec<i64> {
    conn.execute(
        "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size, page_count,
                              status, created_at)
         VALUES (?1, 1, ?2, ?2, ?2, 1, 1, 'ok', '2026-09-09')",
        rusqlite::params![id, title],
    )
    .unwrap();
    let mut ids = Vec::new();
    for (ord, t) in texts.iter().enumerate() {
        conn.execute(
            "INSERT INTO chunk(document_id, ord, heading_path, text, text_norm, kind,
                               page_start, page_end, hash)
             VALUES (?1, ?2, '', ?3, ?3, 'text', 1, 1, ?4)",
            rusqlite::params![id, ord as i64, t, hash_of(t)],
        )
        .unwrap();
        let cid = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO chunk_span(chunk_id, page, char_start, char_end) VALUES (?1, 1, 0, ?2)",
            rusqlite::params![cid, t.chars().count() as i64],
        )
        .unwrap();
        ids.push(cid);
    }
    ids
}

fn put(conn: &Connection, chunk_id: i64, v: &[f32]) {
    let hash: String = conn
        .query_row("SELECT hash FROM chunk WHERE id = ?1", [chunk_id], |r| r.get(0))
        .unwrap();
    vector::save(conn, chunk_id, MODEL, &hash, v).unwrap();
}

fn find(conn: &Connection, text: &str, q: &[f32]) -> SearchResult {
    hybrid_search(
        conn,
        &Hybrid {
            text,
            query_vec: q,
            model: MODEL,
            collection_ids: vec![],
            limit: 10,
            depth: DEFAULT_DEPTH,
        },
    )
    .unwrap()
}

#[test]
fn 색인_안_된_문서도_낱말로_계속_찾힌다() {
    let conn = db();
    // 색인한 문서
    let a = doc(&conn, 1, "색인된 자료", &["방과후학교 자유수강권 지원 대상은 저소득층 학생이다"]);
    put(&conn, a[0], &[1.0, 0.0]);
    // 색인하지 않은 문서 — 벡터가 없다
    let b = doc(&conn, 2, "색인 안 된 자료", &["수익자부담경비는 학교운영위원회 심의를 거친다"]);

    // 색인 안 된 문서에만 있는 낱말로 찾는다
    let r = find(&conn, "수익자부담경비 심의", &[0.0, 1.0]);
    let ids: Vec<i64> = r.hits.iter().map(|h| h.chunk_id).collect();
    assert!(ids.contains(&b[0]), "색인 안 된 문서가 빠졌습니다: {ids:?}");
    assert_eq!(r.mode, "hybrid");
}

#[test]
fn 뜻으로만_가까운_것도_올라온다() {
    let conn = db();
    let a = doc(&conn, 1, "자료", &["저소득층 학생 지원", "전혀 다른 이야기"]);
    put(&conn, a[0], &[1.0, 0.0]);
    put(&conn, a[1], &[0.0, 1.0]);

    // 낱말로는 하나도 안 걸리는 물음. 벡터만 첫 청크에 가깝다.
    let r = find(&conn, "형편이 어려운 아이들", &[1.0, 0.05]);
    assert_eq!(r.hits.first().map(|h| h.chunk_id), Some(a[0]), "{:?}", r.hits.len());
    assert_eq!(r.hits[0].semantic_rank, Some(1));
    assert_eq!(r.hits[0].keyword_rank, None);
}

#[test]
fn 두_방법이_다_찾으면_관련도가_높음이다() {
    let conn = db();
    let a = doc(&conn, 1, "자료", &["자유수강권 지원 대상", "다른 청크"]);
    put(&conn, a[0], &[1.0, 0.0]);
    put(&conn, a[1], &[0.0, 1.0]);

    let r = find(&conn, "자유수강권 지원", &[1.0, 0.0]);
    let top = &r.hits[0];
    assert_eq!(top.chunk_id, a[0]);
    assert_eq!(top.relevance, "높음");
    assert_eq!(top.keyword_rank, Some(1));
    assert_eq!(top.semantic_rank, Some(1));
}

#[test]
fn 한쪽만_찾으면_관련도가_보통이다() {
    let conn = db();
    let a = doc(&conn, 1, "자료", &["자유수강권 지원 대상"]);
    let b = doc(&conn, 2, "다른 자료", &["아주 다른 내용"]);
    put(&conn, b[0], &[1.0, 0.0]);

    let r = find(&conn, "자유수강권", &[1.0, 0.0]);
    let kw = r.hits.iter().find(|h| h.chunk_id == a[0]).unwrap();
    assert_eq!(kw.relevance, "보통");
    assert!(kw.keyword_rank.is_some());
    assert!(kw.semantic_rank.is_none());
}

#[test]
fn 낱말_쪽에서_찾아본_말을_그대로_보여_준다() {
    // 섞었다고 조사를 뗀 사실을 숨기면 결과를 읽을 수 없다
    let conn = db();
    let a = doc(&conn, 1, "자료", &["자유수강권 지원 대상"]);
    put(&conn, a[0], &[1.0, 0.0]);

    let r = find(&conn, "자유수강권은 누가 받나요?", &[1.0, 0.0]);
    assert!(r.terms.iter().any(|t| t == "자유수강권"), "{:?}", r.terms);
}

#[test]
fn 원문_위치가_함께_온다() {
    // 섞어 찾아도 형광펜이 칠할 자리는 그대로 따라와야 한다
    let conn = db();
    let a = doc(&conn, 1, "자료", &["자유수강권 지원 대상"]);
    put(&conn, a[0], &[1.0, 0.0]);

    let r = find(&conn, "자유수강권", &[1.0, 0.0]);
    let top = &r.hits[0];
    assert_eq!(top.spans.len(), 1);
    assert_eq!(top.spans[0].page, 1);
    assert!(top.spans[0].char_end > 0);
}

#[test]
fn 다른_모델로_만든_벡터는_섞이지_않는다() {
    let conn = db();
    let a = doc(&conn, 1, "자료", &["저소득층 학생 지원"]);
    // 다른 모델로 만든 벡터
    let hash: String = conn
        .query_row("SELECT hash FROM chunk WHERE id = ?1", [a[0]], |r| r.get(0))
        .unwrap();
    vector::save(&conn, a[0], "옛-모델", &hash, &[1.0, 0.0]).unwrap();

    // 뜻으로는 가까운 물음인데, 지금 모델의 벡터가 아니므로 뜻 쪽에서는 안 걸려야 한다
    let r = find(&conn, "형편이 어려운 아이들", &[1.0, 0.0]);
    assert!(
        r.hits.iter().all(|h| h.semantic_rank.is_none()),
        "옛 모델 벡터가 검색에 섞였습니다"
    );
}
