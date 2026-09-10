use crate::db::Db;
use crate::error::{AppError, AppResult};
use std::path::PathBuf;

/// 앱이 들고 다니는 것.
///
/// 자료를 열지 못했더라도 앱은 뜬다. 창이 아예 안 뜨면 사용자는 무엇이
/// 잘못됐는지 알 길이 없다 — 화면에 한글로 이유를 보여 주는 편이 낫다.
pub struct AppState {
    pub data_dir: PathBuf,
    db: Option<Db>,
    open_error: Option<String>,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> Self {
        match crate::db::open(&data_dir) {
            Ok(db) => {
                // 지난번에 색인을 하다 앱이 꺼졌으면 '도는 중' 이 그대로 남아
                // 있다. 그대로 두면 문서가 영원히 "색인 중" 으로 보이고
                // 사용자는 기다린다. 켤 때 '멈춤' 으로 돌린다.
                if let Err(e) = db.with(|c| crate::repo::embed_index::reset_running(c)) {
                    log::warn!("색인 상태를 되돌리지 못했습니다: {e}");
                }
                // 등록하다 꺼진 문서도 같다 — '등록 중' 이 영원히 남지 않게 '등록 중단' 으로
                match db.with(|c| crate::repo::document::reset_aborted(c)) {
                    Ok(0) => {}
                    Ok(n) => log::warn!("등록이 끝나지 않은 문서 {n}개를 '등록 중단' 으로 표시했습니다."),
                    Err(e) => log::warn!("등록 상태를 되돌리지 못했습니다: {e}"),
                }
                Self {
                    data_dir,
                    db: Some(db),
                    open_error: None,
                }
            }
            Err(e) => {
                log::error!("자료를 열지 못했습니다: {e}");
                Self {
                    data_dir,
                    db: None,
                    open_error: Some(e.to_string()),
                }
            }
        }
    }

    pub fn db(&self) -> AppResult<&Db> {
        match (&self.db, &self.open_error) {
            (Some(db), _) => Ok(db),
            (None, Some(msg)) => Err(AppError::msg(msg.clone())),
            (None, None) => Err(AppError::msg("자료가 아직 준비되지 않았습니다.")),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.db.is_some()
    }

    pub fn open_error(&self) -> Option<&str> {
        self.open_error.as_deref()
    }
}
