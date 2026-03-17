use axum::Json;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    pub(crate) error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error_message: Option<String>,
}

pub fn internal_error<'a, E>(error: E) -> (StatusCode, Json<ErrorResponse>)
where
    E: std::error::Error,
{
    println!("Internal error: {}", error);
    let message = ErrorResponse {
        error: true,
        error_message: None,
    };

    (StatusCode::INTERNAL_SERVER_ERROR, Json(message))
}
