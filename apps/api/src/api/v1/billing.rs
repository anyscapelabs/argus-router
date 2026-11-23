use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json as AxumJson, Router};
use base64::Engine;
use better_auth::integrations::axum::CurrentSession;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Sha256;

use crate::auth::AppAuthSchema;
use crate::services::app_db::{CheckoutClaim, FREE_PLAN_ID, SubscriptionView};
use crate::services::dodo::DodoClient;
use crate::state::AppState;

type ApiProblem = (StatusCode, AxumJson<Value>);
type HmacSha256 = Hmac<Sha256>;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(summary))
        .route("/checkout", post(checkout))
        .route("/portal", post(portal))
        .route("/webhook", post(webhook))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BillingSummary {
    plan_id: String,
    subscription: Option<SubscriptionView>,
    available_plan_ids: Vec<String>,
    billing_configured: bool,
    has_billing_customer: bool,
}

async fn summary(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
) -> Result<AxumJson<BillingSummary>, ApiProblem> {
    let user_id = user_id(&session)?;
    let (plan_id, subscription, customer_id) = tokio::try_join!(
        state.app_db.plan_of(&user_id),
        state.app_db.subscription_of(&user_id),
        state.app_db.billing_customer_id(&user_id),
    )
    .map_err(database_problem)?;
    let billing_configured = state.dodo.is_some();
    let available_plan_ids = if billing_configured {
        let mut available_plan_ids: Vec<String> = state
            .billing_catalog
            .subscriptions
            .iter()
            .filter(|item| !item.product_id.is_empty())
            .map(|item| item.plan_id.clone())
            .collect();
        for plan_id in state
            .app_db
            .mapped_plan_products()
            .await
            .map_err(database_problem)?
        {
            if !available_plan_ids.contains(&plan_id) {
                available_plan_ids.push(plan_id);
            }
        }
        available_plan_ids
    } else {
        Vec::new()
    };
    Ok(AxumJson(BillingSummary {
        plan_id,
        subscription,
        available_plan_ids,
        billing_configured,
        has_billing_customer: customer_id.is_some(),
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckoutRequest {
    plan_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RedirectResponse {
    checkout_url: String,
}

struct CheckoutTarget {
    checkout_key: String,
    product_id: String,
    metadata: Value,
}

async fn subscription_product_for_checkout(
    state: &AppState,
    dodo: &DodoClient,
    plan_id: &str,
) -> Result<String, ApiProblem> {
    if let Some(product_id) = state
        .app_db
        .plan_product_of(plan_id)
        .await
        .map_err(database_problem)?
    {
        if dodo
            .product_exists(&product_id)
            .await
            .map_err(|error| provider_problem("plan product lookup", error))?
        {
            return Ok(product_id);
        }
        tracing::warn!(
            %plan_id,
            %product_id,
            "stored plan product is unknown to the payment provider; creating a new one"
        );
    }
    if let Some(product_id) = state.billing_catalog.subscription_product(plan_id) {
        if dodo
            .product_exists(product_id)
            .await
            .map_err(|error| provider_problem("plan product lookup", error))?
        {
            return Ok(product_id.to_owned());
        }
        tracing::warn!(
            %plan_id,
            %product_id,
            "catalog plan product is unknown to the payment provider; creating a new one"
        );
    }
    let entry = state
        .billing_catalog
        .subscription_entry(plan_id)
        .filter(|entry| entry.price_usd > 0)
        .ok_or_else(|| billing_product_problem("plan"))?;
    let product_id = dodo
        .create_subscription_product(
            plan_id,
            &format!("Argus {plan_id} Subscription"),
            entry.price_usd * 100,
            json!({ "argus_plan_id": plan_id }),
        )
        .await
        .map_err(|error| provider_problem("subscription product", error))?;
    state
        .app_db
        .upsert_plan_product(plan_id, &product_id)
        .await
        .map_err(database_problem)?;
    Ok(product_id)
}

async fn checkout(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
    AxumJson(request): AxumJson<CheckoutRequest>,
) -> Result<AxumJson<RedirectResponse>, ApiProblem> {
    let user_id = user_id(&session)?;
    let dodo = dodo_client(&state)?;
    let Some(plan_id) = request.plan_id else {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "INVALID_CHECKOUT",
            "Provide a `planId`.",
        ));
    };
    if plan_id == FREE_PLAN_ID {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "INVALID_PLAN",
            "The free plan does not require checkout.",
        ));
    }
    if let Some(subscription) = state
        .app_db
        .subscription_of(&user_id)
        .await
        .map_err(database_problem)?
    {
        if !matches!(
            subscription.status.as_str(),
            "cancelled" | "failed" | "expired"
        ) {
            return Err(problem(
                StatusCode::CONFLICT,
                "ACTIVE_SUBSCRIPTION",
                "Manage the existing subscription in the billing portal.",
            ));
        }
    }
    let product_id = subscription_product_for_checkout(&state, dodo, &plan_id).await?;
    let target = CheckoutTarget {
        checkout_key: format!("plan:{user_id}:{plan_id}"),
        product_id,
        metadata: json!({ "argus_user_id": user_id, "argus_plan_id": plan_id }),
    };
    match state
        .app_db
        .claim_billing_checkout(&target.checkout_key, &user_id)
        .await
        .map_err(database_problem)?
    {
        CheckoutClaim::Existing(checkout_url) => {
            return Ok(AxumJson(RedirectResponse { checkout_url }));
        }
        CheckoutClaim::InProgress => {
            return Err(problem(
                StatusCode::CONFLICT,
                "CHECKOUT_IN_PROGRESS",
                "A checkout session is already being opened. Try again in a moment.",
            ));
        }
        CheckoutClaim::Claimed => {}
    }
    let outcome = async {
        let return_url = format!(
            "{}/console/overview",
            state.settings.dashboard_url.trim_end_matches('/')
        );
        let idempotency_key = format!("argus-checkout-{}", target.checkout_key);
        let customer_id = ensure_customer(&state, dodo, &session, &user_id).await?;
        let checkout = dodo
            .create_checkout(
                &customer_id,
                &target.product_id,
                &return_url,
                target.metadata.clone(),
                &idempotency_key,
            )
            .await;
        let checkout_url = match checkout {
            Ok(checkout_url) => checkout_url,
            Err(error) if error.is_missing_customer() => {
                let customer_id =
                    forcibly_recreate_customer(&state, dodo, &session, &user_id).await?;
                dodo.create_checkout(
                    &customer_id,
                    &target.product_id,
                    &return_url,
                    target.metadata.clone(),
                    &idempotency_key,
                )
                .await
                .map_err(|error| provider_problem("checkout session", error))?
            }
            Err(error) => return Err(provider_problem("checkout session", error)),
        };
        Ok::<String, ApiProblem>(checkout_url)
    }
    .await;
    match outcome {
        Ok(checkout_url) => {
            if let Err(error) = state
                .app_db
                .finish_billing_checkout(&target.checkout_key, &checkout_url)
                .await
            {
                tracing::error!(%error, "billing checkout session registration failed");
            }
            Ok(AxumJson(RedirectResponse { checkout_url }))
        }
        Err(problem_response) => {
            if let Err(error) = state
                .app_db
                .clear_billing_checkout(&target.checkout_key)
                .await
            {
                tracing::error!(%error, "billing checkout session cleanup failed");
            }
            Err(problem_response)
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PortalResponse {
    portal_url: String,
}

async fn portal(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
) -> Result<AxumJson<PortalResponse>, ApiProblem> {
    let user_id = user_id(&session)?;
    let dodo = dodo_client(&state)?;
    let customer_id = ensure_customer(&state, &dodo, &session, &user_id).await?;
    let return_url =
        state.settings.dodo_return_url.clone().unwrap_or_else(|| {
            format!("{}/console/billing", state.settings.dashboard_url)
        });
    let portal = dodo.create_portal(&customer_id, &return_url).await;
    let portal_url = match portal {
        Ok(portal_url) => portal_url,
        Err(error) if error.is_missing_customer() => {
            let customer_id =
                forcibly_recreate_customer(&state, &dodo, &session, &user_id).await?;
            dodo.create_portal(&customer_id, &return_url)
                .await
                .map_err(|error| provider_problem("customer portal", error))?
        }
        Err(error) => return Err(provider_problem("customer portal", error)),
    };
    Ok(AxumJson(PortalResponse { portal_url }))
}

async fn ensure_customer(
    state: &AppState,
    dodo: &Arc<DodoClient>,
    session: &CurrentSession<AppAuthSchema>,
    user_id: &str,
) -> Result<String, ApiProblem> {
    if let Some(customer_id) = state
        .app_db
        .billing_customer_id(user_id)
        .await
        .map_err(database_problem)?
    {
        return Ok(customer_id);
    }
    let email = session
        .user
        .email
        .typed()
        .ok()
        .cloned()
        .flatten()
        .filter(|email| !email.is_empty())
        .ok_or_else(|| {
            problem(
                StatusCode::BAD_REQUEST,
                "BILLING_EMAIL_REQUIRED",
                "A verified account email is required for checkout.",
            )
        })?;
    let name = session
        .user
        .name
        .typed()
        .ok()
        .cloned()
        .flatten()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| email.clone());
    let customer_id = dodo
        .create_customer(user_id, &name, &email)
        .await
        .map_err(|error| provider_problem("customer creation", error))?;
    state
        .app_db
        .upsert_billing_customer(user_id, &customer_id)
        .await
        .map_err(database_problem)?;
    Ok(customer_id)
}

async fn forcibly_recreate_customer(
    state: &AppState,
    dodo: &Arc<DodoClient>,
    session: &CurrentSession<AppAuthSchema>,
    user_id: &str,
) -> Result<String, ApiProblem> {
    tracing::warn!(
        %user_id,
        "billing customer is unknown to the payment provider; recreating it"
    );
    state
        .app_db
        .clear_billing_customer(user_id)
        .await
        .map_err(database_problem)?;
    ensure_customer(state, dodo, session, user_id).await
}

async fn webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, ApiProblem> {
    let secret = state.settings.dodo_webhook_key.as_deref().ok_or_else(|| {
        problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "WEBHOOK_NOT_CONFIGURED",
            "Webhook verification is not configured.",
        )
    })?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CLOCK_ERROR",
                "System clock error.",
            )
        })?
        .as_secs() as i64;
    let webhook_id = verify_webhook_at(&headers, &body, secret, now)
        .map_err(|message| problem(StatusCode::BAD_REQUEST, "INVALID_WEBHOOK", message))?;
    if state
        .app_db
        .webhook_seen(&webhook_id)
        .await
        .map_err(database_problem)?
    {
        return Ok(StatusCode::OK);
    }
    let payload: Value = serde_json::from_slice(&body).map_err(|_| {
        problem(
            StatusCode::BAD_REQUEST,
            "INVALID_WEBHOOK",
            "Webhook payload is not valid JSON.",
        )
    })?;
    process_webhook(&state, &payload).await.map_err(|error| {
        tracing::error!(webhook_id, %error, "billing webhook processing failed");
        problem(
            StatusCode::INTERNAL_SERVER_ERROR,
            "WEBHOOK_PROCESSING_FAILED",
            "Webhook processing failed.",
        )
    })?;
    state
        .app_db
        .mark_webhook_seen(&webhook_id)
        .await
        .map_err(database_problem)?;
    Ok(StatusCode::OK)
}

