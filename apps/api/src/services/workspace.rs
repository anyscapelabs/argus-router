use better_auth::prelude::{CreateMember, CreateOrganization, UpdateOrganization};
use better_auth::{AuthError, BetterAuth, SchemaValue};

use crate::auth::AppAuthSchema;

pub const DEFAULT_WORKSPACE_NAME: &str = "Default";

pub const DEFAULT_WORKSPACE_ROLE: &str = "owner";

#[derive(Clone, Debug)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub role: String,
}

#[derive(Debug)]
pub enum WorkspaceError {
    Store(AuthError),
    Missing,
    Preclaimed,
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
            Self::Preclaimed => write!(
                formatter,
                "the default workspace slug is held by another account"
            ),
        }
    }
}

impl std::error::Error for WorkspaceError {}

pub fn default_slug(user_id: &str) -> String {
    format!("default-{user_id}")
}

fn text(value: SchemaValue<String>, fallback: &str) -> String {
    value
        .typed()
        .ok()
        .cloned()
        .unwrap_or_else(|| fallback.to_owned())
}

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
            .create_organization(CreateOrganization::new(
                DEFAULT_WORKSPACE_NAME,
                slug.clone(),
            ))
            .await
        {
            Ok(created) => created,
            Err(_) => store
                .get_organization_by_slug(&slug)
                .await
                .map_err(WorkspaceError::Store)?
                .ok_or(WorkspaceError::Missing)?,
        },
    };

    let id = text(organization.id, "");
    let name = text(organization.name, DEFAULT_WORKSPACE_NAME);

    let role = match store
        .get_member(&id, user_id)
        .await
        .map_err(WorkspaceError::Store)?
    {
        Some(member) => text(member.role, DEFAULT_WORKSPACE_ROLE),
        None => {
            let existing_members = store
                .count_organization_members(&id)
                .await
                .map_err(WorkspaceError::Store)?;
            if existing_members > 0 {
                return Err(WorkspaceError::Preclaimed);
            }
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

pub async fn rename_default_workspace(
    auth: &BetterAuth<AppAuthSchema>,
    user_id: &str,
    name: &str,
) -> Result<Workspace, WorkspaceError> {
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
