use better_auth::seaorm::{DatabaseHookUpdate, SeaOrmHookContext, SeaOrmHooks};
use better_auth::{AuthSchema, FieldValue};

use crate::services::avatar::initials_avatar_url;

/// Assigns a generated initials avatar when a user is created without one,
/// e.g. email/password sign-ups. OAuth providers that supply a picture are
/// left untouched.
#[derive(Clone, Default)]
pub struct InitialAvatarHook;

#[better_auth::database_hooks()]
impl<S: AuthSchema> SeaOrmHooks<S> for InitialAvatarHook {
    async fn before_create_user(
        &self,
        user: &mut better_auth::FieldMap,
        ctx: &SeaOrmHookContext<'_, S>,
    ) -> better_auth::AuthResult<DatabaseHookUpdate<better_auth::FieldMap>> {
        let _ = ctx;
        let already_has_image = matches!(
            user.get("image"),
            Some(FieldValue::String(image)) if !image.is_empty()
        );
        if already_has_image {
            return Ok(DatabaseHookUpdate::Continue);
        }

        let email = user
            .get("email")
            .and_then(|value| value.json().ok())
            .and_then(|value| value.as_ref().and_then(|v| v.as_str().map(str::to_owned)))
            .filter(|email| !email.is_empty());

        let Some(email) = email else {
            return Ok(DatabaseHookUpdate::Continue);
        };

        let seed = user
            .get("name")
            .and_then(|value| value.json().ok())
            .and_then(|value| value.as_ref().and_then(|v| v.as_str().map(str::to_owned)))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| email.clone());

        user.insert("image".into(), FieldValue::String(initials_avatar_url(&seed)));
        Ok(DatabaseHookUpdate::Continue)
    }
}
