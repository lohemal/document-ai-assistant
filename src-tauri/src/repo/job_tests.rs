//! 작업 기록 시험 — **과거 기록은 변하지 않는다**를 못 박는다.

use super::*;
use crate::db::schema::MIGRATIONS;

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for (_, sql) in MIGRATIONS {
        conn.execute_batch(sql).unwrap();
    }
    conn.execute_batch(
        "INSERT INTO collection(id, name, created_at) VALUES (1, '방과후학교', '2026-09-01T00:00:00+09:00');
         INSERT INTO document(id, collection_id, title, filename, sha256, byte_size, created_at)
              VALUES (10, 1, '길라잡이', 'afterschool.pdf', 'sha-A', 100, '2026-09-01T00:00:00+09:00');",
    )
    .unwrap();
    conn
}

fn ev(source_id: &str, document_id: i64, sha: &str) -> EvidenceIn {
    EvidenceIn {
        source_id: source_id.into(),
        document_id,
        doc_title: "길라잡이".into(),
        doc_sha256: sha.into(),
        collection_name: "방과후학교".into(),
        page_start: 73,
        page_end: 74,
        heading_path: Some("Ⅱ > 다".into()),
        quoted_text: "수용비는 강사료의 5% 이내".into(),
        chunk_id: Some(555),
        spans_json: r#"[{"page":73,"charStart":10,"charEnd":40}]"#.into(),
        cited: true,
    }
}

fn job(status: &str, evidence: Vec<EvidenceIn>) -> JobIn {
    JobIn {
        kind: "interpret".into(),
        question: "수용비는 강사료의 5퍼센트까지 쓸 수 있나요?".into(),
        collections_json: r#"[{"id":1,"name":"방과후학교"}]"#.into(),
        llm_model: Some("gemma3:4b".into()),
        embed_model: Some("bge-m3".into()),
        search_mode: "hybrid".into(),
        status: status.into(),
        answer_json: r#"{"answer":"5% 이내입니다."}"#.into(),
        evidence,
    }
}

