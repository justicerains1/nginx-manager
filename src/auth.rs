use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::app_state::AppState;

pub async fn current_user_id(jar: &CookieJar, state: &AppState) -> Option<i64> {
    let token = jar.get("nm_session")?.value().to_string();
    let sessions = state.sessions.read().await;
    sessions.get(&token).copied()
}

pub async fn require_user(jar: &CookieJar, state: &AppState) -> Result<i64, Response> {
    match current_user_id(jar, state).await {
        Some(id) => Ok(id),
        None => Err(Redirect::to("/login").into_response()),
    }
}

pub fn server_error(msg: &str) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, msg.to_string()).into_response()
}
