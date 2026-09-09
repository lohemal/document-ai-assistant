//! 색인 상태를 가리는 규칙을 시험한다.
//!
//! 여기서 틀리면 "의미 검색 준비됨" 이라고 보여 주면서 실제로는 옛 벡터로
//! 찾는 일이 생긴다. 화면에 아무 표시도 안 나므로 사용자가 알 수 없다.

use super::*;
use crate::db::schema::MIGRATIONS;
use crate::repo::chunk::hash_of;
use crate::repo::vector;

const MODEL: &str = "bge-m3";
const DIM: i64 = 4;

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

/// 문서 하나에 청크 `n` 개를 넣는다. 청크 글은 "글 0", "글 1" …
fn doc(conn: &Connection, id: i64, n: i64) {
    conn.execute(
        "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size, page_count,
                              status, embed_state, created_at)
         VALUES (?1, 1, ?2, 'a.pdf', ?2, 1, 1, 'ok', 'idle', '2026-09-09')",
        params![id, format!("문서 {id}")],
    )
    .unwrap();
    for ord in 0..n {
        let text = format!("글 {ord}");
        conn.execute(
            "INSERT INTO chunk(document_id, ord, heading_path, text, text_norm, kind,
                               page_start, page_end, hash)
             VALUES (?1, ?2, '', ?3, ?3, 'text', 1, 1, ?4)",
            params![id, ord, text, hash_of(&text)],
        )
        .unwrap();
    }
}