#[test]
fn 담은_그대로_되읽는다() {
    let mut c = db();
    let id = save(&mut c, &job("answer", vec![ev("근거1", 10, "sha-A")])).unwrap();
    let d = get(&c, id).unwrap();
    assert_eq!(d.job.question, "수용비는 강사료의 5퍼센트까지 쓸 수 있나요?");
    assert_eq!(d.job.status, "answer");
    assert_eq!(d.job.search_mode, "hybrid");
    assert_eq!(d.job.llm_model.as_deref(), Some("gemma3:4b"));
    assert_eq!(d.job.evidence_count, 1);
    assert_eq!(d.answer_json, r#"{"answer":"5% 이내입니다."}"#);
    let e = &d.evidence[0];
    assert_eq!(e.source_id, "근거1");
    assert_eq!((e.page_start, e.page_end), (73, 74));
    assert_eq!(e.quoted_text, "수용비는 강사료의 5% 이내");
    assert!(e.cited);
    assert_eq!(e.doc_state, "same");
    assert!(e.can_open);
    assert_eq!(d.changed_count, 0);
}

#[test]
fn 문서가_바뀌어도_기록은_그대로다_지문이_다르면_잇지_않는다() {
    // ★ 핵심. 같은 문서 id 의 파일이 바뀌었다 (지문이 다르다).
    let mut c = db();
    let id = save(&mut c, &job("answer", vec![ev("근거1", 10, "sha-A")])).unwrap();
    c.execute("UPDATE document SET sha256 = 'sha-B' WHERE id = 10", []).unwrap();

    let d = get(&c, id).unwrap();
    let e = &d.evidence[0];
    // 당시 원문·쪽·지문은 그대로
    assert_eq!(e.quoted_text, "수용비는 강사료의 5% 이내");
    assert_eq!(e.doc_sha256, "sha-A");
    // 지금 문서와 다르다고 말하고, 원문을 지금 파일에 억지로 열지 않는다
    assert_eq!(e.doc_state, "changed");
    assert!(!e.can_open);
    assert!(e.note.as_deref().unwrap().contains("자료가 변경되었습니다"));
    assert_eq!(d.changed_count, 1);
}

#[test]
fn 새_판이_등록되면_변경으로_알리되_당시_판은_열_수_있다() {
    // 같은 이름의 파일을 다시 등록하면 옛 문서는 남고 superseded_by 가 붙는다 (설계안 6장)
    let mut c = db();
    let id = save(&mut c, &job("answer", vec![ev("근거1", 10, "sha-A")])).unwrap();
    c.execute_batch(
        "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size, created_at, revision_of)
              VALUES (11, 1, '길라잡이', 'afterschool.pdf', 'sha-B', 100, '2026-09-02T00:00:00+09:00', 10);
         UPDATE document SET superseded_by = 11 WHERE id = 10;",
    )
    .unwrap();
    let e = &get(&c, id).unwrap().evidence[0];
    assert_eq!(e.doc_state, "superseded");
    assert!(e.can_open, "옛 판 파일은 남아 있으므로 당시 판을 연다");
    assert!(e.note.as_deref().unwrap().contains("자료가 변경되었습니다"));
    assert!(e.note.as_deref().unwrap().contains("당시 판"));
}

#[test]
fn 문서가_지워지면_기록은_남고_삭제되었다고_말한다() {
    let mut c = db();
    let id = save(&mut c, &job("answer", vec![ev("근거1", 10, "sha-A")])).unwrap();
    c.execute("DELETE FROM document WHERE id = 10", []).unwrap();
    let d = get(&c, id).unwrap();
    let e = &d.evidence[0];
    assert_eq!(e.document_id, None, "ON DELETE SET NULL");
    assert_eq!(e.doc_state, "deleted");
    assert!(!e.can_open);
    assert_eq!(e.quoted_text, "수용비는 강사료의 5% 이내", "원문 복사본은 남는다");
}

#[test]
fn 멈춘_답변도_기록된다() {
    // 사용자가 멈추면 물음과 근거는 남기고 '답변 생성 중지' 로 담는다
    let mut c = db();
    let mut j = job("cancelled", vec![ev("근거1", 10, "sha-A")]);
    j.answer_json = r#"{"answer":"","cancelled":true}"#.into();
    let id = save(&mut c, &j).unwrap();
    assert_eq!(get(&c, id).unwrap().job.status, "cancelled");
}

#[test]
fn 최근과_중요를_따로_본다() {
    let mut c = db();
    let a = save_at(&mut c, &job("answer", vec![]), "2026-09-01T09:00:00+09:00").unwrap();
    let b = save_at(&mut c, &job("refuse", vec![]), "2026-09-02T09:00:00+09:00").unwrap();
    set_pinned(&c, a, true).unwrap();

    let recent = list(&c, false, 50).unwrap();
    assert_eq!(recent.iter().map(|r| r.id).collect::<Vec<_>>(), vec![b, a], "최근 것부터");
    let pinned = list(&c, true, 50).unwrap();
    assert_eq!(pinned.iter().map(|r| r.id).collect::<Vec<_>>(), vec![a]);
    assert!(pinned[0].pinned);
}

#[test]
fn 기한이_지난_것만_지우고_중요는_남긴다() {
    let mut c = db();
    let old = save_at(&mut c, &job("answer", vec![]), "2026-07-01T09:00:00+09:00").unwrap();
    let old_pinned = save_at(&mut c, &job("answer", vec![]), "2026-07-01T09:00:00+09:00").unwrap();
    let fresh = save_at(&mut c, &job("answer", vec![]), "2026-09-09T09:00:00+09:00").unwrap();
    set_pinned(&c, old_pinned, true).unwrap();

    let n = purge_expired(&c, 30, "2026-09-10T09:00:00+09:00").unwrap();
    assert_eq!(n, 1);
    let left: Vec<i64> = list(&c, false, 50).unwrap().iter().map(|r| r.id).collect();
    assert!(left.contains(&fresh));
    assert!(left.contains(&old_pinned), "중요 표시한 것은 지우지 않는다");
    assert!(!left.contains(&old));
}

#[test]
fn 보존기간_0_은_지우지_않는다() {
    let mut c = db();
    save_at(&mut c, &job("answer", vec![]), "2020-01-01T09:00:00+09:00").unwrap();
    assert_eq!(purge_expired(&c, 0, "2026-09-10T09:00:00+09:00").unwrap(), 0);
    assert_eq!(count(&c).unwrap(), 1);
}

#[test]
fn 보존기간은_정해진_값만_받는다() {
    let c = db();
    assert_eq!(retention_days(&c).unwrap(), 30, "V1 기본값");
    set_retention_days(&c, 90).unwrap();
    assert_eq!(retention_days(&c).unwrap(), 90);
    set_retention_days(&c, 0).unwrap();
    assert!(set_retention_days(&c, 45).is_err());
}

#[test]
fn 기록을_지우면_근거도_함께_사라진다() {
    let mut c = db();
    let id = save(&mut c, &job("answer", vec![ev("근거1", 10, "sha-A"), ev("근거2", 10, "sha-A")])).unwrap();
    delete(&c, id).unwrap();
    let n: i64 = c.query_row("SELECT COUNT(*) FROM job_evidence", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0, "ON DELETE CASCADE");
    assert!(get(&c, id).is_err());
}
