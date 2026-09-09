//! 의미 색인이 문서마다 어디까지 되었는지 가린다.
//!
//! **여기서 지키는 규칙 하나: 벡터를 조용히 다시 쓰지 않는다.**
//!
//! 벡터가 쓸 만한지는 세 가지가 모두 맞아야 정해진다.
//!   ① 어느 모델로 만들었나 (`embedding.model`)
//!   ② 몇 차원인가 (`embedding.dim`)
//!   ③ **어느 글로 만들었나** (`embedding.chunk_hash` = `chunk.hash`)
//!
//! ③ 이 핵심이다. 문서를 다시 등록하거나 청크 나누는 규칙을 손보면 청크 글이
//! 달라진다. 그때 옛 벡터가 남아 있으면 검색은 **다른 글을 가리키는 벡터**로
//! 답을 찾는다. 화면에는 아무 표시도 안 나고, 근거는 엉뚱한 자리를 가리킨다.
//! 환각보다 나쁘다 — 사용자는 자료가 맞다고 믿고 있기 때문이다.
//!
//! 그래서 "색인 완료"·"모델 불일치"·"재색인 필요" 는 **담아 두지 않고 그때그때
//! 센다.** 담아 두면 모델을 바꾼 순간 거짓이 된다. 담는 것은 파생할 수 없는
//! 일의 상태뿐이다 — 대기·도는 중·멈춤·실패.

use crate::error::AppResult;
use rusqlite::{params, Connection, ToSql};
use serde::Serialize;

/// 담아 두는 일의 상태. 파생할 수 없는 것만 여기 있다.
pub const IDLE: &str = "idle";
pub const QUEUED: &str = "queued";
pub const RUNNING: &str = "running";
pub const PAUSED: &str = "paused";
pub const FAILED: &str = "failed";

/// 화면에 보여 줄 상태. 일의 상태 + 세어 본 값으로 정한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexState {
    /// 청크가 없다 — 스캔본이거나 아직 추출 전
    NoChunks,
    /// 의미 검색 미색인
    None,
    /// 색인 대기
    Queued,
    /// 색인 중
    Running,
    /// 색인 멈춤 (이어서 할 수 있다)
    Paused,
    /// 색인 실패
    Failed,
    /// 일부만 색인됨 (이어서 할 수 있다)
    Partial,
    /// 색인 완료
    Done,
    /// 다른 검색 모델로 색인되어 있다
    ModelMismatch,
    /// 문서나 청크가 바뀌어 다시 색인해야 한다
    NeedsReindex,
}