async fn process_webhook(state: &AppState, payload: &Value) -> Result<(), String> {
    let event_type = required_string(payload, "type")?;
    let data = payload
        .get("data")
        .ok_or_else(|| "webhook has no data field".to_owned())?;
    if event_type.starts_with("subscription.") {
        sync_subscription(
            state,
            data,
            payload.get("timestamp").and_then(Value::as_str),
        )
        .await
    } else if event_type == "payment.succeeded" || event_type == "payment.failed" {
        tracing::info!(event_type, "ignoring legacy payment event");
        Ok(())
    } else {
        Ok(())
    }
}

pub async fn reconcile_subscriptions(state: &AppState) {
    let Some(dodo) = state.dodo.as_ref() else {
        return;
    };
    let subscriptions = match state.app_db.active_subscriptions().await {
        Ok(subscriptions) => subscriptions,
        Err(error) => {
            tracing::error!(%error, "billing subscription reconciliation lookup failed");
            return;
        }
    };
    for stored in subscriptions {
        let remote = match dodo.subscription(&stored.subscription_id).await {
            Ok(remote) => remote,
            Err(error) => {
                tracing::error!(
                    subscription_id = stored.subscription_id,
                    status = ?error.status(),
                    %error,
                    "Dodo subscription reconciliation request failed"
                );
                continue;
            }
        };
        let plan_id = state
            .billing_catalog
            .plan_for_product(&remote.product_id)
            .unwrap_or(&stored.plan_id)
            .to_owned();
        let result = async {
            state
                .app_db
                .upsert_billing_customer(&stored.user_id, &remote.customer.customer_id)
                .await
                .map_err(|error| error.to_string())?;
            persist_subscription(
                state,
                &remote.subscription_id,
                &stored.user_id,
                &plan_id,
                &remote.status,
                remote.next_billing_date.as_deref(),
                remote.past_due_ends_at.as_deref(),
                remote.cancel_at_next_billing_date,
                remote.cancelled_at.as_deref(),
                None,
            )
            .await?;
            state
                .app_db
                .clear_user_plan_checkouts(&stored.user_id)
                .await
                .map_err(|error| error.to_string())
        }
        .await;
        if let Err(error) = result {
            tracing::error!(
                subscription_id = stored.subscription_id,
                previous_status = stored.status,
                %error,
                "billing subscription reconciliation update failed"
            );
        }
    }
}

