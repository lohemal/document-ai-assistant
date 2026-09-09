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
            Ok(db) => Self {
                data_dir,
                db: Some(db),
                open_error: None,
            },
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
