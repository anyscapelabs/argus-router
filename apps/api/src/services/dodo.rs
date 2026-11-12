use std::fmt;

use reqwest::{RequestBuilder, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

const EMBEDDED_PRODUCTS: &str = include_str!("../../billing/products.json");

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingCatalog {
    pub subscriptions: Vec<SubscriptionProduct>,
    pub credit_packs: Vec<CreditPack>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionProduct {
    pub plan_id: String,
    pub product_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditPack {
    pub id: String,
    pub label: String,
    pub price_usd: i64,
    pub credits_micros: i64,
    #[serde(skip_serializing)]
    pub product_id: String,
}

impl BillingCatalog {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let body = match std::env::var("BILLING_PRODUCTS_PATH") {
            Ok(path) if !path.trim().is_empty() => std::fs::read_to_string(path.trim())?,
            _ => EMBEDDED_PRODUCTS.to_owned(),
        };
        Ok(serde_json::from_str(&body)?)
    }

    pub fn subscription_product(&self, plan_id: &str) -> Option<&str> {
        self.subscriptions
            .iter()
            .find(|item| item.plan_id == plan_id && !item.product_id.is_empty())
            .map(|item| item.product_id.as_str())
    }

    pub fn plan_for_product(&self, product_id: &str) -> Option<&str> {
        self.subscriptions
            .iter()
            .find(|item| item.product_id == product_id)
            .map(|item| item.plan_id.as_str())
    }

    pub fn credit_pack(&self, pack_id: &str) -> Option<&CreditPack> {
        self.credit_packs.iter().find(|item| item.id == pack_id)
    }

    pub fn credit_pack_for_product(&self, product_id: &str) -> Option<&CreditPack> {
        self.credit_packs
            .iter()
            .find(|item| item.product_id == product_id)
    }
}

#[derive(Clone)]
pub struct DodoClient {
    http: reqwest::Client,
    api_key: String,
    base_url: &'static str,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DodoSubscription {
    pub subscription_id: String,
    pub status: String,
    pub product_id: String,
    pub next_billing_date: Option<String>,
    pub past_due_ends_at: Option<String>,
    pub customer: DodoCustomer,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DodoCustomer {
    pub customer_id: String,
}

#[derive(Debug)]
pub struct DodoError {
    status: Option<StatusCode>,
    message: String,
}

impl DodoError {
    fn request(error: reqwest::Error) -> Self {
        Self {
            status: error.status(),
            message: error.to_string(),
        }
    }

    fn response(status: StatusCode, body: String) -> Self {
        Self {
            status: Some(status),
            message: body.chars().take(1000).collect(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self {
            status: None,
            message: message.into(),
        }
    }

    pub fn status(&self) -> Option<StatusCode> {
        self.status
    }
}

impl fmt::Display for DodoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for DodoError {}

impl DodoClient {
    pub fn new(api_key: String, test_mode: bool) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key,
            base_url: if test_mode {
                "https://test.dodopayments.com"
            } else {
                "https://live.dodopayments.com"
            },
        }
    }

    async fn send<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T, DodoError> {
        let response = request
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(DodoError::request)?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(DodoError::response(status, body));
        }
        response.json().await.map_err(DodoError::request)
    }

    pub async fn create_customer(
        &self,
        user_id: &str,
        name: &str,
        email: &str,
    ) -> Result<String, DodoError> {
        #[derive(Deserialize)]
        struct Response {
            customer_id: String,
        }

        let response: Response = self
            .send(
                self.http
                    .post(format!("{}/customers", self.base_url))
                    .header("Idempotency-Key", format!("argus-customer-{user_id}"))
                    .json(&json!({
                        "name": name,
                        "email": email,
                        "metadata": { "argus_user_id": user_id }
                    })),
            )
            .await?;
        Ok(response.customer_id)
    }

    pub async fn create_checkout(
        &self,
        customer_id: &str,
        product_id: &str,
        return_url: &str,
        metadata: Value,
    ) -> Result<String, DodoError> {
        #[derive(Deserialize)]
        struct Response {
            checkout_url: Option<String>,
        }

        let response: Response = self
            .send(
                self.http
                    .post(format!("{}/checkouts", self.base_url))
                    .json(&json!({
                        "product_cart": [{ "product_id": product_id, "quantity": 1 }],
                        "customer": { "customer_id": customer_id },
                        "metadata": metadata,
                        "return_url": return_url
                    })),
            )
            .await?;
        response
            .checkout_url
            .filter(|url| !url.is_empty())
            .ok_or_else(|| DodoError::invalid("Dodo Payments returned no checkout URL"))
    }

    pub async fn create_portal(
        &self,
        customer_id: &str,
        return_url: &str,
    ) -> Result<String, DodoError> {
        #[derive(Deserialize)]
        struct Response {
            link: String,
        }

        let response: Response = self
            .send(
                self.http
                    .post(format!(
                        "{}/customers/{}/customer-portal/session",
                        self.base_url, customer_id
                    ))
                    .query(&[("return_url", return_url)]),
            )
            .await?;
        Ok(response.link)
    }

    pub async fn subscription(&self, subscription_id: &str) -> Result<DodoSubscription, DodoError> {
        self.send(self.http.get(format!(
            "{}/subscriptions/{}",
            self.base_url, subscription_id
        )))
        .await
    }
}
