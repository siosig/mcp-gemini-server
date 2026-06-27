//! Cross-cutting concerns for API calls: timeout, retry with backoff, and
//! error-logging instrumentation. The Rust analogue of the original
//! `withAbortableTimeout` / `withFetchRetry` / `withTelemetry` helpers.

use std::future::Future;
use std::time::Instant;

use tokio::time::{sleep, timeout, Duration};

use crate::utils::errors::GeminiError;
use crate::utils::logger::log_error;

/// Default retry count for transient network errors.
pub const MAX_RETRIES: u32 = 3;
/// Exponential-backoff base (delay = `BASE_RETRY_DELAY_MS * 2^attempt`).
pub const BASE_RETRY_DELAY_MS: u64 = 1_000;

/// Substrings that mark an otherwise-opaque error as transient (retryable).
const TRANSIENT_ERROR_PATTERNS: &[&str] = &[
    "connection reset",
    "connection refused",
    "timed out",
    "timeout",
    "network error",
    "fetch failed",
];

/// Run `fut` with an overall timeout. On expiry, returns [`GeminiError::Timeout`].
pub async fn with_timeout<F, T>(fut: F, ms: u64) -> Result<T, GeminiError>
where
    F: Future<Output = Result<T, GeminiError>>,
{
    match timeout(Duration::from_millis(ms), fut).await {
        Ok(inner) => inner,
        Err(_) => Err(GeminiError::Timeout { timeout_ms: ms }),
    }
}

/// Whether an error is transient and therefore worth retrying.
fn is_transient(error: &GeminiError) -> bool {
    match error {
        GeminiError::Timeout { .. } => true,
        GeminiError::Http { status, .. } => *status == 429 || *status >= 500,
        GeminiError::Network(re) => {
            if re.is_timeout() || re.is_connect() {
                return true;
            }
            let msg = re.to_string().to_lowercase();
            TRANSIENT_ERROR_PATTERNS.iter().any(|p| msg.contains(p))
        }
        _ => false,
    }
}

/// Retry `make_fut` with exponential backoff on transient errors.
///
/// `make_fut` is invoked once per attempt so the underlying request can be rebuilt.
pub async fn with_retry<F, Fut, T>(
    mut make_fut: F,
    max_retries: u32,
    base_delay_ms: u64,
) -> Result<T, GeminiError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, GeminiError>>,
{
    let mut attempt: u32 = 0;
    loop {
        match make_fut().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                if !is_transient(&error) || attempt == max_retries {
                    return Err(error);
                }
                let delay = base_delay_ms.saturating_mul(2u64.saturating_pow(attempt));
                tracing::warn!(
                    attempt = attempt + 1,
                    max_retries,
                    delay_ms = delay,
                    error = %error,
                    "[fetchRetry] transient network error, retrying after {delay}ms ({}/{})",
                    attempt + 1,
                    max_retries
                );
                sleep(Duration::from_millis(delay)).await;
                attempt += 1;
            }
        }
    }
}

/// Options describing the call being instrumented.
pub struct TelemetryOptions<'a> {
    pub tool_name: &'a str,
    pub model: &'a str,
    pub thinking_level: &'a str,
}

/// Measure elapsed time and log on failure. Returns `(result, duration_ms)`.
pub async fn with_telemetry<F, T>(
    opts: TelemetryOptions<'_>,
    fut: F,
) -> Result<(T, f64), GeminiError>
where
    F: Future<Output = Result<T, GeminiError>>,
{
    let start = Instant::now();
    match fut.await {
        Ok(value) => Ok((value, start.elapsed().as_secs_f64() * 1000.0)),
        Err(error) => {
            let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
            log_error(opts.tool_name, opts.model, duration_ms, &error, opts.thinking_level);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[tokio::test]
    async fn with_timeout_returns_value_on_normal_completion() {
        let r = with_timeout(async { Ok::<_, GeminiError>(42) }, 1000).await;
        assert_eq!(r.unwrap(), 42);
    }

    #[tokio::test]
    async fn with_timeout_returns_timeout_error_when_exceeded() {
        let slow = async {
            sleep(Duration::from_millis(200)).await;
            Ok::<_, GeminiError>(1)
        };
        let r = with_timeout(slow, 50).await;
        assert!(matches!(r, Err(GeminiError::Timeout { timeout_ms: 50 })));
    }

    #[tokio::test]
    async fn with_retry_does_not_retry_non_transient_errors() {
        let calls = Cell::new(0);
        let r: Result<i32, GeminiError> = with_retry(
            || {
                calls.set(calls.get() + 1);
                async { Err(GeminiError::Http { status: 400, message: "bad".into() }) }
            },
            3,
            1,
        )
        .await;
        assert!(r.is_err());
        assert_eq!(calls.get(), 1, "400 is non-transient: no retries");
    }

    #[tokio::test]
    async fn with_retry_retries_transient_then_succeeds() {
        let calls = Cell::new(0);
        let r: Result<i32, GeminiError> = with_retry(
            || {
                calls.set(calls.get() + 1);
                let n = calls.get();
                async move {
                    if n < 3 {
                        Err(GeminiError::Timeout { timeout_ms: 1 })
                    } else {
                        Ok(7)
                    }
                }
            },
            3,
            1,
        )
        .await;
        assert_eq!(r.unwrap(), 7);
        assert_eq!(calls.get(), 3);
    }

    #[tokio::test]
    async fn with_telemetry_passes_through_value_and_measures() {
        let (v, dur) = with_telemetry(
            TelemetryOptions { tool_name: "t", model: "m", thinking_level: "" },
            async { Ok::<_, GeminiError>("ok") },
        )
        .await
        .unwrap();
        assert_eq!(v, "ok");
        assert!(dur >= 0.0);
    }
}
