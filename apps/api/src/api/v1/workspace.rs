use std::sync::Arc;

use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json as AxumJson, Router, extract::State};
use better_auth::BetterAuth;
use better_auth::integrations::axum::CurrentSession;
use serde::{Deserialize, Serialize};

use crate::auth::AppAuthSchema;
use crate::services::avatar::{letter_avatar_url, letter_of};
use crate::services::cache::Cache;
use crate::services::workspace::{
    self as workspaces, WorkspaceError, rename_default_workspace, resolve_default_workspace,
};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(current).patch(rename))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub role: String,
    pub initials: String,
    pub avatar_url: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameRequest {
    pub name: String,
}

const MAX_WORKSPACE_NAME_CHARS: usize = 100;

type ApiFailure = (StatusCode, AxumJson<serde_json::Value>);

fn failure(status: StatusCode, code: &str, message: &str) -> ApiFailure {
    (
        status,
        AxumJson(serde_json::json!({"error": {"code": code, "message": message}})),
    )
}

fn provisioning_failure(error: WorkspaceError) -> ApiFailure {
    match error {
        WorkspaceError::Missing => failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "WORKSPACE_MISSING",
            "The default workspace could not be created for this account.",
        ),
        WorkspaceError::Preclaimed => failure(
            StatusCode::CONFLICT,
            "WORKSPACE_PRECLAIMED",
            "Your default workspace slug is held by another account. Contact support to resolve it.",
        ),
        _error => failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "WORKSPACE",
            "An error has occurred. Try again later.",
        ),
    }
}

fn user_id_of(session: &CurrentSession<AppAuthSchema>) -> Result<String, ApiFailure> {
    session
        .user
        .id
        .typed()
        .ok()
        .cloned()
        .filter(|user_id| !user_id.is_empty())
        .ok_or_else(|| {
            failure(
                StatusCode::UNAUTHORIZED,
                "SESSION",
                "The session carries no user identifier.",
            )
        })
}

fn cache_key(user_id: &str) -> String {
    format!("argus:workspace:{user_id}")
}

fn view(workspace: workspaces::Workspace) -> Workspace {
    Workspace {
        initials: letter_of(&workspace.name),
        avatar_url: letter_avatar_url(&workspace.name),
        id: workspace.id,
        name: workspace.name,
        slug: workspace.slug,
        role: workspace.role,
    }
}

async fn current(
    State(auth): State<Arc<BetterAuth<AppAuthSchema>>>,
    State(cache): State<Cache>,
    session: CurrentSession<AppAuthSchema>,
) -> Result<AxumJson<Workspace>, ApiFailure> {
    let user_id = user_id_of(&session)?;

    let key = cache_key(&user_id);
    if let Some(cached) = cache.get::<Workspace>(&key).await {
        return Ok(AxumJson(cached));
    }

    let workspace = resolve_default_workspace(&auth, &user_id)
        .await
        .map_err(provisioning_failure)?;
    let response = view(workspace);
    cache.set(&key, &response, 60).await;
    Ok(AxumJson(response))
}

async fn rename(
    State(auth): State<Arc<BetterAuth<AppAuthSchema>>>,
    State(cache): State<Cache>,
    session: CurrentSession<AppAuthSchema>,
    AxumJson(request): AxumJson<RenameRequest>,
) -> Result<AxumJson<Workspace>, ApiFailure> {
    let user_id = user_id_of(&session)?;

    let name = request.name.trim();
    if name.is_empty() {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            "WORKSPACE_NAME",
            "The workspace name cannot be empty.",
        ));
    }
    if name.chars().count() > MAX_WORKSPACE_NAME_CHARS {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            "WORKSPACE_NAME",
            &format!("The workspace name must be {MAX_WORKSPACE_NAME_CHARS} characters or fewer."),
        ));
    }

    let workspace = rename_default_workspace(&auth, &user_id, name)
        .await
        .map_err(provisioning_failure)?;
    let response = view(workspace);
    cache.set(&cache_key(&user_id), &response, 60).await;
    Ok(AxumJson(response))
}