async fn sync_subscription(
    state: &AppState,
    data: &Value,
    event_at: Option<&str>,
) -> Result<(), String> {
    let subscription_id = required_string(data, "subscription_id")?;
    let status = required_string(data, "status")?;
    let product_id = required_string(data, "product_id")?;
    let existing = state
        .app_db
        .subscription_owner(subscription_id)
        .await
        .map_err(|error| error.to_string())?;
    let customer_id = data
        .pointer("/customer/customer_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    let mapped_user = match customer_id {
        Some(customer_id) => state
            .app_db
            .billing_user_of_customer(customer_id)
            .await
            .map_err(|error| error.to_string())?,
        None => None,
    };
    let metadata_user = data
        .pointer("/metadata/argus_user_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    if let (Some(mapped), Some(metadata)) = (&mapped_user, &metadata_user) {
        if mapped != metadata {
            return Err(
                "billing customer and webhook metadata identify different users".to_owned(),
            );
        }
    }
    let user_id = mapped_user
        .or(metadata_user)
        .or_else(|| existing.as_ref().map(|item| item.0.clone()))
        .ok_or_else(|| "subscription cannot be mapped to an Argus user".to_owned())?;
    if let Some((owner, _)) = &existing {
        if owner != &user_id {
            return Err("subscription owner does not match webhook identity".to_owned());
        }
    }
    if let Some(customer_id) = customer_id {
        state
            .app_db
            .upsert_billing_customer(&user_id, customer_id)
            .await
            .map_err(|error| error.to_string())?;
    }
    let static_plan = state
        .billing_catalog
        .plan_for_product(product_id)
        .map(str::to_owned);
    let db_plan = state
        .app_db
        .plan_for_product(product_id)
        .await
        .map_err(|error| error.to_string())?;
    let plan_id = static_plan
        .or(db_plan)
        .or_else(|| existing.as_ref().map(|item| item.1.clone()))
        .ok_or_else(|| format!("no plan is configured for Dodo product `{product_id}`"))?;
    let next_billing_date = data.get("next_billing_date").and_then(Value::as_str);
    let past_due_ends_at = data.get("past_due_ends_at").and_then(Value::as_str);
    let cancel_at_next_billing_date = data
        .get("cancel_at_next_billing_date")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let cancelled_at = data.get("cancelled_at").and_then(Value::as_str);
    persist_subscription(
        state,
        subscription_id,
        &user_id,
        &plan_id,
        status,
        next_billing_date,
        past_due_ends_at,
        cancel_at_next_billing_date,
        cancelled_at,
        event_at,
    )
    .await?;
    if let Err(error) = state.app_db.clear_user_plan_checkouts(&user_id).await {
        tracing::error!(%error, "billing plan checkout session cleanup failed");
    }
    if matches!(status, "active" | "pending") {
        if let Ok(Some(other)) = state
            .app_db
            .duplicate_managed_subscription(&user_id, subscription_id)
            .await
        {
            tracing::warn!(
                user_id = %user_id,
                subscription_id,
                other_subscription_id = %other,
                "user has more than one managed subscription; manual review recommended"
            );
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn persist_subscription(
    state: &AppState,
    subscription_id: &str,
    user_id: &str,
    plan_id: &str,
    status: &str,
    next_billing_date: Option<&str>,
    past_due_ends_at: Option<&str>,
    cancel_at_next_billing_date: bool,
    cancelled_at: Option<&str>,
    event_at: Option<&str>,
) -> Result<(), String> {
    state
        .app_db
        .upsert_subscription(
            subscription_id,
            user_id,
            plan_id,
            status,
            next_billing_date,
            past_due_ends_at,
            cancel_at_next_billing_date,
            cancelled_at,
            event_at,
        )
        .await
        .map_err(|error| error.to_string())?;
    let entitled_plan = state
        .app_db
        .entitled_subscription_plan(user_id)
        .await
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| FREE_PLAN_ID.to_owned());
    state
        .app_db
        .set_user_plan(user_id, &entitled_plan)
        .await
        .map_err(|error| error.to_string())
}

fn verify_webhook_at(
    headers: &HeaderMap,
    body: &[u8],
    secret: &str,
    now: i64,
) -> Result<String, &'static str> {
    let webhook_id = header(headers, "webhook-id")?;
    let timestamp_header = header(headers, "webhook-timestamp")?;
    let signatures = header(headers, "webhook-signature")?;
    if webhook_id.is_empty() || webhook_id.contains('.') {
        return Err("Invalid webhook id.");
    }
    let timestamp = timestamp_header
        .parse::<i64>()
        .map_err(|_| "Invalid webhook timestamp.")?;
    if now - timestamp > 300 || timestamp - now > 300 {
        return Err("Webhook timestamp is outside the allowed window.");
    }
    let key = base64::engine::general_purpose::STANDARD
        .decode(secret.strip_prefix("whsec_").unwrap_or(secret))
        .map_err(|_| "Invalid webhook signing key.")?;
    if key.is_empty() {
        return Err("Invalid webhook signing key.");
    }
    let mut message =
        Vec::with_capacity(webhook_id.len() + timestamp_header.len() + body.len() + 2);
    message.extend_from_slice(webhook_id.as_bytes());
    message.push(b'.');
    message.extend_from_slice(timestamp_header.as_bytes());
    message.push(b'.');
    message.extend_from_slice(body);
    let valid = signatures.split_ascii_whitespace().any(|signature| {
        let Some(encoded) = signature.strip_prefix("v1,") else {
            return false;
        };
        let Ok(signature) = base64::engine::general_purpose::STANDARD.decode(encoded) else {
            return false;
        };
        let Ok(mut mac) = HmacSha256::new_from_slice(&key) else {
            return false;
        };
        mac.update(&message);
        mac.verify_slice(&signature).is_ok()
    });
    if !valid {
        return Err("Invalid webhook signature.");
    }
    Ok(webhook_id.to_owned())
}

fn header<'a>(headers: &'a HeaderMap, name: &'static str) -> Result<&'a str, &'static str> {
    headers
        .get(name)
        .ok_or("Missing webhook signature headers.")?
        .to_str()
        .map_err(|_| "Invalid webhook signature headers.")
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("webhook data has no `{field}`"))
}

