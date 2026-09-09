//! 벡터 저장·되읽기·닮음 셈을 시험한다.
//!
//! 여기서는 **진짜 모델을 쓰지 않는다.** 손으로 만든 벡터로 셈이 맞는지만 본다.
//! 실제 모델로 재는 일은 골든 셋 쪽(`eval`)에서 한다.

use super::*;
use crate::db::schema::MIGRATIONS;

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
         VALUES (1, 1, '문서', 'a.pdf', 'x', 1, 1, 'ok', '2026-09-09')",
        [],
    )
    .unwrap();
    for ord in 0..3 {
        conn.execute(
            "INSERT INTO chunk(id, document_id, ord, heading_path, text, text_norm, kind,
                               page_start, page_end)
             VALUES (?1, 1, ?1, '', ?2, ?2, 'text', 1, 1)",
            rusqlite::params![ord + 1, format!("청크 {ord}")],
        )
        .unwrap();
    }
    conn
}

#[test]
fn 담은_그대로_되읽는다() {
    let v = vec![0.5f32, -0.25, 0.125, 0.0];
    let back = from_blob(&to_blob(&v)).unwrap();
    assert_eq!(v, back);
}

#[test]
fn 깨진_벡터는_한국어로_알려_준다() {
    let e = from_blob(&[1, 2, 3]).unwrap_err();
    assert!(format!("{e:?}").contains("다시 색인"), "{e:?}");
}

#[test]
fn 같은_방향이면_1_이_나온다() {
    assert!((cosine(&[1.0, 2.0], &[2.0, 4.0]) - 1.0).abs() < 1e-6);
    assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
    // 길이가 다르면 0. 모델을 바꿔 놓고 다시 색인하지 않은 경우다.
    assert_eq!(cosine(&[1.0, 0.0], &[1.0]), 0.0);
}

#[test]
fn 가까운_것부터_돌려준다() {
    let conn = db();
    save(&conn, 1, "테스트", &[1.0, 0.0]).unwrap();
    save(&conn, 2, "테스트", &[0.7, 0.7]).unwrap();
    save(&conn, 3, "테스트", &[0.0, 1.0]).unwrap();

    let (out, looked) = nearest(&conn, &[1.0, 0.05], &[], 3).unwrap();
    assert_eq!(looked, 3, "훑어본 청크 수");
    assert_eq!(out.iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![1, 2, 3]);
}

#[test]
fn 두_번_저장하면_덮어쓴다() {
    let conn = db();
    save(&conn, 1, "테스트", &[1.0, 0.0]).unwrap();
    save(&conn, 1, "테스트", &[0.0, 1.0]).unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM embedding", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
    let (out, _) = nearest(&conn, &[0.0, 1.0], &[], 1).unwrap();
    assert!(out[0].1 > 0.99, "{out:?}");
}

#[test]
fn 색인할_거리를_찾아_준다() {
    let conn = db();
    save(&conn, 2, "테스트", &[1.0, 0.0]).unwrap();
    let todo = missing(&conn, 1).unwrap();
    assert_eq!(todo.iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![1, 3]);
}

#[test]
fn 자료집을_고르면_그_안에서만_찾는다() {
    let conn = db();
    conn.execute(
        "INSERT INTO collection(id, name, created_at) VALUES (2, '다른 자료집', '2026-09-09')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size, page_count,
                              status, created_at)
         VALUES (2, 2, '다른 문서', 'b.pdf', 'y', 1, 1, 'ok', '2026-09-09')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO chunk(id, document_id, ord, heading_path, text, text_norm, kind,
                           page_start, page_end)
         VALUES (9, 2, 0, '', '다른 청크', '다른 청크', 'text', 1, 1)",
        [],
    )
    .unwrap();
    save(&conn, 1, "테스트", &[1.0, 0.0]).unwrap();
    save(&conn, 9, "테스트", &[1.0, 0.0]).unwrap();

    let (out, _) = nearest(&conn, &[1.0, 0.0], &[2], 10).unwrap();
    assert_eq!(out.iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![9]);
}

#[test]
fn 대체된_문서는_근거로_나오지_않는다() {
    let conn = db();
    save(&conn, 1, "테스트", &[1.0, 0.0]).unwrap();
    conn.execute(
        "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size, page_count,
                              status, created_at)
         VALUES (2, 1, '새 문서', 'b.pdf', 'y', 1, 1, 'ok', '2026-09-09')",
        [],
    )
    .unwrap();
    conn.execute("UPDATE document SET superseded_by = 2 WHERE id = 1", [])
        .unwrap();
    let (out, _) = nearest(&conn, &[1.0, 0.0], &[], 10).unwrap();
    assert!(out.is_empty(), "지난 문서가 나왔습니다: {out:?}");
}

#[test]
fn 뜻으로_찾은_결과에도_원문_위치가_붙는다() {
    let conn = db();
    conn.execute(
        "INSERT INTO chunk_span(chunk_id, page, char_start, char_end) VALUES (1, 7, 100, 260)",
        [],
    )
    .unwrap();
    save(&conn, 1, "테스트", &[1.0, 0.0]).unwrap();

    let res = semantic_search(
        &conn,
        &[1.0, 0.0],
        &Request { text: String::new(), collection_ids: vec![], limit: 5 },
    )
    .unwrap();
    assert_eq!(res.hits.len(), 1);
    let h = &res.hits[0];
    assert_eq!(h.rank, 1);
    assert_eq!(h.spans.len(), 1);
    assert_eq!((h.spans[0].page, h.spans[0].char_start, h.spans[0].char_end), (7, 100, 260));
    assert!(h.score > 0.99, "코사인이 점수 자리에 들어와야 합니다: {}", h.score);
}
