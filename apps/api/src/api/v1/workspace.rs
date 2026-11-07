use std::sync::Arc;

use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json as AxumJson, Router, extract::State};
use better_auth::integrations::axum::CurrentSession;
use better_auth::BetterAuth;
use serde::{Deserialize, Serialize};

use crate::auth::AppAuthSchema;
use crate::services::avatar::{letter_avatar_url, letter_of};
use crate::services::cache::Cache;
use crate::services::workspace::{
    self as workspaces, rename_default_workspace, resolve_default_workspace, WorkspaceError,
};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(current).patch(rename))
}

/// Current workspace for the dashboard shell — the shape the top bar renders.
/// `avatarUrl` is a single letter on a transparent plate from the avatar
/// service, so the chip's own background shows through, and `initials` is the
/// same letter for when the image cannot load.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub slug: String,
    /// The caller's role in this workspace.
    pub role: String,
    pub initials: String,
    pub avatar_url: String,
}

/// `PATCH /api/v1/workspace` body.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameRequest {
    pub name: String,
}

/// Display-name cap enforced here, not upstream: better-auth's organization
/// schema puts no length limit on `name`, so the dashboard shell owns the rule
/// that keeps the top-bar chip readable.
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
        error => failure(StatusCode::INTERNAL_SERVER_ERROR, "WORKSPACE", &error.to_string()),
    }
}

/// The workspace cache and lookup are both keyed on the session's user id, so a
/// session without one is a hard failure rather than a workspace owned by
/// nobody.
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

/// Attach the derived presentation fields; both letters come from the same
/// helper, so the glyph and the remote image can never disagree.
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

/// `GET /api/v1/workspace` — session-protected, like the profile route.
///
/// The first call for an account provisions its "Default" workspace
/// (`services::workspace`); later calls read it. Responses are cached in
/// Upstash under `argus:workspace:<user_id>` for 60s, and a rename rewrites
/// that entry so the shell sees the new name immediately.
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

/// `PATCH /api/v1/workspace` — blur-save target for the General settings field.
///
/// Whitespace-only and over-long names are rejected with 400 so the field can
/// snap back to the stored value; anything else is written through the
/// organization store and answered with the same payload as the GET, which the
/// dashboard seeds straight back into its query cache.
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
            &format!(
                "The workspace name must be {MAX_WORKSPACE_NAME_CHARS} characters or fewer."
            ),
        ));
    }

    let workspace = rename_default_workspace(&auth, &user_id, name)
        .await
        .map_err(provisioning_failure)?;
    let response = view(workspace);
    cache.set(&cache_key(&user_id), &response, 60).await;
    Ok(AxumJson(response))
}
