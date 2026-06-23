use axum::{Json, extract::State};
use encoding_rs::{EUC_KR, UTF_8};
use mongodb::bson::{DateTime, doc, oid::ObjectId};
use reqwest::StatusCode;
use reqwest::cookie::{CookieStore, Jar};
use reqwest::header::{CONTENT_TYPE, HeaderValue};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{api::remote_client_builder, state::GlobalState};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<&'static str>,
}

pub async fn login(
    State(state): State<GlobalState>,
    Json(payload): Json<RemoteLogin>,
) -> (StatusCode, Json<RemoteLoginResult>) {
    let user_id = payload.id;
    let password = payload.password;
    if user_id.trim().is_empty() || password.trim().is_empty() {
        return login_error(
            &user_id,
            StatusCode::BAD_REQUEST,
            "missing_credentials",
            "Missing id or password",
        );
    }

    let cookie_store = Arc::new(Jar::default());
    let client = match remote_client_builder()
        .cookie_provider(Arc::clone(&cookie_store))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            eprintln!("Failed to build HTTP client: {error}");

            let result = RemoteLoginResult {
                error: true,
                error_message: Some("Failed to build remote login client".to_string()),
                error_code: Some("internal_error"),
                id: user_id,
                token: None,
            };

            return (StatusCode::INTERNAL_SERVER_ERROR, Json(result));
        }
    };

    let intra_cookie_url = match reqwest::Url::parse(&format!(
        "https://intra.{}/SWupis/V005/",
        state.remote_api_domain
    )) {
        Ok(url) => url,
        Err(error) => {
            eprintln!("Failed to parse intra URL: {error}");

            let result = RemoteLoginResult {
                error: true,
                error_message: Some("Invalid remote login URL".to_string()),
                error_code: Some("internal_error"),
                id: user_id,
                token: None,
            };

            return (StatusCode::INTERNAL_SERVER_ERROR, Json(result));
        }
    };

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
            ("userid", user_id.clone()),
            ("passwd", password.clone()),
        ])
        .send()
        .await;

    if res.is_err() {
        let error = res.unwrap_err();
        eprintln!("Failed to login: {error}");

        return login_error(
            &user_id,
            StatusCode::BAD_GATEWAY,
            "remote_unavailable",
            "Failed to reach remote login server",
        );
    }

    let res = res.unwrap();
    if !res.status().is_success() {
        eprintln!("Failed to login: {}", res.status());

        return login_error(
            &user_id,
            StatusCode::BAD_GATEWAY,
            "remote_unavailable",
            "Remote login server returned an error",
        );
    }

    let login_html = match decode_response_body(res).await {
        Ok(html) => html,
        Err(error) => {
            eprintln!("Failed to read remote login response: {error}");

            return login_error(
                &user_id,
                StatusCode::BAD_GATEWAY,
                "remote_unavailable",
                "Failed to read remote login response",
            );
        }
    };

    if let Some(message) = remote_login_failure_message(&login_html) {
        return login_error(
            &user_id,
            StatusCode::UNAUTHORIZED,
            "remote_login_rejected",
            &message,
        );
    }

    for url in [
        format!(
            "https://intra.{}/SWupis/V005/loginReturn.jsp",
            state.remote_api_domain
        ),
        format!(
            "https://intra.{}/SWupis/V005/index.jsp",
            state.remote_api_domain
        ),
    ] {
        let res = match client.get(url).send().await {
            Ok(res) if res.status().is_success() => res,
            Ok(res) => {
                eprintln!("Failed to create remote session: {}", res.status());

                return login_error(
                    &user_id,
                    StatusCode::BAD_GATEWAY,
                    "remote_session_failed",
                    "Remote session creation failed",
                );
            }
            Err(error) => {
                eprintln!("Failed to create remote session: {error}");

                return login_error(
                    &user_id,
                    StatusCode::BAD_GATEWAY,
                    "remote_session_failed",
                    "Failed to reach remote session server",
                );
            }
        };

        let session_html = match decode_response_body(res).await {
            Ok(html) => html,
            Err(error) => {
                eprintln!("Failed to read remote session response: {error}");

                return login_error(
                    &user_id,
                    StatusCode::BAD_GATEWAY,
                    "remote_session_failed",
                    "Failed to read remote session response",
                );
            }
        };

        if let Some(message) = remote_login_failure_message(&session_html) {
            return login_error(
                &user_id,
                StatusCode::UNAUTHORIZED,
                "remote_login_rejected",
                &message,
            );
        };
    }

    let login_session_cookie = match cookie_store
        .cookies(&intra_cookie_url)
        .and_then(|value| value.to_str().ok().map(str::to_string))
    {
        Some(cookie) if !cookie.is_empty() => cookie,
        _ => {
            return login_error(
                &user_id,
                StatusCode::UNAUTHORIZED,
                "remote_session_failed",
                "Remote login did not create a usable session",
            );
        }
    };

    let token = ObjectId::new().to_hex();
    let sessions = state
        .db_client
        .collection::<mongodb::bson::Document>("sessions");

    if let Err(error) = sessions
        .update_one(
            doc! { "user_id": user_id.clone() },
            doc! {
                "$set": {
                    "user_id": user_id.clone(),
                    "password": password,
                    "token": token.clone(),
                    "login_session_cookie": login_session_cookie,
                    "updated_at": DateTime::now(),
                },
                "$setOnInsert": {
                    "created_at": DateTime::now(),
                },
            },
        )
        .upsert(true)
        .await
    {
        eprintln!("Failed to save login session: {error}");

        let result = RemoteLoginResult {
            error: true,
            error_message: Some("Failed to save login session".to_string()),
            error_code: Some("database_error"),
            id: user_id,
            token: None,
        };

        return (StatusCode::INTERNAL_SERVER_ERROR, Json(result));
    }

    let result = RemoteLoginResult {
        error: false,
        error_message: None,
        error_code: None,
        id: user_id,
        token: Some(token),
    };

    (StatusCode::OK, Json(result))
}

