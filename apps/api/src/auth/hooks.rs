use std::sync::OnceLock;

use better_auth::seaorm::{DatabaseHookUpdate, SeaOrmHookContext, SeaOrmHooks};
use better_auth::{AuthSchema, FieldValue};

use crate::services::avatar::initials_avatar_url;
use crate::services::cache::Cache;

static CACHE: OnceLock<Cache> = OnceLock::new();

pub fn init_profile_cache(cache: Cache) {
    let _ = CACHE.set(cache);
}

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

        user.insert(
            "image".into(),
            FieldValue::String(initials_avatar_url(&seed)),
        );
        Ok(DatabaseHookUpdate::Continue)
    }

    async fn after_update_user(
        &self,
        user: Option<&better_auth::wire::UserView>,
        ctx: &SeaOrmHookContext<'_, S>,
    ) -> better_auth::AuthResult<()> {
        let _ = ctx;
        if let (Some(cache), Some(user)) = (CACHE.get(), user) {
            if let Ok(id) = user.id.typed() {
                cache.delete(&format!("argus:profile:{id}")).await;
            }
        }
        Ok(())
    }
}
