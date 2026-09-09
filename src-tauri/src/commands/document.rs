use crate::db::files_dir;
use crate::error::{AppError, AppResult};
use crate::repo::document as repo;
use crate::state::AppState;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::State;

/// 등록해 둔 원본 PDF 사본이 있는 곳.
///
/// 원본을 그 자리에서 읽지 않고 **베껴 두는** 까닭은, 사용자가 원본을 옮기거나
/// 지워도 근거의 원문 보기가 살아 있어야 하기 때문이다 (설계안 5-1).
fn pdf_path(state: &AppState, document_id: i64) -> PathBuf {
    files_dir(&state.data_dir).join(format!("{document_id}.pdf"))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterResult {
    /// 이미 같은 내용이 있으면 여기에 그 자료가 담기고 새로 만들지 않는다
    pub duplicate_of: Option<repo::Document>,
    /// 새로 만든 자료 (중복이면 None)
    pub document: Option<repo::Document>,
    /// 이름이 같은 옛 자료. 등록을 마치면 이것을 앞선 판으로 잇는다
    pub previous_id: Option<i64>,
}

#[tauri::command]
pub fn document_register(
    state: State<'_, AppState>,
    collection_id: i64,
    source_path: String,
) -> AppResult<RegisterResult> {
    let src = Path::new(&source_path);
    if !src.exists() {
        return Err(AppError::msg("그 파일을 찾지 못했습니다."));
    }
    let filename = src
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::msg("파일 이름을 읽지 못했습니다."))?;
    if !filename.to_lowercase().ends_with(".pdf") {
        return Err(AppError::msg(
            "지금은 PDF 만 등록할 수 있습니다. 한글 파일은 한글에서 PDF 로 저장한 뒤 등록해 주세요.",
        ));
    }

    let bytes = std::fs::read(src)?;
    if bytes.is_empty() {
        return Err(AppError::msg("파일이 비어 있습니다."));
    }
    let sha256 = format!("{:x}", Sha256::digest(&bytes));

    let db = state.db()?;

    // 내용이 똑같은 파일이 이미 있으면 새로 만들지 않는다
    if let Some(same) = db.with(|c| repo::find_same(c, collection_id, &sha256))? {
        return Ok(RegisterResult {
            duplicate_of: Some(same),
            document: None,
            previous_id: None,
        });
    }

    // 이름은 같은데 내용이 다르면 개정본이다. 옛것은 지우지 않는다.
    let previous_id = db
        .with(|c| repo::find_previous(c, collection_id, &filename))?
        .map(|d| d.id);

    let title = filename.trim_end_matches(".pdf").trim_end_matches(".PDF");
    let id = db.with(|c| {
        repo::begin(
            c,
            collection_id,
            title,
            &filename,
            &sha256,
            bytes.len() as i64,
            Some(&source_path),
        )
    })?;

    // 자리를 잡은 뒤에 파일을 벤다. 반대로 하면 이름 붙일 번호가 없다.
    let dest = pdf_path(&state, id);
    if let Err(e) = std::fs::write(&dest, &bytes) {
        // 파일을 못 옮겼으면 자리도 도로 물린다
        let _ = db.with(|c| repo::delete(c, id));
        return Err(AppError::msg(format!(
            "자료를 앱 폴더로 옮기지 못했습니다. ({e})"
        )));
    }

    Ok(RegisterResult {
        duplicate_of: None,
        document: Some(db.with(|c| repo::get(c, id))?),
        previous_id,
    })
}

/// 등록해 둔 PDF 를 그대로 돌려준다. 화면의 pdf.js 가 읽는다.
///
/// JSON 이 아니라 날바이트로 보낸다. 수십 MB 짜리 PDF 를 숫자 배열로 바꾸면
/// 몇 배로 부풀고 느리다.
#[tauri::command]
pub fn document_bytes(
    state: State<'_, AppState>,
    document_id: i64,
) -> AppResult<tauri::ipc::Response> {
    let path = pdf_path(&state, document_id);
    let bytes = std::fs::read(&path).map_err(|_| {
        AppError::msg("등록해 둔 PDF 파일을 찾지 못했습니다. 자료를 다시 등록해 주세요.")
    })?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub fn document_save_pages(
    state: State<'_, AppState>,
    document_id: i64,
    pages: Vec<repo::PageIn>,
) -> AppResult<()> {
    state.db()?.with_mut(|c| repo::save_pages(c, document_id, &pages))
}

#[tauri::command]
pub fn document_finish(
    state: State<'_, AppState>,
    document_id: i64,
    page_count: i64,
    status: String,
    extractor: String,
    previous_id: Option<i64>,
) -> AppResult<repo::Document> {
    let db = state.db()?;
    db.with(|c| repo::finish(c, document_id, page_count, &status, &extractor))?;
    if let Some(old) = previous_id {
        db.with(|c| repo::supersede(c, old, document_id))?;
    }
    db.with(|c| repo::get(c, document_id))
}

#[tauri::command]
pub fn document_list(
    state: State<'_, AppState>,
    collection_id: i64,
) -> AppResult<Vec<repo::Document>> {
    state.db()?.with(|c| repo::list(c, collection_id))
}

#[tauri::command]
pub fn document_get(state: State<'_, AppState>, document_id: i64) -> AppResult<repo::Document> {
    state.db()?.with(|c| repo::get(c, document_id))
}

#[tauri::command]
pub fn document_pages(
    state: State<'_, AppState>,
    document_id: i64,
) -> AppResult<Vec<repo::PageOut>> {
    state.db()?.with(|c| repo::pages(c, document_id))
}

#[tauri::command]
pub fn document_page(
    state: State<'_, AppState>,
    document_id: i64,
    page: i64,
) -> AppResult<repo::PageOut> {
    state.db()?.with(|c| repo::page(c, document_id, page))
}

#[tauri::command]
pub fn document_delete(state: State<'_, AppState>, document_id: i64) -> AppResult<()> {
    state.db()?.with(|c| repo::delete(c, document_id))?;
    let path = pdf_path(&state, document_id);
    if path.exists() {
        if let Err(e) = std::fs::remove_file(&path) {
            log::warn!("파일을 지우지 못했습니다 {}: {e}", path.display());
        }
    }
    Ok(())
}