fn login_error(
    user_id: &str,
    status: StatusCode,
    code: &'static str,
    message: &str,
) -> (StatusCode, Json<RemoteLoginResult>) {
    (
        status,
        Json(RemoteLoginResult {
            id: user_id.to_string(),
            token: None,
            error: true,
            error_message: Some(truncate_message(message, 240)),
            error_code: Some(code),
        }),
    )
}

async fn decode_response_body(response: reqwest::Response) -> Result<String, reqwest::Error> {
    let content_type = response.headers().get(CONTENT_TYPE).cloned();
    let bytes = response.bytes().await?;

    Ok(decode_html(&bytes, content_type.as_ref()))
}

fn remote_login_failure_message(html: &str) -> Option<String> {
    for message in extract_alert_messages(html) {
        if is_login_rejection_message(&message) {
            return Some(message);
        }
    }

    let text = visible_text(html);
    is_login_rejection_message(&text).then(|| truncate_message(&text, 240))
}

fn is_login_rejection_message(message: &str) -> bool {
    let message = clean_text(message);
    let lower = message.to_ascii_lowercase();
    let contains_any = |needles: &[&str]| needles.iter().any(|needle| message.contains(needle));
    let lower_contains_any = |needles: &[&str]| needles.iter().any(|needle| lower.contains(needle));

    if contains_any(&["로그인정보를 찾을 수 없습니다", "다시 로그인해주세요"])
    {
        return true;
    }
    if contains_any(&["아이디", "사용자", "학번"]) && contains_any(&["비밀번호", "패스워드"])
    {
        return true;
    }
    if contains_any(&["비밀번호", "패스워드"]) && contains_any(&["일치", "틀", "확인", "오류"])
    {
        return true;
    }
    if contains_any(&["로그인", "인증"]) && contains_any(&["실패", "오류", "정보", "불가"])
    {
        return true;
    }
    if contains_any(&["계정", "사용자"]) && contains_any(&["잠", "중지", "정지", "제한"])
    {
        return true;
    }

    lower_contains_any(&["invalid", "incorrect", "login failed", "locked"])
        || (lower.contains("password")
            && lower_contains_any(&["wrong", "failed", "failure", "error", "check"]))
}

