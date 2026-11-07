//! Workspaces are better-auth organizations (`OrganizationPlugin`), and every
//! account gets exactly one named "Default" that it owns.
//!
//! Provisioning is lazy: the row appears the first time a live session asks for
//! its workspace. Nothing here runs at boot, so the database never gains
//! placeholder accounts — an organization and its owner membership are written
//! only for a user who actually signed in.
//!
//! The slug is derived from the user id because organization slugs are globally
//! unique (`organization_slug_key`, created in `auth::run_migrations`) and a
//! plain `default` would collide across accounts.
//!
//! Provisioning goes through the store rather than the plugin's
//! `/organization/create` route: it is keyed on the session's user id, needs no
//! caller-supplied slug derivation, and does not depend on a request context.
//! The trade-off is that the organization lifecycle hooks and the session's
//! `activeOrganizationId` are not touched — there is one workspace per account,
//! so there is nothing to activate until workspaces can be shared.

use better_auth::prelude::{CreateMember, CreateOrganization, UpdateOrganization};
use better_auth::{AuthError, BetterAuth, SchemaValue};

use crate::auth::AppAuthSchema;

/// Name every freshly provisioned workspace starts with; the settings page
/// renames it through `rename_default_workspace`.
pub const DEFAULT_WORKSPACE_NAME: &str = "Default";

/// Role written on the owner membership, mirrored by the plugin's
/// `creator_role` so the two cannot drift apart.
pub const DEFAULT_WORKSPACE_ROLE: &str = "owner";

/// A resolved workspace, ready for the dashboard shell.
#[derive(Clone, Debug)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub slug: String,
    /// The caller's role in this workspace.
    pub role: String,
}

#[derive(Debug)]
pub enum WorkspaceError {
    /// Store read or write failed.
    Store(AuthError),
    /// Provisioning raced and the winning row still cannot be read.
    Missing,
}

impl From<AuthError> for WorkspaceError {
    fn from(error: AuthError) -> Self {
        Self::Store(error)
    }
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(error) => write!(formatter, "{error}"),
            Self::Missing => write!(formatter, "the default workspace is unreadable"),
        }
    }
}

impl std::error::Error for WorkspaceError {}

/// Slug reserved for a user's default workspace.
pub fn default_slug(user_id: &str) -> String {
    format!("default-{user_id}")
}

fn text(value: SchemaValue<String>, fallback: &str) -> String {
    value.typed().ok().cloned().unwrap_or_else(|| fallback.to_owned())
}

/// The caller's default workspace, created together with its owner membership
/// on first read.
pub async fn resolve_default_workspace(
    auth: &BetterAuth<AppAuthSchema>,
    user_id: &str,
) -> Result<Workspace, WorkspaceError> {
    let store = auth.store();
    let slug = default_slug(user_id);

    let organization = match store
        .get_organization_by_slug(&slug)
        .await
        .map_err(WorkspaceError::Store)?
    {
        Some(found) => found,
        None => match store
            .create_organization(CreateOrganization::new(DEFAULT_WORKSPACE_NAME, slug.clone()))
            .await
        {
            Ok(created) => created,
            // A concurrent first read loses the unique slug index: use the row
            // the winner created instead of failing the request.
            Err(_) => store
                .get_organization_by_slug(&slug)
                .await
                .map_err(WorkspaceError::Store)?
                .ok_or(WorkspaceError::Missing)?,
        },
    };

    let id = text(organization.id, "");
    let name = text(organization.name, DEFAULT_WORKSPACE_NAME);

    // Membership is what puts the user *under* the workspace. A missing row
    // means the organization predates this feature (or was created by hand),
    // so re-attach the owner rather than reporting a phantom workspace.
    let role = match store.get_member(&id, user_id).await.map_err(WorkspaceError::Store)? {
        Some(member) => text(member.role, DEFAULT_WORKSPACE_ROLE),
        None => {
            let member = store
                .create_member(CreateMember::new(
                    id.clone(),
                    user_id.to_owned(),
                    DEFAULT_WORKSPACE_ROLE,
                ))
                .await
                .map_err(WorkspaceError::Store)?;
            text(member.role, DEFAULT_WORKSPACE_ROLE)
        }
    };

    Ok(Workspace {
        id,
        name,
        slug,
        role,
    })
}

/// Rename the caller's default workspace.
///
/// The slug keeps its `default-<user id>` form: it is the identity the lookup
/// is keyed on, so only the display name moves. The avatar letter follows the
/// name, because the avatar service is seeded from it.
pub async fn rename_default_workspace(
    auth: &BetterAuth<AppAuthSchema>,
    user_id: &str,
    name: &str,
) -> Result<Workspace, WorkspaceError> {
    // Resolving first means the rename can never write to a workspace the
    // caller does not own: the id always comes from their own slug.
    let workspace = resolve_default_workspace(auth, user_id).await?;

    let updated = auth
        .store()
        .update_organization(
            &workspace.id,
            UpdateOrganization {
                name: Some(name.to_owned()),
                ..Default::default()
            },
        )
        .await
        .map_err(WorkspaceError::Store)?;

    Ok(Workspace {
        name: text(updated.name, name),
        ..workspace
    })
}
