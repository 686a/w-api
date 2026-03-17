use axum::{Json, extract::State};
use mongodb::bson::doc;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    errors::{ErrorResponse, internal_error},
    state::GlobalState,
};

#[derive(Deserialize)]
pub struct Login {
    token: String,
}

#[derive(Serialize)]
pub struct LoginResult {
    error: bool,
    error_message: Option<String>,
}

pub async fn login(
    State(state): State<GlobalState>,
    Json(payload): Json<Login>,
) -> Result<Json<LoginResult>, (StatusCode, Json<ErrorResponse>)> {
    let db = state
        .db_client
        .collection::<mongodb::bson::Document>("sessions");

    let query = db
        .find_one(doc! { "token": payload.token })
        .await
        .map_err(internal_error)?;

    if query.is_none() {
        let result = ErrorResponse {
            error: true,
            error_message: Some("Failed to login".to_string()),
        };

        return Err((StatusCode::UNAUTHORIZED, Json(result)));
    }

    let result = LoginResult {
        error: false,
        error_message: None,
    };

    Ok(Json(result))
}
