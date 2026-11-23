use chrono::{DateTime, Datelike, Duration, Months, NaiveDate, TimeZone, Utc};
use sqlx::Row;

use super::AppDb;

const WEEK_SECONDS: i64 = 7 * 24 * 60 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsagePeriod {
    pub monthly_start: DateTime<Utc>,
    pub monthly_reset: DateTime<Utc>,
    pub weekly_start: DateTime<Utc>,
    pub weekly_reset: DateTime<Utc>,
}

impl AppDb {
    pub async fn usage_period(&self, user_id: &str) -> Result<UsagePeriod, sqlx::Error> {
        load_usage_period(&self.pool, user_id, Utc::now()).await
    }
}

pub(crate) async fn load_usage_period<'e, E>(
    executor: E,
    user_id: &str,
    now: DateTime<Utc>,
) -> Result<UsagePeriod, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let row = sqlx::query(
        "SELECT u.created_at AS created_at,
                (SELECT s.next_billing_date
                   FROM billing_subscriptions s
                  WHERE s.user_id = u.id
                    AND s.next_billing_date IS NOT NULL
                    AND (s.status IN ('active', 'past_due')
                         OR (s.status = 'cancelled' AND s.cancel_at_next_billing_date))
                  ORDER BY s.updated_at DESC
                  LIMIT 1) AS next_billing_date
           FROM users u
          WHERE u.id = $1",
    )
    .bind(user_id)
    .fetch_optional(executor)
    .await?;

    let created_at = row
        .as_ref()
        .and_then(|row| row.try_get::<DateTime<Utc>, _>("created_at").ok());
    let next_billing = row
        .as_ref()
        .and_then(|row| row.try_get::<Option<String>, _>("next_billing_date").ok())
        .flatten()
        .and_then(|raw| parse_billing_date(&raw));

    Ok(usage_period_at(now, created_at, next_billing))
}

fn parse_billing_date(raw: &str) -> Option<DateTime<Utc>> {
    let raw = raw.trim();
    if let Ok(value) = DateTime::parse_from_rfc3339(raw) {
        return Some(value.with_timezone(&Utc));
    }
    if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return date.and_hms_opt(0, 0, 0).map(|value| Utc.from_utc_datetime(&value));
    }
    for format in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(value) = chrono::NaiveDateTime::parse_from_str(raw, format) {
            return Some(Utc.from_utc_datetime(&value));
        }
    }
    None
}

pub fn usage_period_at(
    now: DateTime<Utc>,
    created_at: Option<DateTime<Utc>>,
    next_billing: Option<DateTime<Utc>>,
) -> UsagePeriod {
    let (monthly_start, monthly_reset) = match (next_billing, created_at) {
        (Some(next_billing), _) => billing_month(next_billing, now),
        (None, Some(created_at)) => anniversary_month(created_at, now),
        (None, None) => calendar_month(now),
    };
    let weekly_start = weekly_start(monthly_start, now);
    UsagePeriod {
        monthly_start,
        monthly_reset,
        weekly_start,
        weekly_reset: weekly_start + Duration::weeks(1),
    }
}

fn billing_month(mut reset: DateTime<Utc>, now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let mut guard = 0;
    while reset <= now && guard < 1_200 {
        reset = add_months(reset, 1);
        guard += 1;
    }
    (add_months(reset, -1), reset)
}

fn anniversary_month(anchor: DateTime<Utc>, now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let mut months = month_delta(anchor, now);
    while months > 0 && add_months(anchor, months) > now {
        months -= 1;
    }
    let start = add_months(anchor, months);
    (start, add_months(anchor, months + 1))
}

fn calendar_month(now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let start = Utc
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .unwrap_or(now);
    (start, add_months(start, 1))
}

fn weekly_start(monthly_start: DateTime<Utc>, now: DateTime<Utc>) -> DateTime<Utc> {
    let elapsed = now.signed_duration_since(monthly_start).num_seconds();
    if elapsed <= 0 {
        return monthly_start;
    }
    monthly_start + Duration::weeks(elapsed / WEEK_SECONDS)
}

fn month_delta(from: DateTime<Utc>, to: DateTime<Utc>) -> i32 {
    let months = (to.year() - from.year()) * 12 + to.month() as i32 - from.month() as i32;
    months.max(0)
}

fn add_months(value: DateTime<Utc>, months: i32) -> DateTime<Utc> {
    if months >= 0 {
        value
            .checked_add_months(Months::new(months as u32))
            .unwrap_or(value)
    } else {
        value
            .checked_sub_months(Months::new((-months) as u32))
            .unwrap_or(value)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::usage_period_at;

    fn at(year: i32, month: u32, day: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 12, 0, 0)
            .single()
            .unwrap()
    }

    fn midnight(year: i32, month: u32, day: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 0, 0, 0)
            .single()
            .unwrap()
    }

    #[test]
    fn free_account_uses_creation_anniversary() {
        let created = at(2026, 11, 20);
        let period = usage_period_at(at(2026, 11, 22), Some(created), None);
        assert_eq!(period.monthly_start, created);
        assert_eq!(period.monthly_reset, at(2026, 12, 20));
        assert_eq!(period.weekly_start, created);
        assert_eq!(period.weekly_reset, at(2026, 11, 27));
    }

    #[test]
    fn subscriber_uses_next_billing_date() {
        let created = at(2026, 10, 5);
        let period = usage_period_at(at(2026, 11, 22), Some(created), Some(at(2026, 12, 20)));
        assert_eq!(period.monthly_start, at(2026, 11, 20));
        assert_eq!(period.monthly_reset, at(2026, 12, 20));
        assert_eq!(period.weekly_start, at(2026, 11, 20));
        assert_eq!(period.weekly_reset, at(2026, 11, 27));
    }

    #[test]
    fn period_rolls_forward_on_reset() {
        let created = at(2026, 11, 20);
        let period = usage_period_at(at(2026, 12, 20), Some(created), None);
        assert_eq!(period.monthly_start, at(2026, 12, 20));
        assert_eq!(period.monthly_reset, at(2027, 1, 20));
        assert_eq!(period.weekly_start, at(2026, 12, 20));
        assert_eq!(period.weekly_reset, at(2026, 12, 27));
    }

    #[test]
    fn stale_billing_date_rolls_forward() {
        let period = usage_period_at(at(2026, 11, 22), None, Some(at(2026, 10, 20)));
        assert_eq!(period.monthly_start, at(2026, 11, 20));
        assert_eq!(period.monthly_reset, at(2026, 12, 20));
    }

    #[test]
    fn short_month_clamps_anniversary_day() {
        let created = at(2026, 1, 31);
        let period = usage_period_at(at(2026, 2, 10), Some(created), None);
        assert_eq!(period.monthly_start, created);
        assert_eq!(period.monthly_reset, at(2026, 2, 28));
    }

    #[test]
    fn calendar_fallback_uses_month_start() {
        let period = usage_period_at(at(2026, 11, 22), None, None);
        assert_eq!(period.monthly_start, midnight(2026, 11, 1));
        assert_eq!(period.monthly_reset, midnight(2026, 12, 1));
        assert_eq!(period.weekly_start, midnight(2026, 11, 22));
        assert_eq!(period.weekly_reset, midnight(2026, 11, 29));
    }
}
