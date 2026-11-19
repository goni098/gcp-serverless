use std::time::Duration;

use error::R;

pub mod models;
pub mod repositories;

#[derive(Debug, Clone, Default)]
pub struct ConnectOptions {
    pub max_pool_size: Option<usize>,
    pub pool_wait_timeout: Option<Duration>,
    pub pool_create_timeout: Option<Duration>,
}

pub async fn connect(database_url: &str, options: Option<ConnectOptions>) -> R<toasty::Db> {
    let mut builder = toasty::Db::builder();

    if let Some(options) = options {
        if let Some(max_pool_size) = options.max_pool_size {
            builder.max_pool_size(max_pool_size);
        }
        if let Some(timeout) = options.pool_wait_timeout {
            builder.pool_wait_timeout(Some(timeout));
        }
        if let Some(timeout) = options.pool_create_timeout {
            builder.pool_create_timeout(Some(timeout));
        }
    }

    let db = builder.models(models::all()).connect(database_url).await?;

    Ok(db)
}

/// Startup retry settings. Keep `total_timeout` within the service's startup budget.
#[derive(Debug, Clone)]
pub struct RetryOptions {
    pub total_timeout: Duration,
    pub attempt_timeout: Duration,
    pub retry_delay: Duration,
}

impl Default for RetryOptions {
    fn default() -> Self {
        Self {
            total_timeout: Duration::from_secs(180),
            attempt_timeout: Duration::from_secs(10),
            retry_delay: Duration::from_secs(5),
        }
    }
}

/// Connect with bounded retries, including when an individual attempt stalls.
/// Useful during Cloud Run startup while Direct VPC egress becomes available.
/// `None` preserves Toasty's pool defaults and uses the default retry settings.
pub async fn connect_with_retry(
    database_url: &str,
    options: Option<ConnectOptions>,
    retry_options: Option<RetryOptions>,
) -> R<toasty::Db> {
    let retry = retry_options.unwrap_or_default();
    let mut last_failure = String::from("no connection attempt completed");

    tokio::time::timeout(retry.total_timeout, async {
        loop {
            match tokio::time::timeout(
                retry.attempt_timeout,
                connect(database_url, options.clone()),
            )
            .await
            {
                Ok(Ok(db)) => return db,
                Ok(Err(err)) => last_failure = err.to_string(),
                Err(_) => {
                    last_failure = format!(
                        "connection attempt timed out after {:?}",
                        retry.attempt_timeout
                    );
                }
            }
            tracing::warn!(
                retry_delay = ?retry.retry_delay,
                "Database unavailable during startup; retrying"
            );
            tokio::time::sleep(retry.retry_delay).await;
        }
    })
    .await
    .map_err(|_| {
        error::AppError::cust(format!(
            "database startup timed out after {:?}: {last_failure}",
            retry.total_timeout
        ))
    })
}
