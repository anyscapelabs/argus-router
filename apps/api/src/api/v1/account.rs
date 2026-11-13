use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json as AxumJson, Router};
use better_auth::plugins::api_key::VerifyApiKey;

use crate::state::AppState;

fn api_key_of(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
}

type ApiFailure = (StatusCode, AxumJson<serde_json::Value>);

fn failure(status: StatusCode, code: &str, message: &str) -> ApiFailure {
    (
        status,
        AxumJson(serde_json::json!({"error": {"code": code, "message": message}})),
    )
}

fn unauthorized() -> ApiFailure {
    failure(
        StatusCode::UNAUTHORIZED,
        "INVALID_KEY",
        "An error has occurred. Try again later.",
    )
}

async fn user_id_from_key(headers: &HeaderMap, state: &AppState) -> Result<String, ApiFailure> {
    let api_key = api_key_of(headers).ok_or_else(unauthorized)?;

    let view = state
        .api_keys
        .verify_api_key(
            &VerifyApiKey {
                key: api_key,
                config_id: None,
                permissions: None,
            },
            state.auth.context(),
        )
        .await
        .map_err(|_| unauthorized())?;

    let user_id = view.reference_id.typed().cloned();
    user_id.map_err(|_| {
        failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "KEY_REFERENCE",
            "The agent key lacks a user reference.",
        )
    })
}

pub fn router() -> Router<AppState> {
    Router::new().nest("/entitlements", Router::new().route("/", get(entitlements)))
}

fn db_error(_error: sqlx::Error) -> ApiFailure {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        AxumJson(serde_json::json!({
            "error": {"code": "APP_DB", "message": "An error has occurred. Try again later."},
        })),
    )
}

async fn entitlements(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: HeaderMap,
) -> Result<AxumJson<serde_json::Value>, ApiFailure> {
    let user_id = user_id_from_key(&headers, &state).await?;

    let user_record = state
        .auth
        .store()
        .get_user_by_id(&user_id)
        .await
        .map_err(|_| {
            failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "USER_LOOKUP",
                "The key's account could not be read.",
            )
        })?
        .ok_or_else(|| {
            failure(
                StatusCode::UNAUTHORIZED,
                "INVALID_KEY",
                "The agent key's account no longer exists.",
            )
        })?;

    let email_verified = user_record
        .email_verified
        .typed()
        .ok()
        .copied()
        .unwrap_or(false);
    if !email_verified {
        return Err(failure(
            StatusCode::FORBIDDEN,
            "EMAIL_NOT_VERIFIED",
            "Verify your email in the dashboard before pairing the desktop agent.",
        ));
    }

    let plan = state.app_db.plan_of(&user_id).await.map_err(db_error)?;
    let allowlist_ids = state
        .app_db
        .effective_allowlist_of(&user_id, &plan)
        .await
        .map_err(db_error)?;
    let catalog = state.catalog.snapshot();
    let allowlist: Vec<serde_json::Value> = allowlist_ids
        .iter()
        .map(|id| {
            let name = catalog
                .iter()
                .find(|entry| entry.id() == id.as_str())
                .map(|entry| entry.name.clone())
                .unwrap_or_else(|| id.clone());
            serde_json::json!({
                "id": id,
                "display_name": name,
                "name": name,
            })
        })
        .collect();

    Ok(AxumJson(serde_json::json!({
        "user": {
            "id": user_id,
            "name": user_record.name.typed().ok().cloned().unwrap_or(None),
            "email": user_record.email.typed().ok().cloned().unwrap_or(None),
            "image": user_record.image.typed().ok().cloned().unwrap_or(None),
        },
        "plan": {
            "id": plan,
        },
        "allowlist": allowlist,
    })))
}
