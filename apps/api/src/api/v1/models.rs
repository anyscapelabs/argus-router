use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json as AxumJson, Router, extract::State};
use better_auth::integrations::axum::CurrentSession;
use serde::{Deserialize, Serialize};

use crate::auth::AppAuthSchema;
use crate::services::catalog::{ModelView};
use crate::state::AppState;

/// Session-resolved user id (unauthenticated requests are rejected earlier
/// by the `CurrentSession` extractor).
fn user_id(session: &CurrentSession<AppAuthSchema>) -> String {
    session
        .user
        .id
        .typed()
        .ok()
        .cloned()
        .unwrap_or_default()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_models))
        .route("/allowlist", post(toggle_allowlist))
        .route("/refresh", post(refresh_models))
}

/// `GET /api/v1/models` — the dynamic catalog (model_catalog table), merged
/// with the signed-in owner's plan allowlist so the dashboard renders real
/// enablement, never a static list.
async fn list_models(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
) -> Result<AxumJson<serde_json::Value>, ApiProblem> {
    let user = user_id(&session);
    let plan = state.app_db.plan_of(&user).await.map_err(|error| {
        problem(StatusCode::INTERNAL_SERVER_ERROR, "PLAN_LOOKUP", &error.to_string())
    })?;
    let allowlist = state.app_db.allowlist_of(&plan).await.map_err(|error| {
        problem(StatusCode::INTERNAL_SERVER_ERROR, "ALLOWLIST_LOOKUP", &error.to_string())
    })?;
    let models: Vec<ModelView> = state
        .catalog
        .snapshot()
        .iter()
        .map(|entry| entry.view(allowlist.iter().any(|allowed| allowed == entry.id())))
        .collect();
    Ok(AxumJson(serde_json::json!({
        "plan": plan,
        "models": models,
    })))
}

#[derive(Deserialize)]
struct ToggleRequest {
    model_id: String,
    enabled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToggleResponse {
    model_id: String,
    enabled: bool,
    allowlist: Vec<String>,
}

/// `POST /api/v1/models/allowlist` — flips one model on the signed-in
/// owner's plan allowlist (dashboard models page toggle).
async fn toggle_allowlist(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
    AxumJson(request): AxumJson<ToggleRequest>,
) -> Result<AxumJson<ToggleResponse>, ApiProblem> {
    let user = user_id(&session);
    let plan = state.app_db.plan_of(&user).await.map_err(|error| {
        problem(StatusCode::INTERNAL_SERVER_ERROR, "PLAN_LOOKUP", &error.to_string())
    })?;
    let known = state.catalog.find(&request.model_id).is_some();
    if !known {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "UNKNOWN_MODEL",
            &format!("no catalog model named `{}`", request.model_id),
        ));
    }
    state
        .app_db
        .allowlist_set(&plan, &request.model_id, request.enabled)
        .await
        .map_err(|error| problem(StatusCode::INTERNAL_SERVER_ERROR, "ALLOWLIST_UPDATE", &error.to_string()))?;
    let allowlist = state.app_db.allowlist_of(&plan).await.map_err(|error| {
        problem(StatusCode::INTERNAL_SERVER_ERROR, "ALLOWLIST_LOOKUP", &error.to_string())
    })?;
    Ok(AxumJson(ToggleResponse {
        model_id: request.model_id,
        enabled: request.enabled,
        allowlist,
    }))
}

/// `POST /api/v1/models/refresh` — re-seeds the dynamic catalog from our
/// embedded catalog into the table and the live cache, keeping the
/// dashboard's models page truthful without a redeploy. OpenRouter is never
/// used as a catalog source.
async fn refresh_models(
    State(state): State<AppState>,
    _session: CurrentSession<AppAuthSchema>,
) -> Result<AxumJson<serde_json::Value>, ApiProblem> {
    let seed = crate::services::catalog::seed_entries();
    let entries = state
        .app_db
        .load_catalog(&seed)
        .await
        .map_err(|error| {
            problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CATALOG_REFRESH",
                &error.to_string(),
            )
        })?;
    state.catalog.replace(entries);
    let count = state.catalog.snapshot().len();
    Ok(AxumJson(serde_json::json!({"refreshed": true, "count": count})))
}

/// Error body rendered with the right HTTP status.
type ApiProblem = (StatusCode, AxumJson<serde_json::Value>);

fn problem(status: StatusCode, code: &str, message: &str) -> ApiProblem {
    (
        status,
        AxumJson(serde_json::json!({
            "error": {"code": code, "message": message}
        })),
    )
}