fn user_id(session: &CurrentSession<AppAuthSchema>) -> Result<String, ApiProblem> {
    session
        .user
        .id
        .typed()
        .ok()
        .cloned()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            problem(
                StatusCode::UNAUTHORIZED,
                "SESSION",
                "The session carries no user identifier.",
            )
        })
}

fn dodo_client(state: &AppState) -> Result<&Arc<DodoClient>, ApiProblem> {
    state.dodo.as_ref().ok_or_else(|| {
        problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "BILLING_NOT_CONFIGURED",
            "Billing is not configured.",
        )
    })
}

fn database_problem(error: sqlx::Error) -> ApiProblem {
    tracing::error!(%error, "billing database operation failed");
    problem(
        StatusCode::INTERNAL_SERVER_ERROR,
        "BILLING_DATABASE",
        "Billing data could not be loaded.",
    )
}

fn provider_problem(operation: &str, error: crate::services::dodo::DodoError) -> ApiProblem {
    tracing::error!(operation, status = ?error.status(), %error, "Dodo Payments request failed");
    problem(
        StatusCode::BAD_GATEWAY,
        "BILLING_PROVIDER",
        "The billing provider request failed.",
    )
}

fn billing_product_problem(kind: &str) -> ApiProblem {
    problem(
        StatusCode::SERVICE_UNAVAILABLE,
        "BILLING_PRODUCT_NOT_CONFIGURED",
        &format!("The selected {kind} is not configured for checkout."),
    )
}

