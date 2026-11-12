use tracing::{error, instrument};

#[derive(Clone)]
pub struct ResendEmail {
    pub client: ::reqwest::Client,
    pub api_key: Option<String>,
    pub from: String,
}

#[derive(serde::Serialize)]
struct ResendPayload<'a> {
    from: &'a str,
    to: [&'a str; 1],
    subject: &'a str,
    html: &'a str,
}

impl ResendEmail {
    pub async fn send(&self, to: &str, subject: &str, html: &str) -> Result<(), ::reqwest::Error> {
        match &self.api_key {
            Some(key) => {
                let payload = ResendPayload {
                    from: &self.from,
                    to: [to],
                    subject,
                    html,
                };
                let response = self
                    .client
                    .post("https://api.resend.com/emails")
                    .bearer_auth(key)
                    .json(&payload)
                    .send()
                    .await?;
                if let Err(reason) = response.error_for_status() {
                    error!(subject, "Resend rejected email delivery");
                    return Err(reason);
                }
                Ok(())
            }
            None => {
                self.log_dev(to, subject);
                Ok(())
            }
        }
    }

    #[instrument(skip_all)]
    fn log_dev(&self, to: &str, subject: &str) {
        tracing::info!(
            to,
            subject,
            "[EMAIL][dev] email delivery skipped (no RESEND_API_KEY)"
        );
    }
}
