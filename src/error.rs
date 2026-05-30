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

    pub fn partial_failure(message: impl Into<String>) -> Self {
        Self {
            code: ExitCode::PartialFailure,
            message: message.into(),
        }
    }

    pub fn code(&self) -> ExitCode {
        self.code
    }

    pub fn is_rate_limited(&self) -> bool {
        matches!(self.code, ExitCode::RateLimited)
    }

    pub fn exit_code(&self) -> i32 {
        self.code as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    General = 1,
    Auth = 2,
    Config = 3,
    UploadFailed = 4,
    PartialFailure = 5,
    RateLimited = 6,
    Timeout = 7,
    InvalidArguments = 8,
}
