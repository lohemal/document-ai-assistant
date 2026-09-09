//! 낱말과 뜻을 섞어 찾는다.
//!
//! 두 방법은 서로 다른 것을 놓친다.
//!   - 낱말: 원문 표현을 그대로 쓴 물음에 강하다. `50만원` / `500,000원` 처럼
//!           표기가 다르면 못 찾는다.
//!   - 뜻:   다른 말로 물어도 찾는다. 숫자와 고유명사는 흐릿해진다.
//!
//! 그래서 각각 앞쪽 몇 개를 가져와 **순위로** 섞는다(RRF). 점수를 더하지
//! 않는 까닭은 `domain::rrf` 에 적어 두었다.

use super::search::{keyword_search, Request, SearchResult};
use super::{hits, vector};
use crate::domain::rrf;
use crate::error::AppResult;
use rusqlite::Connection;

/// 각 방법에서 이만큼씩 가져와 섞는다.
///
/// 실제 자료 골든 셋(52문항)으로 견준 값 — R@5 는 10 에서 88.5%, 20·30 에서
/// 90.4%, 50 에서 86.5% 였다. **거의 흔들리지 않는다.** 그래서 흔들리지 않는
/// 구간에서 가장 빠른 20 으로 둔다(35ms 대 50ms). 점수가 가장 높아서 고른 값이
/// 아니다 — 문항에 맞춰 세밀하게 맞추면 실제 자료에서 어떻게 될지 알 수 없다.
pub const DEFAULT_DEPTH: i64 = 20;

pub struct Hybrid<'a> {
    pub text: &'a str,
    pub query_vec: &'a [f32],
    pub collection_ids: Vec<i64>,
    pub limit: i64,
    pub depth: i64,
}

pub fn hybrid_search(conn: &Connection, req: &Hybrid) -> AppResult<SearchResult> {
    let started = std::time::Instant::now();
    let depth = req.depth.max(1);

    let kw = keyword_search(
        conn,
        &Request {
            text: req.text.to_string(),
            collection_ids: req.collection_ids.clone(),
            limit: depth,
        },
    )?;
    let (sem, looked) = vector::nearest(
        conn,
        req.query_vec,
        &req.collection_ids,
        depth as usize,
    )?;

    let lists = vec![
        kw.hits.iter().map(|h| h.chunk_id).collect::<Vec<_>>(),
        sem.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
    ];
    let fused = rrf::fuse(&lists, rrf::DEFAULT_K, None);
    let hits = hits::fill(conn, &fused, &req.collection_ids, req.limit)?;

    Ok(SearchResult {
        hits,
        // 낱말 쪽에서 무엇을 찾아봤는지는 화면에 그대로 보여 준다.
        // 섞었다고 해서 "조사를 뗐다" 는 사실을 숨기면 사용자가 결과를 읽을 수 없다.
        terms: kw.terms,
        short_terms: kw.short_terms,
        dropped_terms: kw.dropped_terms,
        candidates: kw.candidates + looked,
        elapsed_ms: started.elapsed().as_millis() as i64,
        note: kw.note,
    })
}
