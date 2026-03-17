use axum::{Json, extract::State};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::state::GlobalState;

#[derive(Deserialize)]
pub struct RemoteLogin {
    id: String,
    password: String,
}

#[derive(Serialize)]
pub struct RemoteLoginResult {
    id: String,
    token: Option<String>,
    error: bool,
    error_message: Option<String>,
}

pub async fn login(
    State(state): State<GlobalState>,
    Json(payload): Json<RemoteLogin>,
) -> (StatusCode, Json<RemoteLoginResult>) {
    let client = reqwest::Client::new();

    let res = client
        .post(format!(
            "https://auth.{}/Cert/User/Login/login.jsp",
            state.remote_api_domain
        ))
        .form(&[
            (
                "nextURL",
                format!(
                    "https://intra.{}/SWupis/V005/loginReturn.jsp",
                    state.remote_api_domain
                ),
            ),
            ("site", "SWUPIS".to_string()),
            ("userid", payload.id.clone()),
            ("passwd", payload.password),
        ])
        .send()
        .await;

    if res.is_err() {
        let error = res.unwrap_err();
        eprintln!("Failed to login: {}", error.status().unwrap_or_default());

        let result = RemoteLoginResult {
            error: true,
            error_message: Some("failed".to_string()),
            id: payload.id,
            token: None,
        };

        return (StatusCode::UNAUTHORIZED, Json(result));
    }

    res.unwrap()
        .cookies()
        .for_each(|x| println!("{}:{}", x.name(), x.value()));

    let result = RemoteLoginResult {
        error: false,
        error_message: None,
        id: payload.id,
        token: Some("".to_string()),
    };

    (StatusCode::OK, Json(result))
}
