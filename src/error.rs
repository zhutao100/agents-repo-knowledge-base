use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidArgument,
    NotFound,
    BackendMissing,
    BackendFailed,
    Internal,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::InvalidArgument => "INVALID_ARGUMENT",
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::BackendMissing => "BACKEND_MISSING",
            ErrorCode::BackendFailed => "BACKEND_FAILED",
            ErrorCode::Internal => "INTERNAL",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
pub struct ErrorDetail {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct KbError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Vec<ErrorDetail>,
}

impl KbError {
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidArgument,
            message: message.into(),
            details: Vec::new(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::NotFound,
            message: message.into(),
            details: Vec::new(),
        }
    }

    pub fn backend_missing(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::BackendMissing,
            message: message.into(),
            details: Vec::new(),
        }
    }

    pub fn backend_failed(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::BackendFailed,
            message: message.into(),
            details: Vec::new(),
        }
    }

    pub fn internal(err: impl fmt::Display, context: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Internal,
            message: context.into(),
            details: vec![ErrorDetail {
                key: "cause".to_string(),
                value: err.to_string(),
            }],
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.push(ErrorDetail {
            key: key.into(),
            value: value.into(),
        });
        self.details.sort();
        self.details.dedup();
        self
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    pub fn to_json_error(&self) -> JsonError<'_> {
        JsonError {
            error: JsonErrorBody {
                code: self.code.as_str(),
                message: &self.message,
                details: &self.details,
            },
        }
    }
}

impl fmt::Display for KbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for KbError {}

#[derive(serde::Serialize)]
pub struct JsonError<'a> {
    pub error: JsonErrorBody<'a>,
}

#[derive(serde::Serialize)]
pub struct JsonErrorBody<'a> {
    pub code: &'a str,
    pub message: &'a str,
    pub details: &'a [ErrorDetail],
}
