#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrCode {
    InvalidRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    RateLimited,
    DailyLimitReached,
    WeeklyLimitReached,
    MonthlyLimitReached,
    Overloaded,
    Upstream,
    Network,
    ProviderUnconfigured,
    Timeout,
}

impl ErrCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrCode::InvalidRequest => "invalid_request_error",
            ErrCode::Unauthorized => "unauthorized",
            ErrCode::Forbidden => "forbidden",
            ErrCode::NotFound => "not_found",
            ErrCode::RateLimited => "rate_limit_error",
            ErrCode::DailyLimitReached => "daily_limit_reached",
            ErrCode::WeeklyLimitReached => "weekly_limit_reached",
            ErrCode::MonthlyLimitReached => "monthly_limit_reached",
            ErrCode::Overloaded => "overloaded_error",
            ErrCode::Upstream => "upstream_error",
            ErrCode::Network => "network_error",
            ErrCode::ProviderUnconfigured => "provider_unconfigured",
            ErrCode::Timeout => "timeout_error",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Errx {
    pub code: ErrCode,
    pub status: u16,
    pub message: String,
    pub provider: Option<String>,
    pub request_id: Option<String>,
    pub retry_after: Option<u64>,
}

impl Errx {
    pub fn new(code: ErrCode, status: u16, message: impl Into<String>) -> Self {
        Self {
            code,
            status,
            message: message.into(),
            provider: None,
            request_id: None,
            retry_after: None,
        }
    }

    pub fn with_provider(mut self, provider: &str) -> Self {
        self.provider = Some(provider.to_owned());
        self
    }

    pub fn with_retry_after(mut self, seconds: u64) -> Self {
        self.retry_after = Some(seconds);
        self
    }

    pub fn public_message(&self) -> String {
        match self.code {
            ErrCode::RateLimited
            | ErrCode::DailyLimitReached
            | ErrCode::WeeklyLimitReached
            | ErrCode::MonthlyLimitReached => self.message.clone(),
            _ => "An error has occurred. Try again later.".to_owned(),
        }
    }

    pub fn from_upstream_status(status: u16, message: impl Into<String>) -> Self {
        let code = match status {
            400 | 404 | 422 => ErrCode::InvalidRequest,
            401 => ErrCode::Unauthorized,
            403 => ErrCode::Forbidden,
            408 => ErrCode::Timeout,
            429 => ErrCode::RateLimited,
            409 => ErrCode::Upstream,
            500 | 502 | 503 | 504 => ErrCode::Upstream,
            _ => ErrCode::Upstream,
        };
        Self::new(code, status, message)
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrCode, Errx};

    #[test]
    fn masks_upstream_invalid_request_detail() {
        let local = Errx::new(ErrCode::InvalidRequest, 400, "messages are required");
        assert_eq!(
            local.public_message(),
            "An error has occurred. Try again later."
        );

        let upstream = Errx::new(
            ErrCode::InvalidRequest,
            400,
            "the upstream provider rejected the request: {secret detail}",
        )
        .with_provider("provider/model");
        assert_eq!(
            upstream.public_message(),
            "An error has occurred. Try again later."
        );
    }

    #[test]
    fn keeps_rate_limit_message() {
        let error = Errx::new(ErrCode::RateLimited, 429, "Try again in 30 seconds");
        assert_eq!(error.public_message(), "Try again in 30 seconds");
    }
}
