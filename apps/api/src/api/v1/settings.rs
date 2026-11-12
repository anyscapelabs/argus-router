use axum::http::StatusCode;
use axum::routing::{get, patch};
use axum::{Json as AxumJson, Router, extract::State};
use better_auth::integrations::axum::CurrentSession;
use serde::Deserialize;

use crate::auth::AppAuthSchema;
use crate::state::AppState;

fn user_id(session: &CurrentSession<AppAuthSchema>) -> String {
    session.user.id.typed().ok().cloned().unwrap_or_default()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_settings))
        .route("/", patch(update_settings))
}

async fn get_settings(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
) -> Result<AxumJson<crate::services::app_db::UserSettings>, ApiProblem> {
    let user = user_id(&session);
    let settings = state.app_db.settings_of(&user).await.map_err(|error| {
        problem(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SETTINGS_LOOKUP",
            &error.to_string(),
        )
    })?;
    Ok(AxumJson(settings))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsPatch {
    allow_free_training: Option<bool>,
    max_key_lifetime_days: Option<i64>,
}

const ALLOWED_LIFETIMES: [i64; 7] = [0, 7, 30, 60, 90, 180, 365];

async fn update_settings(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
    AxumJson(patch): AxumJson<SettingsPatch>,
) -> Result<AxumJson<crate::services::app_db::UserSettings>, ApiProblem> {
    let user = user_id(&session);
    if let Some(days) = patch.max_key_lifetime_days {
        if !ALLOWED_LIFETIMES.contains(&days) {
            return Err(problem(
                StatusCode::BAD_REQUEST,
                "INVALID_LIFETIME",
                &format!(
                    "`maxKeyLifetimeDays` must be one of {:?} (0 = no limit)",
                    ALLOWED_LIFETIMES
                ),
            ));
        }
    }
    if let Some(allow) = patch.allow_free_training {
        state
            .app_db
            .set_allow_free_training(&user, allow)
            .await
            .map_err(|error| {
                problem(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "SETTINGS_UPDATE",
                    &error.to_string(),
                )
            })?;
    }
    if let Some(days) = patch.max_key_lifetime_days {
        state
            .app_db
            .set_max_key_lifetime(&user, days)
            .await
            .map_err(|error| {
                problem(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "SETTINGS_UPDATE",
                    &error.to_string(),
                )
            })?;
        state
            .app_db
            .clamp_key_lifetimes(&user, days)
            .await
            .map_err(|error| {
                problem(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "SETTINGS_UPDATE",
                    &error.to_string(),
                )
            })?;
    }
    let settings = state.app_db.settings_of(&user).await.map_err(|error| {
        problem(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SETTINGS_LOOKUP",
            &error.to_string(),
        )
    })?;
    Ok(AxumJson(settings))
}

type ApiProblem = (StatusCode, AxumJson<serde_json::Value>);

fn problem(status: StatusCode, code: &str, message: &str) -> ApiProblem {
    (
        status,
        AxumJson(serde_json::json!({
            "error": {"code": code, "message": message}
        })),
    )
}
