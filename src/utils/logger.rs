//! Structured logging to **stderr only** (stdout is reserved for the MCP JSON-RPC
//! stream). Replaces the original `pino` setup; only error logs are emitted
//! (success access logs / pricing were removed in spec 021).

use crate::utils::errors::GeminiError;

/// Initialize the global tracing subscriber: JSON formatter, stderr writer, level
/// taken from `LOG_LEVEL` (default `info`). Idempotent-safe: a second call is a no-op
/// because `try_init` returns an error that we deliberately ignore.
pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let filter = EnvFilter::try_new(&level).unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .json()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .try_init();
}

/// Emit a structured error log for a failed tool invocation.
pub fn log_error(
    tool_name: &str,
    model: &str,
    duration_ms: f64,
    error: &GeminiError,
    thinking_level: &str,
) {
    let error_type = error.error_type();
    let error_message = error.to_string();
    tracing::error!(
        log_type = "mcp_error",
        tool_name,
        model,
        status = "error",
        duration_ms = (duration_ms * 1000.0).round() / 1000.0,
        thinking_level,
        error_type = %error_type,
        error_message = %error_message,
        "[{tool_name}] {error_type}: {error_message}"
    );
}
