use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
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
use crate::services::dodo::{CreditPack, DodoClient};
use crate::state::AppState;

type ApiProblem = (StatusCode, AxumJson<Value>);
type HmacSha256 = Hmac<Sha256>;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(summary))
        .route("/checkout", post(checkout))
        .route("/portal", post(portal))
        .route("/webhook", post(webhook))
        .route("/invoices", get(invoices))
        .route("/invoices/{payment_id}/download", get(invoice_download))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BillingSummary {
    plan_id: String,
    subscription: Option<SubscriptionView>,
    credit_balance_micros: i64,
    credit_packs: Vec<CreditPackView>,
    available_plan_ids: Vec<String>,
    billing_configured: bool,
    has_billing_customer: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreditPackView {
    id: String,
    label: String,
    price_usd: i64,
    credits_micros: i64,
    available: bool,
}

impl CreditPackView {
    fn from_pack(pack: &CreditPack, billing_configured: bool) -> Self {
        Self {
            id: pack.id.clone(),
            label: pack.label.clone(),
            price_usd: pack.price_usd,
            credits_micros: pack.credits_micros,
            available: billing_configured && !pack.product_id.is_empty(),
        }
    }
}

async fn summary(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
) -> Result<AxumJson<BillingSummary>, ApiProblem> {
    let user_id = user_id(&session)?;
    let (plan_id, subscription, credit_balance_micros, customer_id) = tokio::try_join!(
        state.app_db.plan_of(&user_id),
        state.app_db.subscription_of(&user_id),
        state.app_db.credit_balance(&user_id),
        state.app_db.billing_customer_id(&user_id),
    )
    .map_err(database_problem)?;
    let billing_configured = state.dodo.is_some();
    let credit_packs = state
        .billing_catalog
        .credit_packs
        .iter()
        .map(|pack| CreditPackView::from_pack(pack, billing_configured))
        .collect();
    let available_plan_ids = if billing_configured {
        state
            .billing_catalog
            .subscriptions
            .iter()
            .filter(|item| !item.product_id.is_empty())
            .map(|item| item.plan_id.clone())
            .collect()
    } else {
        Vec::new()
    };
    Ok(AxumJson(BillingSummary {
        plan_id,
        subscription,
        credit_balance_micros,
        credit_packs,
        available_plan_ids,
        billing_configured,
        has_billing_customer: customer_id.is_some(),
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckoutRequest {
    plan_id: Option<String>,
    credit_pack_id: Option<String>,
    amount_usd: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RedirectResponse {
    checkout_url: String,
}

const MIN_CUSTOM_CREDIT_CENTS: i64 = 100;
const MAX_CUSTOM_CREDIT_CENTS: i64 = 50_000;

fn custom_amount_cents(amount_usd: f64) -> Option<i64> {
    if !amount_usd.is_finite() {
        return None;
    }
    let cents = (amount_usd * 100.0).round() as i64;
    (MIN_CUSTOM_CREDIT_CENTS..=MAX_CUSTOM_CREDIT_CENTS).contains(&cents).then_some(cents)
}

fn credit_product_name(cents: i64) -> String {
    if cents % 100 == 0 {
        format!("Argus Prepaid Credits ${}", cents / 100)
    } else {
        format!("Argus Prepaid Credits ${:.2}", cents as f64 / 100.0)
    }
}

struct CheckoutTarget {
    checkout_key: String,
    product_id: String,
    metadata: Value,
}

async fn checkout(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
    AxumJson(request): AxumJson<CheckoutRequest>,
) -> Result<AxumJson<RedirectResponse>, ApiProblem> {
    let user_id = user_id(&session)?;
    let dodo = dodo_client(&state)?;
    let target = match (&request.plan_id, &request.credit_pack_id, request.amount_usd) {
        (Some(plan_id), None, None) => {
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
            let product_id = state
                .billing_catalog
                .subscription_product(plan_id)
                .ok_or_else(|| billing_product_problem("plan"))?;
            CheckoutTarget {
                checkout_key: format!("plan:{user_id}:{plan_id}"),
                product_id: product_id.to_owned(),
                metadata: json!({ "argus_user_id": user_id, "argus_plan_id": plan_id }),
            }
        }
        (None, Some(pack_id), None) => {
            let pack = state
                .billing_catalog
                .credit_pack(pack_id)
                .filter(|pack| !pack.product_id.is_empty())
                .ok_or_else(|| billing_product_problem("credit pack"))?;
            CheckoutTarget {
                checkout_key: format!("credits:{user_id}:{}", pack.id),
                product_id: pack.product_id.clone(),
                metadata: json!({ "argus_user_id": user_id, "argus_credit_pack_id": pack.id }),
            }
        }
        (None, None, Some(amount_usd)) => {
            let cents = custom_amount_cents(amount_usd).ok_or_else(|| {
                problem(
                    StatusCode::BAD_REQUEST,
                    "INVALID_AMOUNT",
                    "Custom credit amounts must be between $1 and $500.",
                )
            })?;
            let product_id = match state
                .app_db
                .credit_product_of_amount(cents)
                .await
                .map_err(database_problem)?
            {
                Some(product_id) => product_id,
                None => {
                    let product_id = dodo
                        .create_credit_product(
                            &credit_product_name(cents),
                            cents,
                            json!({ "argus_credit_amount_cents": cents }),
                        )
                        .await
                        .map_err(|error| provider_problem("credit product", error))?;
                    state
                        .app_db
                        .upsert_credit_product(cents, &product_id)
                        .await
                        .map_err(database_problem)?;
                    state
                        .app_db
                        .credit_product_of_amount(cents)
                        .await
                        .map_err(database_problem)?
                        .ok_or_else(|| {
                            problem(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "BILLING_DATABASE",
                                "The credit product mapping could not be loaded.",
                            )
                        })?
                }
            };
            CheckoutTarget {
                checkout_key: format!("credits:{user_id}:{cents}"),
                product_id,
                metadata: json!({ "argus_user_id": user_id, "argus_credit_amount_cents": cents }),
            }
        }
        _ => {
            return Err(problem(
                StatusCode::BAD_REQUEST,
                "INVALID_CHECKOUT",
                "Provide exactly one of `planId`, `creditPackId` or `amountUsd`.",
            ));
        }
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
        let customer_id = ensure_customer(&state, dodo, &session, &user_id).await?;
        let return_url = state.settings.dodo_return_url.clone().unwrap_or_else(|| {
            format!("{}/console/settings/billing", state.settings.dashboard_url)
        });
        let checkout_url = dodo
            .create_checkout(
                &customer_id,
                &target.product_id,
                &return_url,
                target.metadata,
                &format!("argus-checkout-{}", target.checkout_key),
            )
            .await
            .map_err(|error| provider_problem("checkout session", error))?;
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
            format!("{}/console/settings/billing", state.settings.dashboard_url)
        });
    let portal_url = dodo
        .create_portal(&customer_id, &return_url)
        .await
        .map_err(|error| provider_problem("customer portal", error))?;
    Ok(AxumJson(PortalResponse { portal_url }))
}

async fn billing_customer_id_of(state: &AppState, user_id: &str) -> Result<String, ApiProblem> {
    state
        .app_db
        .billing_customer_id(user_id)
        .await
        .map_err(database_problem)?
        .ok_or_else(|| {
            problem(
                StatusCode::NOT_FOUND,
                "BILLING_CUSTOMER_NOT_FOUND",
                "No billing customer exists for this account.",
            )
        })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InvoiceView {
    payment_id: String,
    created_at: String,
    total_amount: i64,
    currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invoice_id: Option<String>,
    description: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InvoiceList {
    invoices: Vec<InvoiceView>,
}

async fn invoices(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
) -> Result<AxumJson<InvoiceList>, ApiProblem> {
    let user_id = user_id(&session)?;
    let dodo = dodo_client(&state)?;
    let Some(customer_id) = state
        .app_db
        .billing_customer_id(&user_id)
        .await
        .map_err(database_problem)?
    else {
        return Ok(AxumJson(InvoiceList { invoices: Vec::new() }));
    };
    let payments = dodo
        .list_payments(&customer_id)
        .await
        .map_err(|error| provider_problem("payment history", error))?;
    let invoices = payments
        .into_iter()
        .map(|payment| {
            let description = match payment.charge_type.as_deref() {
                Some("new") => "Subscription".to_owned(),
                Some("renewal") => "Subscription renewal".to_owned(),
                Some("plan_change") => "Plan change".to_owned(),
                Some("on_demand") => "On-demand charge".to_owned(),
                _ => "Prepaid credits".to_owned(),
            };
            InvoiceView {
                payment_id: payment.payment_id,
                created_at: payment.created_at,
                total_amount: payment.total_amount,
                currency: payment.currency,
                status: payment.status,
                invoice_id: payment.invoice_id,
                description,
            }
        })
        .collect();
    Ok(AxumJson(InvoiceList { invoices }))
}

async fn invoice_download(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
    Path(payment_id): Path<String>,
) -> Result<Response, ApiProblem> {
    let user_id = user_id(&session)?;
    let dodo = dodo_client(&state)?;
    let customer_id = billing_customer_id_of(&state, &user_id).await?;
    let payment = dodo
        .payment(&payment_id)
        .await
        .map_err(|error| provider_problem("payment lookup", error))?;
    if payment
        .customer
        .as_ref()
        .is_none_or(|customer| customer.customer_id != customer_id)
    {
        return Err(problem(
            StatusCode::NOT_FOUND,
            "INVOICE_NOT_FOUND",
            "No invoice was found for this account.",
        ));
    }
    let pdf = dodo
        .invoice_pdf(&payment_id)
        .await
        .map_err(|error| provider_problem("invoice download", error))?;
    let disposition = format!("attachment; filename=\"invoice-{payment_id}.pdf\"");
    Ok((
        StatusCode::OK,
        [
            (
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("application/pdf"),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                axum::http::HeaderValue::from_str(&disposition)
                    .unwrap_or(axum::http::HeaderValue::from_static("attachment")),
            ),
        ],
        Body::from(pdf),
    )
        .into_response())
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
    } else if event_type == "payment.succeeded" {
        grant_purchased_credits(state, data).await
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
    let plan_id = state
        .billing_catalog
        .plan_for_product(product_id)
        .map(str::to_owned)
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

async fn grant_purchased_credits(state: &AppState, data: &Value) -> Result<(), String> {
    let payment_id = required_string(data, "payment_id")?;
    let cart = match data.get("product_cart").and_then(Value::as_array) {
        Some(cart) => cart,
        None => return Ok(()),
    };
    let mut purchases: Vec<(i64, String)> = Vec::new();
    let mut purchased_pack_ids: Vec<&str> = Vec::new();
    let mut purchased_custom_cents: Vec<i64> = Vec::new();
    for item in cart {
        let Some(product_id) = item.get("product_id").and_then(Value::as_str) else {
            continue;
        };
        let Some(quantity) = item.get("quantity").and_then(Value::as_i64) else {
            continue;
        };
        if quantity <= 0 {
            continue;
        }
        let per_unit_micros = if let Some(pack) =
            state.billing_catalog.credit_pack_for_product(product_id)
        {
            purchased_pack_ids.push(pack.id.as_str());
            pack.credits_micros
        } else if let Some(amount_cents) = state
            .app_db
            .credit_amount_of_product(product_id)
            .await
            .map_err(|error| error.to_string())?
        {
            purchased_custom_cents.push(amount_cents);
            amount_cents
                .checked_mul(10_000)
                .ok_or_else(|| "credit purchase amount overflowed".to_owned())?
        } else {
            tracing::warn!(
                payment_id,
                product_id,
                "payment product is not a configured credit product; skipping"
            );
            continue;
        };
        let delta = per_unit_micros
            .checked_mul(quantity)
            .ok_or_else(|| "credit purchase amount overflowed".to_owned())?;
        purchases.push((delta, product_id.to_owned()));
    }
    if let Some(expected) = data
        .pointer("/metadata/argus_credit_pack_id")
        .and_then(Value::as_str)
    {
        if !purchased_pack_ids.contains(&expected) {
            return Err(format!(
                "payment `{payment_id}` was for credit pack `{expected}` but no configured credit product matched"
            ));
        }
    }
    if let Some(expected_cents) = data
        .pointer("/metadata/argus_credit_amount_cents")
        .and_then(Value::as_i64)
    {
        if !purchased_custom_cents.contains(&expected_cents) {
            return Err(format!(
                "payment `{payment_id}` was for a custom credit amount but no matching credit product was found"
            ));
        }
    }
    if purchases.is_empty() {
        return Ok(());
    }
    let customer_id = data
        .pointer("/customer/customer_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "credit payment has no customer id".to_owned())?;
    let mapped_user = state
        .app_db
        .billing_user_of_customer(customer_id)
        .await
        .map_err(|error| error.to_string())?;
    let metadata_user = data
        .pointer("/metadata/argus_user_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    if let (Some(mapped), Some(metadata)) = (&mapped_user, metadata_user) {
        if mapped != metadata {
            return Err(
                "billing customer and payment metadata identify different users".to_owned(),
            );
        }
    }
    let user_id = mapped_user
        .or_else(|| data
            .pointer("/metadata/argus_user_id")
            .and_then(Value::as_str)
            .map(str::to_owned))
        .ok_or_else(|| "credit payment cannot be mapped to an Argus user".to_owned())?;
    state
        .app_db
        .upsert_billing_customer(&user_id, customer_id)
        .await
        .map_err(|error| error.to_string())?;
    for (delta, product_id) in purchases {
        let reference = format!("{payment_id}:{product_id}");
        state
            .app_db
            .add_credits(&user_id, delta, "purchase", &reference)
            .await
            .map_err(|error| error.to_string())?;
    }
    if let Err(error) = state.app_db.clear_user_credit_checkouts(&user_id).await {
        tracing::error!(%error, "billing credit checkout session cleanup failed");
    }
    Ok(())
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

    use super::{credit_product_name, custom_amount_cents, verify_webhook_at};

    #[test]
    fn accepts_custom_amounts_in_range() {
        assert_eq!(custom_amount_cents(1.0), Some(100));
        assert_eq!(custom_amount_cents(5.0), Some(500));
        assert_eq!(custom_amount_cents(5.5), Some(550));
        assert_eq!(custom_amount_cents(499.999), Some(50_000));
        assert_eq!(custom_amount_cents(500.0), Some(50_000));
    }

    #[test]
    fn rejects_custom_amounts_out_of_range() {
        assert_eq!(custom_amount_cents(0.0), None);
        assert_eq!(custom_amount_cents(0.99), None);
        assert_eq!(custom_amount_cents(500.01), None);
        assert_eq!(custom_amount_cents(f64::NAN), None);
        assert_eq!(custom_amount_cents(f64::INFINITY), None);
    }

    #[test]
    fn formats_credit_product_names() {
        assert_eq!(credit_product_name(500), "Argus Prepaid Credits $5");
        assert_eq!(credit_product_name(550), "Argus Prepaid Credits $5.50");
    }

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
