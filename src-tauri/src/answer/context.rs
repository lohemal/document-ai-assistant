//! LLM 에게 넘길 **근거를 고른다.**
//!
//! 검색이 찾아 준 것을 그대로 다 넣지 않는다. 세 가지 이유가 있다.
//!
//! 1. 근거가 많을수록 **답변이 느려진다.** 이 PC 는 CPU 로 돌므로 프롬프트가
//!    두 배면 첫 글자가 나오는 시간도 두 배다.
//! 2. 근거가 많을수록 **모델이 엉뚱한 것을 인용한다.** 스무 개를 주면 스무
//!    개 중에서 아무거나 고를 수 있다.
//! 3. 근거는 화면에 다 보여 줘야 한다. 사용자가 열어 볼 수 있는 만큼만 준다.
//!
//! 그래서 **상위 5개 + 각자의 앞뒤 ±1** 로 시작한다. 이웃을 붙이는 까닭은
//! 청크 경계가 문장 가운데를 지날 수 있고, 표의 머리와 값이 갈릴 수 있어서다.
//!
//! **이웃은 검색 순위에 끼어들지 않는다** (P4a 에서 정한 규칙). 순위는 이미
//! 정해진 뒤이고, 여기서는 문맥을 넓히기만 한다.

use crate::error::AppResult;
use crate::repo::chunk::{self, Span};
use crate::repo::search::{neighbors, Hit};
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    /// LLM 이 인용할 이름. `근거1` 처럼 사람도 읽을 수 있게 둔다 —
    /// 답변 글에 그대로 섞여 나와도 뜻이 통해야 하기 때문이다.
    pub source_id: String,
    pub document_id: i64,
    pub doc_title: String,
    pub chunk_id: i64,
    pub ord: i64,
    pub page_start: i64,
    pub page_end: i64,
    pub heading_path: Option<String>,
    pub text: String,
    /// 원문 형광펜 자리 (P3)
    pub spans: Vec<Span>,
    /// 검색이 고른 것인가(false), 이웃으로 딸려 온 것인가(true)
    pub neighbor: bool,
    /// 검색에서 몇 등이었나 — 개발용
    pub keyword_rank: Option<i64>,
    pub semantic_rank: Option<i64>,
    pub search_rank: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
pub struct Plan {
    /// 검색 결과 가운데 이만큼만 근거로 쓴다
    pub top_k: usize,
    /// 각 근거의 앞뒤로 이만큼 붙인다
    pub radius: i64,
    /// 근거 전체 글자 수 상한
    pub max_chars: usize,
}

/// 시작값. 골든 셋으로 확인한 뒤 필요하면 고친다.
///
/// `max_chars` 6,000자는 청크 여덟 개쯤이다. 4B 모델이 이 PC(CPU)에서
/// 프롬프트를 읽는 데만 20초쯤 걸리는 크기다 — 더 늘리면 기다리기 어렵다.
///
/// ⚠ 이 값을 늘릴 때는 **모델의 문맥 창**을 함께 봐야 한다. 한국어는
/// 1토큰이 1.5자쯤이라 6,000자면 4,000토큰이 넘고, Ollama 기본값(4,096)으로는
/// 앞쪽 근거가 조용히 잘린다. 그래서 `chat_stream` 이 num_ctx 를 정해 준다.
pub const DEFAULT_PLAN: Plan = Plan {
    top_k: 5,
    radius: 1,
    max_chars: 6000,
};

