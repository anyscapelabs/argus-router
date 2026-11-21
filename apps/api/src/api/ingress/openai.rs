use serde_json::{Value, json};

use crate::error::{ErrCode, Errx};

pub fn error_body(item: &Errx) -> Value {
    json!({
        "error": {
            "message": item.public_message(),
            "type": error_type(item.code),
            "code": item.code.as_str(),
            "param": Value::Null,
        }
    })
}

fn error_type(code: ErrCode) -> &'static str {
    match code {
        ErrCode::InvalidRequest => "invalid_request_error",
        ErrCode::Unauthorized => "authentication_error",
        ErrCode::Forbidden => "permission_error",
        ErrCode::NotFound => "invalid_request_error",
        ErrCode::RateLimited
        | ErrCode::DailyLimitReached
        | ErrCode::WeeklyLimitReached
        | ErrCode::MonthlyLimitReached => "rate_limit_error",
        ErrCode::Overloaded => "overloaded_error",
        ErrCode::Upstream | ErrCode::Network | ErrCode::Timeout => "api_error",
        ErrCode::ProviderUnconfigured => "api_error",
    }
}

pub fn models_list(entries: Vec<Value>) -> Value {
    json!({
        "object": "list",
        "data": entries,
    })
}

pub fn error_response(item: Errx) -> axum::response::Response {
    use axum::Json;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    let status = StatusCode::from_u16(item.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(error_body(&item))).into_response()
}
