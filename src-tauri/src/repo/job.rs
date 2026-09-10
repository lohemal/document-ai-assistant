//! 작업 기록 (P6) — **그때 무엇을 물었고, 무엇을 근거로, 어떻게 답했는가**를 남긴다.
//!
//! 지켜야 하는 것 하나: **과거 기록은 변하지 않는다.** 원본 자료가 나중에 고쳐지거나
//! 바뀌어도, 그때의 답과 그때의 근거 원문은 그대로 보여야 한다. 그래서 근거를
//! 가리키지 않고 **복사해 둔다** — 문서 이름·쪽·원문·형광펜 자리·파일 지문(sha256)
//! 까지. 답변 전체(검증·판단 포함)도 JSON 으로 통째로 담는다.
//!
//! 다시 열 때는 그때의 문서 판과 지금의 문서를 견준다.
//!
//! | 지금 문서 | 보여 주는 것 |
//! |---|---|
//! | 같은 파일(지문 같고 새 판 없음) | 원문을 그 자리에 다시 연다 |
//! | 새 판이 등록됨(옛 판은 남아 있음) | **자료가 변경되었습니다** — 원문 보기는 당시 판을 연다 |
//! | 같은 id 인데 지문이 다름 | **자료가 변경되었습니다** — 당시 자리를 지금 원문에 잇지 않는다 |
//! | 삭제됨 | **자료가 삭제되었습니다** — 원문은 아래 복사본만 |
//!
//! 기록은 `job` 하나가 단위다. "이 작업에서 계속 질문하기" 를 나중에 붙일 자리
//! (`job_turn`)를 그대로 둔다.

use crate::error::{AppError, AppResult};
use chrono::{Duration, Local, SecondsFormat};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// 기본 보존기간(일). 설정에서 바꾼다.
pub const DEFAULT_RETENTION_DAYS: i64 = 30;
/// 고를 수 있는 값. 0 은 "직접 지울 때까지".
pub const RETENTION_CHOICES: &[i64] = &[7, 30, 90, 365, 0];

/// 지금 시각을 기록에 적는 꼴. 같은 꼴끼리는 글자 순서가 시간 순서다.
pub fn now() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

// ── 담기 ────────────────────────────────────────────────────────────

/// 당시 근거 하나 — 복사해 둘 것.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceIn {
    pub source_id: String,
    pub document_id: i64,
    pub doc_title: String,
    /// 당시 파일의 지문. 다시 열 때 지금 파일과 견준다.
    pub doc_sha256: String,
    pub collection_name: String,
    pub page_start: i64,
    pub page_end: i64,
    pub heading_path: Option<String>,
    /// 당시 원문 그대로
    pub quoted_text: String,
    pub chunk_id: Option<i64>,
    /// 당시 형광펜 자리 (JSON)
    pub spans_json: String,
    pub cited: bool,
}

pub struct JobIn {
    /// search | interpret | draft …
    pub kind: String,
    pub question: String,
    /// [{id, name}] — 빈 목록이면 '전체 자료'
    pub collections_json: String,
    pub llm_model: Option<String>,
    pub embed_model: Option<String>,
    /// hybrid | keyword
    pub search_mode: String,
    /// answer | limited | refuse | no_model | cancelled
    pub status: String,
    /// 답변 전체 (검증·판단 포함) JSON
    pub answer_json: String,
    pub evidence: Vec<EvidenceIn>,
}

pub fn save(conn: &mut Connection, j: &JobIn) -> AppResult<i64> {
    save_at(conn, j, &now())
}

/// 시각을 밖에서 받는 판 — 보존기간 시험에서 쓴다.
pub fn save_at(conn: &mut Connection, j: &JobIn, created_at: &str) -> AppResult<i64> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO job(kind, question, collections_json, llm_model, embed_model,
                         answer_json, pinned, created_at, status, search_mode)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8, ?9)",
        params![
            j.kind,
            j.question,
            j.collections_json,
            j.llm_model,
            j.embed_model,
            j.answer_json,
            created_at,
            j.status,
            j.search_mode
        ],
    )?;
    let id = tx.last_insert_rowid();
    for (i, e) in j.evidence.iter().enumerate() {
        tx.execute(
            "INSERT INTO job_evidence(job_id, ord, document_id, doc_title, doc_sha256, page,
                                      heading_path, quoted_text, chunk_id, page_end, spans_json,
                                      source_id, cited, collection_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                id,
                i as i64,
                e.document_id,
                e.doc_title,
                e.doc_sha256,
                e.page_start,
                e.heading_path,
                e.quoted_text,
                e.chunk_id,
                e.page_end,
                e.spans_json,
                e.source_id,
                e.cited as i64,
                e.collection_name
            ],
        )?;
    }
    tx.commit()?;
    Ok(id)
}