/// 검색 결과에서 근거 묶음을 만든다.
///
/// 순서: **검색이 고른 것을 먼저 다 넣고**, 그다음 이웃을 붙인다. 자리가
/// 모자랄 때 이웃이 아니라 정작 찾은 근거가 빠지면 안 되기 때문이다.
pub fn build(conn: &Connection, hits: &[Hit], plan: Plan) -> AppResult<Vec<Evidence>> {
    let picked: Vec<&Hit> = hits.iter().take(plan.top_k).collect();
    let mut out: Vec<Evidence> = Vec::new();
    let mut used = 0usize;

    // ① 검색이 고른 청크
    for h in &picked {
        if used > 0 && used + h.text.chars().count() > plan.max_chars {
            break;
        }
        used += h.text.chars().count();
        out.push(Evidence {
            source_id: String::new(), // 마지막에 매긴다
            document_id: h.document_id,
            doc_title: h.doc_title.clone(),
            chunk_id: h.chunk_id,
            ord: h.ord,
            page_start: h.page_start,
            page_end: h.page_end,
            heading_path: h.heading_path.clone(),
            text: h.text.clone(),
            spans: h.spans.clone(),
            neighbor: false,
            keyword_rank: h.keyword_rank,
            semantic_rank: h.semantic_rank,
            search_rank: Some(h.rank),
        });
    }

    // ② 이웃. 같은 청크가 여러 근거의 이웃일 수 있으므로 겹치는 것을 버린다.
    if plan.radius > 0 {
        for h in &picked {
            for id in neighbors(conn, h.chunk_id, plan.radius)? {
                if out.iter().any(|e| e.chunk_id == id) {
                    continue;
                }
                let c = chunk::get(conn, id)?;
                if used + c.text.chars().count() > plan.max_chars {
                    continue;
                }
                used += c.text.chars().count();
                out.push(Evidence {
                    source_id: String::new(),
                    document_id: c.document_id,
                    doc_title: h.doc_title.clone(),
                    chunk_id: c.id,
                    ord: c.ord,
                    page_start: c.page_start,
                    page_end: c.page_end,
                    heading_path: c.heading_path.clone(),
                    text: c.text,
                    spans: c.spans,
                    neighbor: true,
                    keyword_rank: None,
                    semantic_rank: None,
                    search_rank: None,
                });
            }
        }
    }

    // ③ 같은 문서·순서대로 늘어놓고 이름을 매긴다.
    //
    // 검색 순위대로 두지 않는 까닭: 이웃을 붙였으니 **읽는 순서**가 되어야
    // 모델이 앞뒤를 잇는다. 조각들이 순서 없이 섞여 있으면 "위에서 이어지는
    // 내용" 을 모델이 알아볼 수 없다.
    out.sort_by(|a, b| {
        a.document_id
            .cmp(&b.document_id)
            .then(a.ord.cmp(&b.ord))
    });
    for (i, e) in out.iter_mut().enumerate() {
        e.source_id = format!("근거{}", i + 1);
    }
    Ok(out)
}

/// 근거를 프롬프트에 넣을 꼴로 적는다.
///
/// 제목 경로를 함께 넣는다 — `Ⅳ. 회계관리 > 나. 주요업무` 는 그 조각이 무엇에
/// 관한 것인지 알려 주는 값싼 문맥이다.
///
/// **검색 몇 등이었는지도 함께 적는다.** 읽는 순서대로 늘어놓았기 때문에,
/// 그것만 주면 작은 모델은 맨 앞의 근거를 답으로 고르는 쪽으로 기운다.
/// 등수를 적어 주면 "가장 가까운 것" 을 알 수 있고, 이웃으로 딸려 온 것은
/// 답이 아니라 문맥이라는 것도 알 수 있다.
pub fn render(list: &[Evidence]) -> String {
    let mut s = String::new();
    for e in list {
        s.push_str(&format!(
            "[{}] {} {}쪽",
            e.source_id,
            e.doc_title,
            if e.page_start == e.page_end {
                e.page_start.to_string()
            } else {
                format!("{}~{}", e.page_start, e.page_end)
            }
        ));
        match e.search_rank {
            Some(r) => s.push_str(&format!(" · 물음과 가까운 순서 {r}위")),
            None => s.push_str(" · 앞뒤 문맥"),
        }
        if let Some(h) = &e.heading_path {
            if !h.is_empty() {
                s.push_str(&format!(" · {h}"));
            }
        }
        s.push('\n');
        s.push_str(e.text.trim());
        s.push_str("\n\n");
    }
    s
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
