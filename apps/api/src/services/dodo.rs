use std::fmt;

use reqwest::{RequestBuilder, StatusCode};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

const EMBEDDED_PRODUCTS: &str = include_str!("../../billing/products.json");

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingCatalog {
    pub subscriptions: Vec<SubscriptionProduct>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionProduct {
    pub plan_id: String,
    #[serde(default)]
    pub price_usd: i64,
    #[serde(default)]
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

    pub fn subscription_entry(&self, plan_id: &str) -> Option<&SubscriptionProduct> {
        self.subscriptions
            .iter()
            .find(|item| item.plan_id == plan_id)
    }

    pub fn plan_for_product(&self, product_id: &str) -> Option<&str> {
        self.subscriptions
            .iter()
            .find(|item| item.product_id == product_id)
            .map(|item| item.plan_id.as_str())
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
    #[serde(default)]
    pub cancel_at_next_billing_date: bool,
    pub cancelled_at: Option<String>,
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

    pub fn is_missing_customer(&self) -> bool {
        self.status == Some(StatusCode::NOT_FOUND)
            && self.message.to_lowercase().contains("customer")
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
        idempotency_key: &str,
    ) -> Result<String, DodoError> {
        #[derive(Deserialize)]
        struct Response {
            checkout_url: Option<String>,
        }

        let response: Response = self
            .send(
                self.http
                    .post(format!("{}/checkouts", self.base_url))
                    .header("Idempotency-Key", idempotency_key)
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


    pub async fn create_subscription_product(
        &self,
        plan_id: &str,
        name: &str,
        price_cents: i64,
        metadata: Value,
    ) -> Result<String, DodoError> {
        #[derive(Deserialize)]
        struct Response {
            product_id: String,
        }

        let response: Response = self
            .send(
                self.http
                    .post(format!("{}/products", self.base_url))
                    .header("Idempotency-Key", format!("argus-plan-product-{plan_id}"))
                    .json(&json!({
                        "name": name,
                        "price": {
                            "currency": "USD",
                            "price": price_cents,
                            "payment_frequency_count": 1,
                            "payment_frequency_interval": "Monthly",
                            "type": "recurring_price"
                        },
                        "tax_category": "digital_products",
                        "metadata": metadata
                    })),
            )
            .await?;
        Ok(response.product_id)
    }



    pub async fn product_exists(&self, product_id: &str) -> Result<bool, DodoError> {
        let response = self
            .http
            .get(format!("{}/products/{}", self.base_url, product_id))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(DodoError::request)?;
        let status = response.status();
        if status.is_success() {
            return Ok(true);
        }
        if status == StatusCode::NOT_FOUND {
            return Ok(false);
        }
        Err(DodoError::response(
            status,
            response.text().await.unwrap_or_default(),
        ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_customer_404_is_recognised() {
        let error = DodoError::response(
            StatusCode::NOT_FOUND,
            r#"{"code":"NOT_FOUND","message":"Customer cus_123 not found"}"#.to_owned(),
        );
        assert!(error.is_missing_customer());
    }

    #[test]
    fn other_404_is_not_a_missing_customer() {
        let error = DodoError::response(
            StatusCode::NOT_FOUND,
            r#"{"code":"NOT_FOUND","message":"Product pdt_123 not found"}"#.to_owned(),
        );
        assert!(!error.is_missing_customer());
    }

    #[test]
    fn customer_error_with_other_status_is_not_missing() {
        let error =
            DodoError::response(StatusCode::BAD_REQUEST, "Customer is invalid".to_owned());
        assert!(!error.is_missing_customer());
    }
}
