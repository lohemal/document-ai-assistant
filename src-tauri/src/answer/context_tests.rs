//! 근거 고르기 시험.

use super::*;
use crate::db::schema::MIGRATIONS;
use crate::repo::chunk::hash_of;
use crate::repo::search::{keyword_search, Request};

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
    conn.execute(
        "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size, page_count,
                              status, created_at)
         VALUES (1, 1, '길라잡이', 'a.pdf', 'x', 1, 9, 'ok', '2026-09-09')",
        [],
    )
    .unwrap();
    conn
}

fn add(conn: &Connection, ord: i64, page: i64, text: &str) -> i64 {
    conn.execute(
        "INSERT INTO chunk(document_id, ord, heading_path, text, text_norm, kind,
                           page_start, page_end, hash)
         VALUES (1, ?1, '제1장', ?2, ?2, 'text', ?3, ?3, ?4)",
        rusqlite::params![ord, text, page, hash_of(text)],
    )
    .unwrap();
    let id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO chunk_span(chunk_id, page, char_start, char_end) VALUES (?1, ?2, 0, ?3)",
        rusqlite::params![id, page, text.chars().count() as i64],
    )
    .unwrap();
    id
}

fn find(conn: &Connection, q: &str) -> Vec<Hit> {
    keyword_search(
        conn,
        &Request { text: q.into(), collection_ids: vec![], limit: 20 },
    )
    .unwrap()
    .hits
}

#[test]
fn 상위_다섯_개만_쓴다() {
    let c = db();
    for i in 0..10 {
        add(&c, i, i + 1, &format!("자유수강권 이야기 {i} 번째 조각입니다"));
    }
    let hits = find(&c, "자유수강권");
    assert!(hits.len() > 5);

    let ev = build(&c, &hits, Plan { top_k: 5, radius: 0, max_chars: 100_000 }).unwrap();
    assert_eq!(ev.len(), 5);
    assert!(ev.iter().all(|e| !e.neighbor));
}

#[test]
fn 앞뒤_이웃을_붙인다() {
    let c = db();
    add(&c, 0, 1, "앞 조각");
    add(&c, 1, 2, "자유수강권 지원 대상");
    add(&c, 2, 3, "뒤 조각");

    let hits = find(&c, "자유수강권");
    let ev = build(&c, &hits, DEFAULT_PLAN).unwrap();
    assert_eq!(ev.len(), 3, "{ev:?}");
    // 읽는 순서대로 늘어서야 한다
    assert_eq!(ev.iter().map(|e| e.ord).collect::<Vec<_>>(), vec![0, 1, 2]);
    assert_eq!(ev.iter().filter(|e| e.neighbor).count(), 2);
    // 이름은 늘어놓은 순서대로
    assert_eq!(ev[0].source_id, "근거1");
    assert_eq!(ev[2].source_id, "근거3");
}

#[test]
fn 겹치는_이웃은_한_번만_넣는다() {
    // 이어진 두 청크가 다 검색되면, 서로의 이웃이 되어 두 번 들어갈 수 있다
    let c = db();
    add(&c, 0, 1, "앞 조각");
    add(&c, 1, 2, "자유수강권 첫째");
    add(&c, 2, 3, "자유수강권 둘째");
    add(&c, 3, 4, "뒤 조각");

    let hits = find(&c, "자유수강권");
    let ev = build(&c, &hits, DEFAULT_PLAN).unwrap();
    let ids: Vec<i64> = ev.iter().map(|e| e.chunk_id).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(ids.len(), sorted.len(), "같은 청크가 두 번 들어갔습니다: {ids:?}");
    assert_eq!(ev.len(), 4);
}

#[test]
fn 자리가_모자라면_이웃부터_버린다() {
    // 정작 찾은 근거가 빠지고 이웃만 남으면 안 된다
    let c = db();
    let long = "가".repeat(400);
    for i in 0..6 {
        add(&c, i, i + 1, &format!("자유수강권 {i} {long}"));
    }
    let hits = find(&c, "자유수강권");
    let ev = build(&c, &hits, Plan { top_k: 5, radius: 1, max_chars: 2500 }).unwrap();

    // 상위 근거가 먼저 자리를 차지한다
    assert!(ev.iter().filter(|e| !e.neighbor).count() >= 5, "{:?}", ev.len());
    let chars: usize = ev.iter().map(|e| e.text.chars().count()).sum();
    assert!(chars <= 2500 + 500, "너무 많이 담았습니다: {chars}자");
}

#[test]
fn 근거가_하나면_그것만_담는다() {
    let c = db();
    add(&c, 0, 1, "자유수강권 지원 대상");
    let hits = find(&c, "자유수강권");
    let ev = build(&c, &hits, DEFAULT_PLAN).unwrap();
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0].source_id, "근거1");
}

#[test]
fn 검색_결과가_없으면_근거도_없다() {
    let c = db();
    add(&c, 0, 1, "아무 상관 없는 글");
    let hits = find(&c, "자유수강권");
    assert!(hits.is_empty());
    assert!(build(&c, &hits, DEFAULT_PLAN).unwrap().is_empty());
}

#[test]
fn 원문_위치가_따라온다() {
    // 답변 화면에서 [원문 보기] 를 누를 수 있어야 한다
    let c = db();
    add(&c, 0, 7, "자유수강권 지원 대상");
    let hits = find(&c, "자유수강권");
    let ev = build(&c, &hits, DEFAULT_PLAN).unwrap();
    assert_eq!(ev[0].spans.len(), 1);
    assert_eq!(ev[0].spans[0].page, 7);
}

#[test]
fn 프롬프트에_넣을_꼴에_문서명과_쪽이_들어간다() {
    let c = db();
    add(&c, 0, 5, "자유수강권 지원 대상은 저소득층 학생이다");
    let hits = find(&c, "자유수강권");
    let ev = build(&c, &hits, DEFAULT_PLAN).unwrap();
    let text = render(&ev);
    assert!(text.contains("[근거1]"), "{text}");
    assert!(text.contains("길라잡이"), "{text}");
    assert!(text.contains("5쪽"), "{text}");
    assert!(text.contains("제1장"), "{text}");
    assert!(text.contains("저소득층 학생"), "{text}");
}
