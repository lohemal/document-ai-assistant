//! 낱말 검색을 **실제 시험 자료**로 시험한다.
//!
//! `npm run fixtures:chunks` 가 내보낸 청크를 읽어 자료를 만든다. 손으로
//! 지어낸 글로 검색을 시험하면, 실제 PDF 에서 나온 글의 생김새(탭으로 나뉜 표,
//! 붙어 있는 조사, 쉼표를 없앤 숫자)를 놓친다.

use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    collections: Vec<FixCollection>,
    documents: Vec<FixDocument>,
}

#[derive(Deserialize)]
struct FixCollection {
    id: i64,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixDocument {
    id: i64,
    collection_id: i64,
    title: String,
    filename: String,
    chunks: Vec<FixChunk>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixChunk {
    ord: i64,
    heading_path: String,
    text: String,
    text_norm: String,
    kind: String,
    page_start: i64,
    page_end: i64,
    spans: Vec<FixSpan>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixSpan {
    page: i64,
    char_start: i64,
    char_end: i64,
}

/// 자료를 만들고, 청크 id 를 "문서제목#순번" 으로 읽을 수 있게 짝지어 돌려준다.
fn load() -> (Connection, Vec<(i64, String)>) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../test/search/chunks.json");
    let raw = std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("{path} 를 읽지 못했습니다 ({e}). `npm run fixtures:chunks` 를 먼저 돌리세요.")
    });
    let fx: Fixture = serde_json::from_str(&raw).unwrap();

    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for (_, sql) in crate::db::schema::MIGRATIONS {
        conn.execute_batch(sql).unwrap();
    }

    for c in &fx.collections {
        conn.execute(
            "INSERT INTO collection(id, name, created_at) VALUES (?1, ?2, '2026-09-09')",
            rusqlite::params![c.id, c.name],
        )
        .unwrap();
    }

    let mut names: Vec<(i64, String)> = Vec::new();

    for d in &fx.documents {
        conn.execute(
            "INSERT INTO document(id, collection_id, title, filename, sha256, byte_size,
                                  page_count, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, 1, 'ok', '2026-09-09')",
            rusqlite::params![d.id, d.collection_id, d.title, d.filename, d.title],
        )
        .unwrap();

        for ch in &d.chunks {
            conn.execute(
                "INSERT INTO chunk(document_id, ord, heading_path, text, text_norm, kind,
                                   page_start, page_end)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    d.id,
                    ch.ord,
                    ch.heading_path,
                    ch.text,
                    ch.text_norm,
                    ch.kind,
                    ch.page_start,
                    ch.page_end
                ],
            )
            .unwrap();
            let cid = conn.last_insert_rowid();
            names.push((cid, format!("{}#{}", d.title, ch.ord)));
            for s in &ch.spans {
                conn.execute(
                    "INSERT INTO chunk_span(chunk_id, page, char_start, char_end)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![cid, s.page, s.char_start, s.char_end],
                )
                .unwrap();
            }
        }
    }

    (conn, names)
}

fn find(conn: &Connection, text: &str, collections: Vec<i64>) -> SearchResult {
    keyword_search(
        conn,
        &Request {
            text: text.into(),
            collection_ids: collections,
            limit: 20,
        },
    )
    .unwrap()
}

fn labels(res: &SearchResult, names: &[(i64, String)]) -> Vec<String> {
    res.hits
        .iter()
        .map(|h| {
            names
                .iter()
                .find(|(id, _)| *id == h.chunk_id)
                .map(|(_, n)| n.clone())
                .unwrap_or_default()
        })
        .collect()
}

// ── 정확한 낱말 ──────────────────────────────────────────────────────

#[test]
fn 낱말을_그대로_찾는다() {
    let (c, n) = load();
    let r = find(&c, "자유수강권", vec![]);
    let got = labels(&r, &n);
    assert!(!got.is_empty(), "자유수강권 이 하나도 안 걸렸습니다");
    assert!(
        got.iter().all(|l| l.starts_with("cid-ko")),
        "엉뚱한 것이 걸렸습니다: {got:?}"
    );
}

// ── 조사 ─────────────────────────────────────────────────────────────

#[test]
fn 문서에_조사가_붙어_있어도_찾는다() {
    // 문서: "자유수강권 운영지침을 …" / 찾는 말: "자유수강권"
    // trigram 이 부분 일치라서 된다. 이게 trigram 을 고른 까닭이다.
    let (c, n) = load();
    let r = find(&c, "자유수강권", vec![]);
    assert!(labels(&r, &n).contains(&"cid-ko#0".to_string()));
}