// ── 읽기 ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRow {
    pub id: i64,
    pub kind: String,
    pub question: String,
    pub status: String,
    pub pinned: bool,
    pub created_at: String,
    pub collections_json: String,
    pub llm_model: Option<String>,
    pub search_mode: String,
    pub evidence_count: i64,
}

const ROW: &str = "SELECT j.id, j.kind, j.question, j.status, j.pinned, j.created_at,
                          j.collections_json, j.llm_model, j.search_mode,
                          (SELECT COUNT(*) FROM job_evidence e WHERE e.job_id = j.id)
                     FROM job j";

fn row(r: &rusqlite::Row) -> rusqlite::Result<JobRow> {
    Ok(JobRow {
        id: r.get(0)?,
        kind: r.get(1)?,
        question: r.get(2)?,
        status: r.get(3)?,
        pinned: r.get::<_, i64>(4)? != 0,
        created_at: r.get(5)?,
        collections_json: r.get(6)?,
        llm_model: r.get(7)?,
        search_mode: r.get(8)?,
        evidence_count: r.get(9)?,
    })
}

/// 최근 것부터. `pinned_only` 면 중요 표시한 것만.
pub fn list(conn: &Connection, pinned_only: bool, limit: i64) -> AppResult<Vec<JobRow>> {
    let sql = format!(
        "{ROW} {} ORDER BY j.created_at DESC, j.id DESC LIMIT ?1",
        if pinned_only { "WHERE j.pinned = 1" } else { "" }
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map(params![limit], row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn count(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM job", [], |r| r.get(0))?)
}

/// 당시 근거 + 지금 문서와 견준 결과.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceOut {
    pub ord: i64,
    pub source_id: String,
    /// 문서가 지워졌으면 None
    pub document_id: Option<i64>,
    pub doc_title: String,
    pub doc_sha256: String,
    pub collection_name: String,
    pub page_start: i64,
    pub page_end: i64,
    pub heading_path: Option<String>,
    pub quoted_text: String,
    pub chunk_id: Option<i64>,
    pub spans_json: String,
    pub cited: bool,
    /// same | superseded | changed | deleted
    pub doc_state: String,
    /// 사람에게 보여 줄 말 (same 이면 None)
    pub note: Option<String>,
    /// 원문을 그 자리에 다시 열어도 되는가 (same·superseded)
    pub can_open: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDetail {
    pub job: JobRow,
    pub answer_json: String,
    pub evidence: Vec<EvidenceOut>,
    /// 지금 문서와 다른 근거 수
    pub changed_count: usize,
}

/// 당시 문서와 지금 문서를 견준다. **당시 자리를 지금 원문에 억지로 잇지 않는다.**
fn doc_state(conn: &Connection, document_id: Option<i64>, then_sha: &str) -> (String, Option<String>, bool) {
    let Some(id) = document_id else {
        return (
            "deleted".into(),
            Some("이 자료는 그 뒤에 삭제되었습니다. 당시 원문은 아래 복사본으로만 남아 있고, PDF 는 열 수 없습니다.".into()),
            false,
        );
    };
    let now: Option<(String, Option<i64>)> = conn
        .query_row(
            "SELECT sha256, superseded_by FROM document WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .unwrap_or(None);
    match now {
        None => (
            "deleted".into(),
            Some("이 자료는 그 뒤에 삭제되었습니다. 당시 원문은 아래 복사본으로만 남아 있고, PDF 는 열 수 없습니다.".into()),
            false,
        ),
        Some((sha, _)) if sha != then_sha => (
            "changed".into(),
            Some("자료가 변경되었습니다. 지금 등록된 파일이 당시 파일과 달라, 당시 위치를 지금 원문에 연결하지 않습니다. 아래 복사본이 당시 원문입니다.".into()),
            false,
        ),
        Some((_, Some(_))) => (
            "superseded".into(),
            Some("자료가 변경되었습니다 — 그 뒤에 새 판이 등록되었습니다. [원문 보기]는 당시 판을 엽니다. 지금 판은 자료집에서 확인해 주세요.".into()),
            true,
        ),
        Some((_, None)) => ("same".into(), None, true),
    }
}

pub fn get(conn: &Connection, id: i64) -> AppResult<JobDetail> {
    let job = conn
        .query_row(&format!("{ROW} WHERE j.id = ?1"), params![id], row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::msg("그 기록을 찾지 못했습니다. 지워졌을 수 있습니다."),
            other => AppError::from(other),
        })?;
    let answer_json: String = conn.query_row("SELECT answer_json FROM job WHERE id = ?1", params![id], |r| r.get(0))?;

    let mut st = conn.prepare(
        "SELECT ord, source_id, document_id, doc_title, doc_sha256, collection_name, page, page_end,
                heading_path, quoted_text, chunk_id, spans_json, cited
           FROM job_evidence WHERE job_id = ?1 ORDER BY ord",
    )?;
    let raw: Vec<(i64, String, Option<i64>, String, String, String, i64, i64, Option<String>, String, Option<i64>, String, i64)> = st
        .query_map(params![id], |r| {
            Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?,
                r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?, r.get(12)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut evidence = Vec::with_capacity(raw.len());
    let mut changed_count = 0;
    for (ord, source_id, document_id, doc_title, doc_sha256, collection_name, page_start, page_end, heading_path, quoted_text, chunk_id, spans_json, cited) in raw {
        let (doc_state, note, can_open) = doc_state(conn, document_id, &doc_sha256);
        if doc_state != "same" {
            changed_count += 1;
        }
        evidence.push(EvidenceOut {
            ord,
            source_id,
            document_id,
            doc_title,
            doc_sha256,
            collection_name,
            page_start,
            page_end: if page_end == 0 { page_start } else { page_end },
            heading_path,
            quoted_text,
            chunk_id,
            spans_json,
            cited: cited != 0,
            doc_state,
            note,
            can_open,
        });
    }
    Ok(JobDetail { job, answer_json, evidence, changed_count })
}

// ── 고치기 · 지우기 ─────────────────────────────────────────────────

pub fn set_pinned(conn: &Connection, id: i64, pinned: bool) -> AppResult<()> {
    let n = conn.execute("UPDATE job SET pinned = ?1 WHERE id = ?2", params![pinned as i64, id])?;
    if n == 0 {
        return Err(AppError::msg("그 기록을 찾지 못했습니다."));
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM job WHERE id = ?1", params![id])?;
    Ok(())
}

// ── 보존기간 ────────────────────────────────────────────────────────

pub fn retention_days(conn: &Connection) -> AppResult<i64> {
    let v = super::setting::get(conn, "retention_days")?;
    Ok(v.trim().parse::<i64>().unwrap_or(DEFAULT_RETENTION_DAYS))
}

pub fn set_retention_days(conn: &Connection, days: i64) -> AppResult<()> {
    if !RETENTION_CHOICES.contains(&days) {
        return Err(AppError::msg(format!(
            "보존기간은 {} 가운데 하나여야 합니다 (0 은 직접 지울 때까지).",
            RETENTION_CHOICES.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("·")
        )));
    }
    super::setting::set(conn, "retention_days", &days.to_string())
}

/// 기한이 지난 기록을 지운다. **중요 표시한 것은 남긴다.** 지운 수를 돌려준다.
///
/// `days <= 0` 이면 아무것도 지우지 않는다 (직접 지울 때까지).
/// `now` 는 `now()` 와 같은 꼴 — 시험에서 시각을 바꿔 넣으려고 밖에서 받는다.
pub fn purge_expired(conn: &Connection, days: i64, now: &str) -> AppResult<usize> {
    if days <= 0 {
        return Ok(0);
    }
    let now_t = chrono::DateTime::parse_from_rfc3339(now)
        .map_err(|e| AppError::msg(format!("시각을 읽지 못했습니다: {e}")))?;
    let cutoff = (now_t - Duration::days(days)).to_rfc3339_opts(SecondsFormat::Secs, false);
    let n = conn.execute(
        "DELETE FROM job WHERE pinned = 0 AND created_at < ?1",
        params![cutoff],
    )?;
    Ok(n)
}

#[cfg(test)]
#[path = "job_tests.rs"]
mod tests;
