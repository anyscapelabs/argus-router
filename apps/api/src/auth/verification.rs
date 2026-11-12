use better_auth::plugins::SendVerificationEmail;
use tracing::instrument;

use crate::config::Settings;
use crate::services::email::ResendEmail;

pub struct ResendVerificationSender {
    email: ResendEmail,
    dashboard_url: String,
}

impl ResendVerificationSender {
    pub fn new(settings: &Settings, client: ::reqwest::Client) -> Self {
        Self {
            email: ResendEmail {
                client,
                api_key: settings.resend_api_key.clone(),
                from: settings.resend_from.clone(),
            },
            dashboard_url: settings.dashboard_url.trim_end_matches('/').to_owned(),
        }
    }
}

#[async_trait::async_trait]
impl SendVerificationEmail for ResendVerificationSender {
    #[instrument(skip_all, fields(url))]
    async fn send(
        &self,
        user: &better_auth::FieldValue,
        url: &str,
        token: &str,
    ) -> better_auth::AuthResult<()> {
        let user_email = user
            .json()
            .ok()
            .and_then(|json| json.as_ref().and_then(|value| value.get("email").cloned()))
            .and_then(|email| email.as_str().map(str::to_owned))
            .ok_or_else(|| {
                better_auth::AuthError::internal("verification recipient has no email address")
            })?;

        let dashboard_url = format!("{}/auth/verify-email?token={}", self.dashboard_url, token);

        let html = format!(
            r#"<div style="font-family:sans-serif">
<h2>Verify your Argus Agent account</h2>
<p>Welcome aboard. Confirm your email address to activate your account.</p>
<p><a href="{dashboard_url}" style="background:#f97316;color:#000;padding:10px 20px;border-radius:6px;text-decoration:none;font-weight:600">Verify email</a></p>
<p style="color:#666;font-size:13px">This link expires in one hour. If you didn't create an account, you can ignore this email.</p>
</div>"#,
        );
        let _ = url;

        self.email
            .send(&user_email, "Verify your email — Argus Agent", &html)
            .await
            .map_err(|error| better_auth::AuthError::internal(error.to_string()))?;
        Ok(())
    }
}
