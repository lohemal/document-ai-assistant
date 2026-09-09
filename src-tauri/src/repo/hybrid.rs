//! 낱말과 뜻을 섞어 찾는다.
//!
//! 두 방법은 서로 다른 것을 놓친다 (P4b 에서 실제 자료로 잰 값, 설계안 2-5):
//!   - 낱말: 숫자·고유명사에 강하다. `50만원`/`500,000원` 표기가 갈리면 약하다.
//!           바꿔쓰기 R@5 25%.
//!   - 뜻:   다른 말로 물어도 찾는다. 숫자를 흐리게 본다. 숫자 표기 R@5 75%.
//!   - 섞기: R@5 90.4% · MRR 0.733 — 둘 중 나쁜 쪽으로 끌려가지 않는다.
//!
//! **의미 색인이 없는 문서도 낱말로는 계속 찾힌다** (요구사항 8). 뜻 검색은
//! 벡터가 있는 청크만 보고, 낱말 검색은 모든 청크를 본다. 그 둘을 순위로
//! 섞으므로, 자료집 안에 색인된 문서와 안 된 문서가 섞여 있어도 색인 안 된
//! 문서가 검색에서 빠지지 않는다.

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
///
/// 사용자 설정으로 내보내지 않는다. 시험과 개발용으로만 바꾼다.
pub const DEFAULT_DEPTH: i64 = 20;

pub struct Hybrid<'a> {
    pub text: &'a str,
    pub query_vec: &'a [f32],
    /// 물음 벡터를 만든 모델의 태그. 벡터를 고르는 조건에 들어간다.
    pub model: &'a str,
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
        req.model,
        &req.collection_ids,
        depth as usize,
    )?;

    let kw_ids: Vec<i64> = kw.hits.iter().map(|h| h.chunk_id).collect();
    let sem_ids: Vec<i64> = sem.iter().map(|(id, _)| *id).collect();
    let fused = rrf::fuse(&[kw_ids.clone(), sem_ids.clone()], rrf::DEFAULT_K, None);

    // 각 방법에서 몇 등이었는지 함께 들고 간다 — 개발용 설명과 관련도 표시에 쓴다
    let scored: Vec<hits::Scored> = fused
        .iter()
        .map(|(id, score)| hits::Scored {
            chunk_id: *id,
            score: *score,
            keyword_rank: kw_ids.iter().position(|k| k == id).map(|i| i as i64 + 1),
            semantic_rank: sem_ids.iter().position(|s| s == id).map(|i| i as i64 + 1),
        })
        .collect();

    let hits = hits::fill(conn, &scored, &req.collection_ids, req.limit)?;

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
        mode: "hybrid".to_string(),
        mode_note: None,
    })
}

#[cfg(test)]
#[path = "hybrid_tests.rs"]
mod tests;