fn problem(status: StatusCode, code: &str, message: &str) -> ApiProblem {
    (
        status,
        AxumJson(json!({ "error": { "code": code, "message": message } })),
    )
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::verify_webhook_at;

    #[test]
    fn verifies_standard_webhook_test_vector() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "webhook-id",
            HeaderValue::from_static("msg_27UH4WbU6Z5A5EzD8u03UvzRbpk"),
        );
        headers.insert("webhook-timestamp", HeaderValue::from_static("1649367553"));
        headers.insert(
            "webhook-signature",
            HeaderValue::from_static("v1,tZ1I4/hDygAJgO5TYxiSd6Sd0kDW6hPenDe+bTa3Kkw="),
        );
        let body = br#"{"email":"test@example.com","username":"test_user"}"#;
        let result = verify_webhook_at(
            &headers,
            body,
            "whsec_C2FVsBQIhrscChlQIMV+b5sSYspob7oD",
            1649367553,
        );
        assert_eq!(result.as_deref(), Ok("msg_27UH4WbU6Z5A5EzD8u03UvzRbpk"));
    }

    #[test]
    fn rejects_stale_webhook() {
        let mut headers = HeaderMap::new();
        headers.insert("webhook-id", HeaderValue::from_static("msg_test"));
        headers.insert("webhook-timestamp", HeaderValue::from_static("100"));
        headers.insert("webhook-signature", HeaderValue::from_static("v1,invalid"));
        assert!(verify_webhook_at(&headers, b"{}", "dGVzdA==", 401).is_err());
    }
}
