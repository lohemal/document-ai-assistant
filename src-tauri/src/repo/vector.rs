//! 뜻으로 찾는다 — 청크 벡터와 물음 벡터의 코사인 닮음.
//!
//! **벡터 전용 DB 를 두지 않는다.** 학교 자료 규모(자료집 하나에 청크 수백~
//! 수천)에서는 전부 훑어 셈해도 충분히 빠르고, 색인 구조(HNSW 같은 것)를
//! 들이면 그것을 저장하고 되살리고 망가졌을 때 고치는 일이 전부 새로 생긴다.
//! 그 값이 이 규모에서는 남지 않는다. 느려지면 그때 바꾼다 — 그때가 언제인지
//! 알 수 있게 `자료가_늘어도_버틴다` 시험에 청크 수를 적어 둔다.
//!
//! 벡터는 f32 리틀엔디언으로 이어 붙여 BLOB 하나에 담는다.

use super::search::{Request, SearchResult};
use crate::error::{AppError, AppResult};
use rusqlite::{Connection, ToSql};

/// f32 벡터를 BLOB 으로.
pub fn to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// BLOB 을 f32 벡터로. 길이가 4의 배수가 아니면 자료가 깨진 것이다.
pub fn from_blob(b: &[u8]) -> AppResult<Vec<f32>> {
    if b.len() % 4 != 0 {
        return Err(AppError::msg("저장된 벡터가 깨졌습니다. 자료집을 다시 색인해 주세요."));
    }
    Ok(b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

/// 청크 하나의 벡터를 저장한다. 이미 있으면 덮어쓴다.
///
/// `chunk_hash` 를 함께 담는 것이 핵심이다 — 나중에 청크 글이 바뀌면 이 값이
/// 어긋나고, 그 벡터는 검색에서 조용히 제외된다 (`repo::embed_index`).
pub fn save(
    conn: &Connection,
    chunk_id: i64,
    model: &str,
    chunk_hash: &str,
    vec: &[f32],
) -> AppResult<()> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO embedding(chunk_id, model, dim, vec, chunk_hash, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(chunk_id) DO UPDATE SET
           model = ?2, dim = ?3, vec = ?4, chunk_hash = ?5, created_at = ?6",
        rusqlite::params![chunk_id, model, vec.len() as i64, to_blob(vec), chunk_hash, now],
    )?;
    Ok(())
}

/// 코사인 닮음. 둘 다 길이가 0 이 아니라고 보고 셈한다.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0f32;
    let mut na = 0f32;
    let mut nb = 0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// 물음 벡터에 가까운 청크를 점수와 함께 돌려준다. 순위는 점수 내림차순.
///
/// **지금 모델·지금 글로 만든 벡터만 본다.** 이 조건이 여기 있는 것이 중요하다 —
/// 부르는 쪽에서 잊어도 옛 벡터가 검색에 섞이지 않는다. 어긋난 벡터는 조용히
/// 빠지고, 그 사실은 화면이 `repo::embed_index` 로 따로 알린다.
///
/// 화면에 보여 줄 꼴로 채우는 일은 `hits::fill` 이 한다.
pub fn nearest(
    conn: &Connection,
    query: &[f32],
    model: &str,
    collections: &[i64],
    take: usize,
) -> AppResult<(Vec<(i64, f64)>, i64)> {
    let coll = if collections.is_empty() {
        String::new()
    } else {
        let h = collections.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        format!(" AND d.collection_id IN ({h})")
    };
    let sql = format!(
        "SELECT e.chunk_id, e.vec FROM embedding e
           JOIN chunk c ON c.id = e.chunk_id
           JOIN document d ON d.id = c.document_id
          WHERE d.superseded_by IS NULL
            AND e.model = ?1 AND e.dim = ?2 AND e.chunk_hash = c.hash{coll}"
    );

    let mut params: Vec<Box<dyn ToSql>> =
        vec![Box::new(model.to_string()), Box::new(query.len() as i64)];
    for id in collections {
        params.push(Box::new(*id));
    }

    let mut st = conn.prepare(&sql)?;
    let mut rows = st.query(rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())))?;

    let mut scored: Vec<(i64, f64)> = Vec::new();
    let mut looked = 0i64;
    while let Some(r) = rows.next()? {
        let id: i64 = r.get(0)?;
        let blob: Vec<u8> = r.get(1)?;
        let v = from_blob(&blob)?;
        looked += 1;
        scored.push((id, cosine(query, &v) as f64));
    }

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    scored.truncate(take);
    Ok((scored, looked))
}

/// 뜻으로 찾은 결과를 낱말 검색과 같은 꼴로 돌려준다.
pub fn semantic_search(
    conn: &Connection,
    query: &[f32],
    model: &str,
    req: &Request,
) -> AppResult<SearchResult> {
    let started = std::time::Instant::now();
    let (scored, looked) =
        nearest(conn, query, model, &req.collection_ids, req.limit.max(1) as usize)?;
    let scored: Vec<super::hits::Scored> = scored
        .iter()
        .enumerate()
        .map(|(i, (id, score))| super::hits::Scored {
            chunk_id: *id,
            score: *score,
            keyword_rank: None,
            semantic_rank: Some(i as i64 + 1),
        })
        .collect();
    let hits = super::hits::fill(conn, &scored, &req.collection_ids, req.limit)?;
    Ok(SearchResult {
        hits,
        terms: vec![],
        short_terms: vec![],
        dropped_terms: vec![],
        candidates: looked,
        elapsed_ms: started.elapsed().as_millis() as i64,
        note: None,
        // 뜻만 쓰는 검색은 화면에 내놓지 않는다 — 시험과 평가에서만 쓴다.
        // (섞기가 낱말을 늘 함께 보므로, 사용자에게는 두 가지 방식만 보인다)
        mode: "semantic".to_string(),
        mode_note: None,
    })
}

#[cfg(test)]
#[path = "vector_tests.rs"]
mod tests;