fn chunk_ids(conn: &Connection, document_id: i64) -> Vec<i64> {
    let mut st = conn
        .prepare("SELECT id FROM chunk WHERE document_id = ?1 ORDER BY ord")
        .unwrap();
    st.query_map([document_id], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<i64>, _>>()
        .unwrap()
}

/// 청크 하나에 지금 글·지금 모델로 벡터를 만들어 준다
fn embed(conn: &Connection, chunk_id: i64, model: &str) {
    let hash: String = conn
        .query_row("SELECT hash FROM chunk WHERE id = ?1", [chunk_id], |r| r.get(0))
        .unwrap();
    vector::save(conn, chunk_id, model, &hash, &[1.0, 0.0, 0.0, 0.0]).unwrap();
}

fn one(conn: &Connection) -> DocIndex {
    status(conn, MODEL, DIM, &[]).unwrap().pop().unwrap()
}

#[test]
fn 아무것도_안_했으면_미색인이다() {
    let c = db();
    doc(&c, 1, 3);
    let s = one(&c);
    assert_eq!(s.state, IndexState::None);
    assert_eq!((s.total, s.done), (3, 0));
    assert_eq!(s.label, "의미 검색 미색인");
}

#[test]
fn 청크가_없으면_색인할_것도_없다() {
    let c = db();
    doc(&c, 1, 0);
    assert_eq!(one(&c).state, IndexState::NoChunks);
}

#[test]
fn 다_만들면_완료다() {
    let c = db();
    doc(&c, 1, 3);
    for id in chunk_ids(&c, 1) {
        embed(&c, id, MODEL);
    }
    let s = one(&c);
    assert_eq!(s.state, IndexState::Done);
    assert_eq!((s.total, s.done), (3, 3));
    assert!(s.state.semantic_usable());
}

#[test]
fn 일부만_만들면_일부다() {
    let c = db();
    doc(&c, 1, 3);
    embed(&c, chunk_ids(&c, 1)[0], MODEL);
    let s = one(&c);
    assert_eq!(s.state, IndexState::Partial);
    assert_eq!((s.total, s.done), (3, 1));
    // 일부라도 뜻으로 찾을 수 있어야 한다 — 나머지는 낱말 검색이 받쳐 준다
    assert!(s.state.semantic_usable());
}

#[test]
fn 남은_청크만_이어서_만든다() {
    let c = db();
    doc(&c, 1, 3);
    let ids = chunk_ids(&c, 1);
    embed(&c, ids[0], MODEL);
    let left = todo(&c, 1, MODEL, DIM).unwrap();
    assert_eq!(left.len(), 2);
    assert_eq!(left.iter().map(|(id, _, _)| *id).collect::<Vec<_>>(), vec![ids[1], ids[2]]);
}

#[test]
fn 다른_모델로_만든_벡터는_쓰지_않는다() {
    let c = db();
    doc(&c, 1, 2);
    for id in chunk_ids(&c, 1) {
        embed(&c, id, "다른-모델");
    }
    let s = one(&c);
    assert_eq!(s.state, IndexState::ModelMismatch);
    assert_eq!(s.done, 0, "다른 모델 벡터를 셈에 넣었습니다");
    assert_eq!(s.other_model, 2);
    assert_eq!(s.indexed_with.as_deref(), Some("다른-모델"));
    assert!(!s.state.semantic_usable());
    // 이어서 할 거리로는 전부 남아 있어야 한다
    assert_eq!(todo(&c, 1, MODEL, DIM).unwrap().len(), 2);
}

#[test]
fn 청크_글이_바뀌면_다시_색인해야_한다() {
    let c = db();
    doc(&c, 1, 2);
    let ids = chunk_ids(&c, 1);
    for id in &ids {
        embed(&c, *id, MODEL);
    }
    assert_eq!(one(&c).state, IndexState::Done);

    // 문서를 다시 추출했다고 하자 — 글이 달라졌다
    let 새글 = "글이 바뀌었다";
    c.execute(
        "UPDATE chunk SET text_norm = ?2, hash = ?3 WHERE id = ?1",
        params![ids[0], 새글, hash_of(새글)],
    )
    .unwrap();

    let s = one(&c);
    assert_eq!(s.state, IndexState::NeedsReindex, "{s:?}");
    assert_eq!(s.done, 1, "바뀐 청크의 옛 벡터를 그대로 셌습니다");
    assert_eq!(s.stale, 1);
    // 바뀐 청크만 다시 만들면 된다
    let left = todo(&c, 1, MODEL, DIM).unwrap();
    assert_eq!(left.iter().map(|(id, _, _)| *id).collect::<Vec<_>>(), vec![ids[0]]);
}

#[test]
fn 벡터_길이가_다르면_쓰지_않는다() {
    // 같은 태그라도 모델이 바뀌어 길이가 달라질 수 있다
    let c = db();
    doc(&c, 1, 1);
    let id = chunk_ids(&c, 1)[0];
    let hash: String = c
        .query_row("SELECT hash FROM chunk WHERE id = ?1", [id], |r| r.get(0))
        .unwrap();
    vector::save(&c, id, MODEL, &hash, &[1.0, 0.0]).unwrap(); // 2차원
    let s = one(&c);
    assert_eq!(s.state, IndexState::ModelMismatch);
    assert_eq!(s.done, 0);
}

#[test]
fn 쓸_수_없는_벡터를_지우고_다시_시작할_수_있다() {
    let c = db();
    doc(&c, 1, 2);
    for id in chunk_ids(&c, 1) {
        embed(&c, id, "다른-모델");
    }
    let dropped = drop_unusable(&c, 1, MODEL, DIM).unwrap();
    assert_eq!(dropped, 2);
    assert_eq!(one(&c).state, IndexState::None);
}

#[test]
fn 도는_중에_꺼졌으면_멈춤으로_돌린다() {
    let c = db();
    doc(&c, 1, 3);
    doc(&c, 2, 3);
    set_work_state(&c, 1, RUNNING, None).unwrap();
    set_work_state(&c, 2, QUEUED, None).unwrap();

    let n = reset_running(&c).unwrap();
    assert_eq!(n, 2);
    let all = status(&c, MODEL, DIM, &[]).unwrap();
    // 만들어 둔 것이 하나도 없어도 '멈춤' 으로 보인다.
    // "하려다 멈췄다" 는 사실을 지우지 않는 편이 낫다 — 사용자가 시작해 둔
    // 일이 조용히 '미색인' 으로 되돌아가 있으면, 왜 안 됐는지 알 수 없다.
    assert_eq!(all[0].state, IndexState::Paused);
    assert_eq!(all[1].state, IndexState::Paused);
    assert_eq!(all[0].done, 0);
}

#[test]
fn 멈춘_색인은_이어서_하기로_보인다() {
    let c = db();
    doc(&c, 1, 3);
    embed(&c, chunk_ids(&c, 1)[0], MODEL);
    set_work_state(&c, 1, PAUSED, None).unwrap();
    let s = one(&c);
    assert_eq!(s.state, IndexState::Paused);
    assert_eq!((s.total, s.done), (3, 1));
}

#[test]
fn 실패한_까닭이_남는다() {
    let c = db();
    doc(&c, 1, 3);
    set_work_state(&c, 1, FAILED, Some("AI 실행환경에 붙지 못했습니다.")).unwrap();
    let s = one(&c);
    assert_eq!(s.state, IndexState::Failed);
    assert_eq!(s.error.as_deref(), Some("AI 실행환경에 붙지 못했습니다."));
    assert!(s.state.needs_action());
}

#[test]
fn 다_끝난_문서는_멈춤이_아니라_완료다() {
    // 마지막 청크를 만든 뒤 상태를 못 바꾼 채 꺼진 경우
    let c = db();
    doc(&c, 1, 2);
    for id in chunk_ids(&c, 1) {
        embed(&c, id, MODEL);
    }
    set_work_state(&c, 1, PAUSED, None).unwrap();
    assert_eq!(one(&c).state, IndexState::Done);
}

#[test]
fn 자료집을_한_줄로_요약한다() {
    let c = db();
    doc(&c, 1, 2);
    doc(&c, 2, 2);
    doc(&c, 3, 2);
    for id in chunk_ids(&c, 1) {
        embed(&c, id, MODEL);
    }
    embed(&c, chunk_ids(&c, 2)[0], MODEL); // 일부

    let s = collection_summary(&c, MODEL, DIM, 1).unwrap();
    assert_eq!(s.documents, 3);
    assert_eq!(s.ready, 2, "일부 색인도 쓸 수 있는 것으로 센다");
    assert_eq!(s.summary, "문서 3개 중 2개 의미 검색 준비됨");
}

#[test]
fn 대체된_문서는_색인_목록에_없다() {
    let c = db();
    doc(&c, 1, 2);
    doc(&c, 2, 2);
    c.execute("UPDATE document SET superseded_by = 2 WHERE id = 1", [])
        .unwrap();
    let all = status(&c, MODEL, DIM, &[]).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].document_id, 2);
}
