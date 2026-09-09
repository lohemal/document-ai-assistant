use serde::Serialize;

/// 화면으로 넘어가는 오류.
///
/// 사용자에게 그대로 보여도 되는 한글 문장을 담는다. 기술적인 내용은
/// 로그로만 남기고, 화면에는 무엇을 하면 되는지를 적는다.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),

    #[error("자료를 저장하는 중 문제가 생겼습니다. ({0})")]
    Db(#[from] rusqlite::Error),

    #[error("파일을 다루는 중 문제가 생겼습니다. ({0})")]
    Io(#[from] std::io::Error),
}

impl AppError {
    pub fn msg(text: impl Into<String>) -> Self {
        AppError::Message(text.into())
    }
}

// Tauri 명령은 오류를 직렬화할 수 있어야 한다.
impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
