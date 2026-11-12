use crate::proxy::adapters::{self, EventStream, TargetOptions};
use crate::proxy::ir::{Chat, ErrCode, Errx, Turn};
use crate::services::catalog::{CatalogCache, ModelCatalogEntry};
use crate::services::providers::{self, Provider};

pub struct Resolved {
    pub entry: ModelCatalogEntry,
    pub provider: &'static Provider,
    pub key: String,
}

pub fn resolve(cache: &CatalogCache, model_id: &str) -> Result<Resolved, Errx> {
    let entry = cache.find(model_id).ok_or_else(|| {
        Errx::new(
            ErrCode::NotFound,
            404,
            format!("no model named `{model_id}` is published"),
        )
    })?;
    let provider = providers::lookup(&entry.provider).ok_or_else(|| {
        Errx::new(
            ErrCode::Upstream,
            503,
            format!("the catalog provider `{}` does not exist", entry.provider),
        )
    })?;
    if entry.compat != provider.compat {
        return Err(Errx::new(
            ErrCode::Upstream,
            503,
            "the catalog points this model at a mismatched provider family",
        ));
    }
    let key = providers::key_of(provider).ok_or_else(|| {
        Errx::new(
            ErrCode::ProviderUnconfigured,
            503,
            format!(
                "the {} upstream is not configured on this router",
                provider.name
            ),
        )
        .with_provider(provider.name)
    })?;
    Ok(Resolved {
        entry,
        provider,
        key,
    })
}

pub fn guard_entitlement(resolved: &Resolved, allowlist: &[String]) -> Result<(), Errx> {
    if allowlist
        .iter()
        .any(|allowed| allowed == resolved.entry.id())
    {
        Ok(())
    } else {
        Err(Errx::new(
            ErrCode::Forbidden,
            403,
            format!(
                "model `{}` is not enabled for your plan",
                resolved.entry.id()
            ),
        ))
    }
}

fn target_for(resolved: &Resolved) -> Result<adapters::Target, Errx> {
    let mut target = adapters::target_for(
        resolved.provider,
        &resolved.entry.upstream_model,
        &resolved.key,
        TargetOptions::default(),
    )?;
    target.provider_routing = resolved.entry.provider_routing.clone();
    Ok(target)
}

pub async fn stream_chat(
    client: &reqwest::Client,
    resolved: &Resolved,
    chat: Chat,
) -> Result<EventStream, Errx> {
    let target = target_for(resolved)?;
    adapters::chat_stream(client, &target, chat).await
}

pub async fn finish_chat(
    client: &reqwest::Client,
    resolved: &Resolved,
    chat: Chat,
) -> Result<Turn, Errx> {
    let target = target_for(resolved)?;
    adapters::chat_once(client, &target, chat).await
}