#[test]
fn 찾는_말에_조사가_붙어_있어도_찾는다() {
    // 반대 방향. trigram 만으로는 안 된다 — 조사를 떼어 낸 꼴을 함께 찾기
    // 때문에 된다 (domain::query).
    let (c, n) = load();
    let with = find(&c, "자유수강권을", vec![]);
    let without = find(&c, "자유수강권", vec![]);
    assert!(!with.hits.is_empty(), "조사가 붙은 말로는 아무것도 못 찾았습니다");
    assert_eq!(labels(&with, &n), labels(&without, &n));
}

// ── 낱말의 일부 ──────────────────────────────────────────────────────

#[test]
fn 낱말의_일부로도_찾는다() {
    let (c, n) = load();
    let r = find(&c, "수강권", vec![]);
    assert!(labels(&r, &n).contains(&"cid-ko#0".to_string()));
}

// ── 여러 핵심어의 순위 ★ ─────────────────────────────────────────────

#[test]
fn 낱말이_더_많이_든_청크가_앞에_온다() {
    // 초등학교 : plain-ko#0, plain-ko#2, cid-ko#0, table-ko#0
    // 지원액   : table-ko#0
    // 500000원 : table-ko#0
    // -> 셋 다 든 table-ko#0 이 1위여야 한다
    let (c, n) = load();
    let r = find(&c, "초등학교 지원액 500000원", vec![]);
    let got = labels(&r, &n);
    assert_eq!(got.first().map(String::as_str), Some("table-ko#0"), "{got:?}");
    assert_eq!(r.hits[0].matched, 3);
    assert!(r.hits[1..].iter().all(|h| h.matched < 3));
}

// ── 숫자 표기 ────────────────────────────────────────────────────────

#[test]
fn 쉼표가_있든_없든_같은_숫자로_찾는다() {
    let (c, n) = load();
    let with = find(&c, "500,000원", vec![]);
    let without = find(&c, "500000원", vec![]);
    assert_eq!(labels(&with, &n), vec!["table-ko#0".to_string()]);
    assert_eq!(labels(&with, &n), labels(&without, &n));
}

#[test]
fn 만_단위_표기는_아직_못_찾는다() {
    // 알려진 한계다. 50만원 과 500,000원 은 글자가 아예 다르다. 낱말 검색만
    // 으로는 이을 수 없고, P4b 의 뜻 검색이 다룰 몫이다. 못 찾는다는 것을
    // 시험으로 붙잡아 둔다 — 나중에 되게 만들면 이 시험이 깨져서 알려 준다.
    let (c, _) = load();
    let r = find(&c, "50만원", vec![]);
    assert!(r.hits.is_empty(), "이제 찾을 수 있게 되었다면 이 시험을 고치세요");
}

// ── 자연어 질문 ──────────────────────────────────────────────────────

#[test]
fn 자연어_질문에서도_알맞은_청크가_1위다() {
    let (c, n) = load();
    let r = find(&c, "초등학교 3학년의 1인당 지원액은 얼마야?", vec![]);
    let got = labels(&r, &n);
    assert_eq!(got.first().map(String::as_str), Some("table-ko#0"), "{got:?}");
    assert!(r.dropped_terms.contains(&"얼마야".to_string()));
}

// ── 자료집 필터 ★ ────────────────────────────────────────────────────

#[test]
fn 자료집을_고르면_그_안에서만_찾는다() {
    // 초등학교 는 두 자료집 모두에 있다
    let (c, n) = load();

    let all = labels(&find(&c, "초등학교", vec![]), &n);
    let one = labels(&find(&c, "초등학교", vec![1]), &n);
    let two = labels(&find(&c, "초등학교", vec![2]), &n);

    assert!(all.len() > one.len(), "전체가 자료집 하나보다 많아야 한다");
    assert!(
        one.iter()
            .all(|l| l.starts_with("plain-ko") || l.starts_with("cid-ko")),
        "자료집 1 밖의 것이 섞였습니다: {one:?}"
    );
    assert!(
        two.iter().all(|l| l.starts_with("table-ko")
            || l.starts_with("manual-ko")
            || l.starts_with("repeat-ko")
            || l.starts_with("odd-layer")),
        "자료집 2 밖의 것이 섞였습니다: {two:?}"
    );
    assert_eq!(one.len() + two.len(), all.len());
}

#[test]
fn 자료집_여러_개를_한꺼번에_고를_수_있다() {
    let (c, n) = load();
    let both = labels(&find(&c, "초등학교", vec![1, 2]), &n);
    let all = labels(&find(&c, "초등학교", vec![]), &n);
    assert_eq!(both.len(), all.len());
}

// ── 짧은 낱말 ────────────────────────────────────────────────────────