fn decode_html(bytes: &[u8], content_type: Option<&HeaderValue>) -> String {
    let content_type = content_type
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let charset_source = format!("{content_type}\n{prefix}").to_ascii_lowercase();
    let encoding = if charset_source.contains("utf-8") || charset_source.contains("utf8") {
        UTF_8
    } else {
        EUC_KR
    };
    let (text, _, _) = encoding.decode(bytes);

    text.into_owned()
}

fn extract_alert_messages(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut cursor = 0;
    let mut messages = Vec::new();

    while let Some(found) = lower[cursor..].find("alert") {
        let alert_index = cursor + found;
        let Some(open_offset) = lower[alert_index..].find('(') else {
            break;
        };
        let mut index = alert_index + open_offset + 1;
        while html[index..].starts_with(char::is_whitespace) {
            index += html[index..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
        }

        let Some(quote) = html[index..].chars().next() else {
            break;
        };
        if quote != '"' && quote != '\'' {
            cursor = index.saturating_add(quote.len_utf8());
            continue;
        }

        index += quote.len_utf8();
        let mut escaped = false;
        let mut message = String::new();
        let mut end_index = index;
        for character in html[index..].chars() {
            end_index += character.len_utf8();
            if escaped {
                message.push(match character {
                    'n' | 'r' | 't' => ' ',
                    other => other,
                });
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                continue;
            }
            if character == quote {
                break;
            }
            message.push(character);
        }

        let message = clean_text(&message);
        if !message.is_empty() && !messages.contains(&message) {
            messages.push(message);
        }
        cursor = end_index;
    }

    messages
}

fn visible_text(html: &str) -> String {
    let without_scripts = strip_tag_blocks(html, "script");
    let without_styles = strip_tag_blocks(&without_scripts, "style");
    let mut text = String::new();
    let mut in_tag = false;

    for character in without_styles.chars() {
        match character {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }

    clean_text(&text)
}

fn strip_tag_blocks(html: &str, tag: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut cursor = 0;
    let mut output = String::new();

    while let Some(open_offset) = lower[cursor..].find(&open) {
        let open_index = cursor + open_offset;
        output.push_str(&html[cursor..open_index]);
        if let Some(close_offset) = lower[open_index..].find(&close) {
            cursor = open_index + close_offset + close.len();
        } else {
            cursor = html.len();
            break;
        }
    }
    output.push_str(&html[cursor..]);

    output
}

fn clean_text(value: &str) -> String {
    value
        .replace('\u{00a0}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_message(message: &str, max_chars: usize) -> String {
    let message = clean_text(message);
    let mut truncated = message.chars().take(max_chars).collect::<String>();
    if message.chars().count() > max_chars {
        truncated.push_str("...");
    }

    truncated
}

#[cfg(test)]
mod tests {
    use super::{extract_alert_messages, remote_login_failure_message};

    #[test]
    fn extracts_alert_messages_with_escaped_newlines() {
        let html = r#"<script>alert("아이디 또는\n비밀번호가 일치하지 않습니다.");</script>"#;

        assert_eq!(
            extract_alert_messages(html),
            vec!["아이디 또는 비밀번호가 일치하지 않습니다."]
        );
    }

    #[test]
    fn detects_alert_based_login_failure() {
        let html = r#"<html><script>alert("아이디 또는 비밀번호를 확인하세요.");</script></html>"#;

        assert_eq!(
            remote_login_failure_message(html).as_deref(),
            Some("아이디 또는 비밀번호를 확인하세요.")
        );
    }

    #[test]
    fn detects_session_missing_body() {
        let html = "<html><body>로그인정보를 찾을 수 없습니다S.. 다시 로그인해주세요</body></html>";

        assert!(remote_login_failure_message(html).is_some());
    }

    #[test]
    fn ignores_success_page_without_failure_text() {
        let html = "<html><body>웹정보서비스 메인 페이지</body></html>";

        assert!(remote_login_failure_message(html).is_none());
    }

    #[test]
    fn does_not_treat_password_word_alone_as_failure() {
        let html = "<html><body>Password change notice</body></html>";

        assert!(remote_login_failure_message(html).is_none());
    }
}
