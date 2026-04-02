use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use postpub_core::PostpubError;
use postpub_types::ErrorResponse;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
}

impl From<PostpubError> for ApiError {
    fn from(error: PostpubError) -> Self {
        let status = match &error {
            PostpubError::Validation(_) | PostpubError::InvalidPath(_) => StatusCode::BAD_REQUEST,
            PostpubError::NotFound(_) => StatusCode::NOT_FOUND,
            PostpubError::Conflict(_) => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        Self {
            status,
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(ErrorResponse::new(self.message))).into_response()
    }
}