#[test]
fn 두_글자_낱말도_찾는다() {
    // trigram 은 세 글자 아래를 색인하지 못한다. 훑어서 찾는다.
    let (c, n) = load();
    let r = find(&c, "교재", vec![]);
    let got = labels(&r, &n);
    assert!(!got.is_empty(), "교재 를 하나도 못 찾았습니다");
    assert!(r.short_terms.contains(&"교재".to_string()));
}

// ── 표 안의 자료 ─────────────────────────────────────────────────────

#[test]
fn 표_안의_값으로도_찾는다() {
    let (c, n) = load();
    let r = find(&c, "중위소득", vec![]);
    assert!(labels(&r, &n).contains(&"table-ko#1".to_string()));
}

// ── 찾을 것이 없을 때 ────────────────────────────────────────────────

#[test]
fn 물음말만_있으면_까닭을_알려_준다() {
    let (c, _) = load();
    let r = find(&c, "얼마야?", vec![]);
    assert!(r.hits.is_empty());
    assert!(r.note.is_some(), "왜 못 찾았는지 알려 줘야 한다");
}

#[test]
fn 없는_말은_없다고_한다() {
    let (c, _) = load();
    let r = find(&c, "교원성과상여금", vec![]);
    assert!(r.hits.is_empty());
}

// ── 이웃 청크는 검색과 따로 논다 ★ ───────────────────────────────────

#[test]
fn 이웃_청크는_검색_순위에_끼어들지_않는다() {
    // 검색이 청크를 고르고, 문맥을 넓히는 일은 그 뒤에 따로 한다.
    // 섞으면 나중에 검색 품질을 따로 잴 수 없다.
    let (c, n) = load();
    let r = find(&c, "중위소득", vec![]);
    let top = r.hits[0].chunk_id;

    let around = neighbors(&c, top, 1).unwrap();
    assert!(around.contains(&top));
    assert!(around.len() > 1, "이웃이 있어야 한다");

    let again = find(&c, "중위소득", vec![]);
    assert_eq!(labels(&r, &n), labels(&again, &n));
}

// ── 근거 위치가 함께 온다 ────────────────────────────────────────────

#[test]
fn 검색_결과에_원문_위치가_함께_온다() {
    // 이게 있어야 결과를 눌러 PDF 형광펜으로 이어진다
    let (c, _) = load();
    let r = find(&c, "중위소득", vec![]);
    let h = &r.hits[0];
    assert!(!h.spans.is_empty(), "가리키는 자리가 없으면 근거를 못 보여 준다");
    assert!(h.spans.iter().all(|s| s.char_end > s.char_start));
    assert!(h.page_start >= 1);
}

// ── 자료가 많아지면 얼마나 걸리는가 ──────────────────────────────────

/// 시험 자료를 여러 벌 복사해 늘린다. 실제 학교 자료집 규모를 흉내 낸다.
fn inflate(conn: &Connection, times: i64) -> i64 {
    let rows: Vec<(i64, i64, Option<String>, String, String, String, i64, i64)> = {
        let mut st = conn
            .prepare(
                "SELECT document_id, ord, heading_path, text, text_norm, kind, page_start, page_end
                   FROM chunk",
            )
            .unwrap();
        st.query_map([], |r| {
            Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    };

    for _ in 0..times {
        for r in &rows {
            conn.execute(
                "INSERT INTO chunk(document_id, ord, heading_path, text, text_norm, kind,
                                   page_start, page_end)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7],
            )
            .unwrap();
            let cid = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO chunk_span(chunk_id, page, char_start, char_end)
                 VALUES (?1, ?2, 0, 10)",
                rusqlite::params![cid, r.6],
            )
            .unwrap();
        }
    }
    conn.query_row("SELECT COUNT(*) FROM chunk", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn 자료가_많아도_충분히_빠르다() {
    let (c, _) = load();
    let total = inflate(&c, 120);

    // 한 번은 몸을 풀고
    let _ = find(&c, "초등학교 지원액", vec![]);

    let mut worst = 0i64;
    for q in [
        "자유수강권",
        "초등학교 지원액 500000원",
        "초등학교 3학년의 1인당 지원액은 얼마야?",
        "교재",
    ] {
        let r = find(&c, q, vec![]);
        worst = worst.max(r.elapsed_ms);
        println!(
            "  청크 {total}개 · \"{q}\" -> 후보 {}개 · {}ms",
            r.candidates, r.elapsed_ms
        );
    }

    // 사람이 기다린다고 느끼기 시작하는 선을 넉넉히 잡았다.
    // 여기 걸리면 검색이 느려진 것이므로 들여다봐야 한다.
    assert!(worst < 1500, "가장 느린 검색이 {worst}ms 걸렸습니다 (청크 {total}개)");
}
