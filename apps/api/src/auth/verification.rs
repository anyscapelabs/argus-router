use better_auth::plugins::{MagicLinkMessage, SendMagicLink, SendVerificationEmail};
use better_auth_api::plugins::user_management::SendDeleteAccountVerification;
use better_auth::prelude::AuthRequest;
use tracing::instrument;

use crate::config::Settings;
use crate::services::email::ResendEmail;

const EMAIL_FONT: &str =
    "-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif";

fn branded_email_shell(
    dashboard_url: &str,
    heading: &str,
    body: &str,
    button_label: &str,
    button_url: &str,
    helper: &str,
) -> String {
    format!(
        r#"<body style="margin:0;padding:0;background-color:#f6f6f6;">
<div style="background-color:#f6f6f6;padding:48px 24px;">
  <table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0"><tr><td align="center">
    <table role="presentation" width="560" cellpadding="0" cellspacing="0" border="0" style="max-width:560px;width:100%;">
      <tr><td align="center" style="padding-bottom:24px;">
        <img src="{dashboard_url}/icon-black.png" width="35" height="28" alt="Anyscape Gateway" style="display:block;border:0;" />
      </td></tr>
      <tr><td align="center" style="padding-bottom:24px;font-family:{EMAIL_FONT};font-size:24px;line-height:32px;font-weight:700;color:#171717;">{heading}</td></tr>
      <tr><td>
        <div style="background-color:#ffffff;border:1px solid #e5e5e5;border-radius:12px;padding:32px 24px;text-align:center;font-family:{EMAIL_FONT};">
          <p style="margin:0 0 24px;font-size:14px;line-height:22px;color:#404040;">{body}</p>
          <a href="{button_url}" style="display:inline-block;background-color:#f97316;color:#0a0a0a;font-size:14px;font-weight:600;line-height:20px;padding:10px 20px;border-radius:8px;text-decoration:none;">{button_label}</a>
          <p style="margin:16px 0 0;font-size:12px;line-height:18px;color:#737373;">{helper}</p>
        </div>
      </td></tr>
      <tr><td align="center" style="padding-top:24px;font-family:{EMAIL_FONT};font-size:12px;line-height:18px;color:#737373;">
        Any questions? Just reply to this email &mdash; we&apos;re happy to help.
      </td></tr>
    </table>
  </td></tr></table>
</div>
<div style="background-color:#0a0a0a;padding:20px;">
  <table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0"><tr><td align="center">
    <img src="{dashboard_url}/icon-white.png" width="20" height="16" alt="" style="display:block;border:0;" />
  </td></tr></table>
</div>
</body>"#,
    )
}

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

        let html = branded_email_shell(
            &self.dashboard_url,
            "Hey! Welcome on board!",
            "Please click the button below to confirm your email address and finish setting up your account.",
            "Confirm my email",
            &dashboard_url,
            "This link expires in one hour. If you didn&#39;t create an account, you can safely ignore this email.",
        );
        let _ = url;

        self.email
            .send(&user_email, "Confirm your email — Argus Agent", &html)
            .await
            .map_err(|error| better_auth::AuthError::internal(error.to_string()))?;
        Ok(())
    }
}

pub struct ResendMagicLinkSender {
    email: ResendEmail,
    dashboard_url: String,
}

impl ResendMagicLinkSender {
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
impl SendMagicLink for ResendMagicLinkSender {
    #[instrument(skip_all, fields(email))]
    async fn send(&self, message: &MagicLinkMessage) -> better_auth::AuthResult<()> {
        let html = branded_email_shell(
            &self.dashboard_url,
            "Sign in to Argus",
            "Click the button below to sign in to your account. If you do not have an account yet, we will create one for you.",
            "Sign in to Argus",
            &message.url,
            "This link expires in 10 minutes and can be used once. If you didn&#39;t request it, you can safely ignore this email.",
        );

        self.email
            .send(&message.email, "Your sign-in link — Argus Agent", &html)
            .await
            .map_err(|error| better_auth::AuthError::internal(error.to_string()))?;
        Ok(())
    }
}

pub struct ResendDeleteAccountSender {
    email: ResendEmail,
    dashboard_url: String,
}

impl ResendDeleteAccountSender {
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
impl SendDeleteAccountVerification for ResendDeleteAccountSender {
    #[instrument(skip_all, fields(token))]
    async fn send(
        &self,
        user: &better_auth::FieldValue,
        _url: &str,
        token: &str,
        _request: Option<&AuthRequest>,
    ) -> better_auth::AuthResult<()> {
        let user_email = user
            .json()
            .ok()
            .and_then(|json| json.as_ref().and_then(|value| value.get("email").cloned()))
            .and_then(|email| email.as_str().map(str::to_owned))
            .ok_or_else(|| {
                better_auth::AuthError::internal("delete account recipient has no email address")
            })?;

        let dashboard_url = format!("{}/auth/delete-account?token={}", self.dashboard_url, token);

        let html = branded_email_shell(
            &self.dashboard_url,
            "Delete your account?",
            "We received a request to permanently delete your account. Once you confirm, your account and all its data are removed and this can&#39;t be undone.",
            "Delete my account",
            &dashboard_url,
            "This link expires in one hour and must be opened while signed in. If you didn&#39;t request this, you can safely ignore this email.",
        );

        self.email
            .send(
                &user_email,
                "Confirm account deletion — Argus Agent",
                &html,
            )
            .await
            .map_err(|error| better_auth::AuthError::internal(error.to_string()))?;
        Ok(())
    }
}
