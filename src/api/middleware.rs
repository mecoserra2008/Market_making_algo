// Middleware for authentication, authorization, rate limiting, etc.
// TODO: Implement JWT authentication middleware
// TODO: Implement rate limiting middleware
// TODO: Implement proper CORS configuration for production

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

/// Error response for middleware failures
pub async fn handle_error() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "error": "Unauthorized",
            "details": "Authentication required"
        })),
    )
        .into_response()
}