impl IndexState {
    /// 화면에 그대로 쓸 짧은 말
    pub fn label(self) -> &'static str {
        match self {
            IndexState::NoChunks => "글자 없음",
            IndexState::None => "의미 검색 미색인",
            IndexState::Queued => "색인 대기",
            IndexState::Running => "색인 중",
            IndexState::Paused => "색인 멈춤",
            IndexState::Failed => "색인 실패",
            IndexState::Partial => "색인 일부",
            IndexState::Done => "의미 검색 준비됨",
            IndexState::ModelMismatch => "다른 모델로 색인됨",
            IndexState::NeedsReindex => "자료가 바뀜 — 다시 색인 필요",
        }
    }

    /// 이 상태에서 의미 검색을 쓸 수 있는가.
    ///
    /// `Partial` 도 쓸 수 있다 — 만들어 둔 청크까지는 뜻으로 찾고, 나머지는
    /// 낱말 검색이 받쳐 준다(요구사항 8).
    pub fn semantic_usable(self) -> bool {
        matches!(
            self,
            IndexState::Done | IndexState::Partial | IndexState::Running | IndexState::Paused
        )
    }

    /// 사용자가 손을 대야 하는 상태인가
    pub fn needs_action(self) -> bool {
        matches!(
            self,
            IndexState::ModelMismatch | IndexState::NeedsReindex | IndexState::Failed
        )
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocIndex {
    pub document_id: i64,
    pub collection_id: i64,
    pub title: String,
    /// 청크 수
    pub total: i64,
    /// 지금 모델·지금 글로 만들어진 벡터 수
    pub done: i64,
    /// 다른 모델로 만들어진 벡터 수
    pub other_model: i64,
    /// 글이 달라져 죽은 벡터 수
    pub stale: i64,
    pub state: IndexState,
    pub label: &'static str,
    /// 이 문서를 색인한 모델 (여러 개면 아무 하나 — 섞이는 일은 없어야 한다)
    pub indexed_with: Option<String>,
    pub error: Option<String>,
}

fn state_of(work: &str, total: i64, done: i64, other: i64, stale: i64) -> IndexState {
    match work {
        QUEUED => return IndexState::Queued,
        RUNNING => return IndexState::Running,
        PAUSED if done < total => return IndexState::Paused,
        FAILED => return IndexState::Failed,
        _ => {}
    }
    if total == 0 {
        return IndexState::NoChunks;
    }
    if done >= total {
        return IndexState::Done;
    }
    if done == 0 {
        // 쓸 수 있는 벡터가 하나도 없다. 왜 없는지에 따라 할 일이 다르다.
        if stale > 0 {
            return IndexState::NeedsReindex;
        }
        if other > 0 {
            return IndexState::ModelMismatch;
        }
        return IndexState::None;
    }
    // 쓸 수 있는 벡터가 일부 있다. 죽은 것이 섞여 있으면 그쪽을 먼저 알린다.
    if stale > 0 {
        IndexState::NeedsReindex
    } else {
        IndexState::Partial
    }
}

/// `model` 은 지금 쓰기로 한 검색 모델의 태그, `dim` 은 그 모델의 벡터 길이.
///
/// `dim` 을 함께 보는 까닭: 같은 태그로도 모델이 갈릴 수 있다(`:latest` 가
/// 가리키는 것이 바뀌는 경우). 길이가 다르면 코사인을 셈할 수조차 없다.
pub fn status(
    conn: &Connection,
    model: &str,
    dim: i64,
    collection_ids: &[i64],
) -> AppResult<Vec<DocIndex>> {
    let coll = if collection_ids.is_empty() {
        String::new()
    } else {
        let h = collection_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        format!(" AND d.collection_id IN ({h})")
    };
    let sql = format!(
        "SELECT d.id, d.collection_id, d.title, d.embed_state, d.index_error,
                (SELECT COUNT(*) FROM chunk c WHERE c.document_id = d.id),
                (SELECT COUNT(*) FROM chunk c JOIN embedding e ON e.chunk_id = c.id
                  WHERE c.document_id = d.id
                    AND e.model = ?1 AND e.dim = ?2 AND e.chunk_hash = c.hash),
                (SELECT COUNT(*) FROM chunk c JOIN embedding e ON e.chunk_id = c.id
                  WHERE c.document_id = d.id AND (e.model <> ?1 OR e.dim <> ?2)),
                (SELECT COUNT(*) FROM chunk c JOIN embedding e ON e.chunk_id = c.id
                  WHERE c.document_id = d.id
                    AND e.model = ?1 AND e.dim = ?2 AND e.chunk_hash <> c.hash),
                (SELECT e.model FROM chunk c JOIN embedding e ON e.chunk_id = c.id
                  WHERE c.document_id = d.id LIMIT 1)
           FROM document d
          WHERE d.superseded_by IS NULL{coll}
          ORDER BY d.id"
    );

    let mut params: Vec<Box<dyn ToSql>> = vec![Box::new(model.to_string()), Box::new(dim)];
    for id in collection_ids {
        params.push(Box::new(*id));
    }

    let mut st = conn.prepare(&sql)?;
    let rows = st
        .query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |r| {
                let work: String = r.get(3)?;
                let total: i64 = r.get(5)?;
                let done: i64 = r.get(6)?;
                let other: i64 = r.get(7)?;
                let stale: i64 = r.get(8)?;
                let state = state_of(&work, total, done, other, stale);
                Ok(DocIndex {
                    document_id: r.get(0)?,
                    collection_id: r.get(1)?,
                    title: r.get(2)?,
                    error: r.get(4)?,
                    total,
                    done,
                    other_model: other,
                    stale,
                    state,
                    label: state.label(),
                    indexed_with: r.get(9)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 아직 만들어야 하는 청크. 이어서 하기의 근거다.
///
/// **문서 단위가 아니라 청크 단위로 본다.** 그래서 중간에 멈추거나 앱을 껐다
/// 켜도 만들어 둔 것을 버리지 않는다.
pub fn todo(
    conn: &Connection,
    document_id: i64,
    model: &str,
    dim: i64,
) -> AppResult<Vec<(i64, String, String)>> {
    let mut st = conn.prepare(
        "SELECT c.id, c.text_norm, c.hash FROM chunk c
          WHERE c.document_id = ?1
            AND NOT EXISTS (
              SELECT 1 FROM embedding e
               WHERE e.chunk_id = c.id AND e.model = ?2 AND e.dim = ?3
                 AND e.chunk_hash = c.hash)
          ORDER BY c.ord",
    )?;
    let rows = st
        .query_map(params![document_id, model, dim], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 일의 상태를 적는다. 실패했을 때만 까닭을 함께 적는다.
pub fn set_work_state(
    conn: &Connection,
    document_id: i64,
    state: &str,
    error: Option<&str>,
) -> AppResult<()> {
    conn.execute(
        "UPDATE document SET embed_state = ?2, index_error = ?3 WHERE id = ?1",
        params![document_id, state, error],
    )?;
    Ok(())
}

/// 앱이 도는 중에 꺼졌으면 `running` 이 그대로 남는다. 켤 때 되돌린다.
///
/// 이것이 없으면 문서가 영원히 "색인 중" 으로 보이고, 사용자는 기다린다.
pub fn reset_running(conn: &Connection) -> AppResult<usize> {
    let n = conn.execute(
        "UPDATE document SET embed_state = ?1 WHERE embed_state IN (?2, ?3)",
        params![PAUSED, RUNNING, QUEUED],
    )?;
    if n > 0 {
        log::info!("지난번에 하던 색인 {n}건을 '멈춤' 으로 돌렸습니다. 이어서 할 수 있습니다.");
    }
    Ok(n)
}

/// 지금 모델로 쓸 수 없는 벡터를 지운다. 다시 색인하기 전에 부른다.
pub fn drop_unusable(conn: &Connection, document_id: i64, model: &str, dim: i64) -> AppResult<usize> {
    let n = conn.execute(
        "DELETE FROM embedding WHERE chunk_id IN (
           SELECT c.id FROM chunk c
             JOIN embedding e ON e.chunk_id = c.id
            WHERE c.document_id = ?1
              AND (e.model <> ?2 OR e.dim <> ?3 OR e.chunk_hash <> c.hash))",
        params![document_id, model, dim],
    )?;
    Ok(n)
}

/// 자료집 하나를 한 줄로 요약한다 — "문서 5개 중 3개 의미 검색 준비됨"
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionIndex {
    pub collection_id: i64,
    pub documents: i64,
    /// 의미 검색을 쓸 수 있는 문서 수
    pub ready: i64,
    /// 손을 대야 하는 문서 수 (모델 불일치·재색인·실패)
    pub needs_action: i64,
    pub summary: String,
}

pub fn collection_summary(
    conn: &Connection,
    model: &str,
    dim: i64,
    collection_id: i64,
) -> AppResult<CollectionIndex> {
    let docs = status(conn, model, dim, &[collection_id])?;
    let usable = docs.iter().filter(|d| d.state.semantic_usable()).count() as i64;
    let action = docs.iter().filter(|d| d.state.needs_action()).count() as i64;
    let total = docs.len() as i64;
    let summary = if total == 0 {
        "자료가 없습니다".to_string()
    } else if usable == total {
        format!("문서 {total}개 모두 의미 검색 준비됨")
    } else {
        format!("문서 {total}개 중 {usable}개 의미 검색 준비됨")
    };
    Ok(CollectionIndex {
        collection_id,
        documents: total,
        ready: usable,
        needs_action: action,
        summary,
    })
}

#[cfg(test)]
#[path = "embed_index_tests.rs"]
mod tests;
