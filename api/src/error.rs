use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// API errors. The messages are in French: they surface verbatim in the
/// interface.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("authentification requise")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("erreur interne")]
    Internal(#[from] anyhow::Error),
    #[error("erreur base de donnees")]
    Db(#[from] sqlx::Error),
}

impl AppError {
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }

    /// Detailed message, cause included. `Display` stays deliberately quiet so
    /// nothing leaks into an HTTP response; but a failure recorded for an admin
    /// (a visual that never came out, say) must say what actually happened.
    pub fn detail(&self) -> String {
        match self {
            Self::Internal(e) => format!("{e:#}"),
            Self::Db(e) => format!("base de donnees : {e}"),
            other => other.to_string(),
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Internal(_) | Self::Db(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        // A 404 on another collective's resource is deliberate: we do not
        // reveal that it exists (§15, isolation).
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = ?self, "internal error");
        }
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;
