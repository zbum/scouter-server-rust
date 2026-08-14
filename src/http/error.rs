use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: u16,
}

pub enum HttpError {
    BadRequest(String),
    InternalError(String),
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            HttpError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            HttpError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        let body = Json(ErrorResponse {
            error: message,
            code: status.as_u16(),
        });
        (status, body).into_response()
    }
}
