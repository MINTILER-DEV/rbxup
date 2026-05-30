use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
#[error("{message}")]
pub struct AppError {
    code: ExitCode,
    message: String,
}

impl AppError {
    pub fn general(message: impl Into<String>) -> Self {
        Self {
            code: ExitCode::General,
            message: message.into(),
        }
    }

    pub fn auth(message: impl Into<String>) -> Self {
        Self {
            code: ExitCode::Auth,
            message: message.into(),
        }
    }

    pub fn config(message: impl Into<String>) -> Self {
        Self {
            code: ExitCode::Config,
            message: message.into(),
        }
    }

    pub fn upload(message: impl Into<String>) -> Self {
        Self {
            code: ExitCode::UploadFailed,
            message: message.into(),
        }
    }

    pub fn invalid_args(message: impl Into<String>) -> Self {
        Self {
            code: ExitCode::InvalidArguments,
            message: message.into(),
        }
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self {
            code: ExitCode::RateLimited,
            message: message.into(),
        }
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self {
            code: ExitCode::Timeout,
            message: message.into(),
        }
    }

    pub fn exit_code(&self) -> i32 {
        self.code as i32
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum ExitCode {
    General = 1,
    Auth = 2,
    Config = 3,
    UploadFailed = 4,
    RateLimited = 6,
    Timeout = 7,
    InvalidArguments = 8,
}
