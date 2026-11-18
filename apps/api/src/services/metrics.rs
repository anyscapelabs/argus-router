use std::time::Duration;

use aws_sdk_cloudwatch::types::{MetricDatum, StandardUnit};

use crate::services::app_db::AppDb;

const NAMESPACE: &str = "Argus/Gateway";
const INTERVAL: Duration = Duration::from_secs(60);

pub fn spawn_user_gauge(app_db: AppDb, region: String) {
    tokio::spawn(async move {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_sdk_cloudwatch::config::Region::new(region))
            .load()
            .await;
        let client = aws_sdk_cloudwatch::Client::new(&config);
        let mut ticker = tokio::time::interval(INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            match app_db.count_users().await {
                Ok(count) => publish(&client, count).await,
                Err(error) => tracing::warn!(%error, "cloudwatch user count query failed"),
            }
        }
    });
}

async fn publish(client: &aws_sdk_cloudwatch::Client, count: i64) {
    #[allow(clippy::cast_precision_loss)]
    let datum = MetricDatum::builder()
        .metric_name("Users")
        .value(count as f64)
        .unit(StandardUnit::Count)
        .build();
    if let Err(error) = client
        .put_metric_data()
        .namespace(NAMESPACE)
        .metric_data(datum)
        .send()
        .await
    {
        tracing::warn!(%error, "cloudwatch user count publish failed");
    }
}
