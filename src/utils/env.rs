//! Startup-time environment configuration and Fail-Fast validation.
//!
//! [`validate_env`] terminates startup (via the caller) when a required or enum value
//! is invalid. Free-form strings such as model names are not checked here (they are
//! validated on the API side).

use crate::utils::errors::GeminiError;

const THINKING_LEVELS: &[&str] = &["minimal", "low", "medium", "high"];
const SERVICE_TIERS: &[&str] = &["flex", "priority", "standard"];

const THINKING_LEVEL_VARS: &[&str] = &[
    "GEMINI_THINKING_LEVEL",
    "GEMINI_AGENT_THINKING_LEVEL",
    "GEMINI_VISION_THINKING_LEVEL",
    "GEMINI_CODE_THINKING_LEVEL",
    "GEMINI_IMAGE_THINKING_LEVEL",
    "GEMINI_TEAM_THINKING_LEVEL",
];

/// Runtime configuration resolved once at startup. Immutable thereafter.
///
/// `IMAGEN_OUTPUT_DIR` and `LOG_LEVEL` are read directly where needed
/// (`resolve_output_dir` / `init_tracing`) rather than stored here.
#[derive(Debug, Clone)]
pub struct EnvConfig {
    pub gemini_api_key: String,
    /// Standard / priority tier timeout in milliseconds.
    pub timeout_ms: u64,
    /// Flex tier timeout in milliseconds.
    pub flex_timeout_ms: u64,
}

impl EnvConfig {
    /// Build from the current process environment. Requires `GEMINI_API_KEY`.
    pub fn from_env() -> Result<Self, GeminiError> {
        let gemini_api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
        if gemini_api_key.is_empty() {
            return Err(GeminiError::MissingApiKey);
        }

        // GEMINI_TIMEOUT is expressed in seconds (default 360s); flex is fixed at 900s.
        let timeout_secs = std::env::var("GEMINI_TIMEOUT")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(360);

        Ok(EnvConfig {
            gemini_api_key,
            timeout_ms: timeout_secs.saturating_mul(1000),
            flex_timeout_ms: 900_000,
        })
    }
}

/// Fail-Fast validation of the environment. Returns a combined error message listing
/// every problem, or `Ok(())` when valid.
pub fn validate_env() -> Result<(), String> {
    let mut errors: Vec<String> = Vec::new();

    match std::env::var("GEMINI_API_KEY") {
        Ok(v) if !v.is_empty() => {}
        _ => errors.push("GEMINI_API_KEY is required".to_string()),
    }

    for key in THINKING_LEVEL_VARS {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() && !THINKING_LEVELS.contains(&v.as_str()) {
                errors.push(format!(
                    "{key} must be one of minimal|low|medium|high (got \"{v}\")"
                ));
            }
        }
    }

    if let Ok(v) = std::env::var("GEMINI_SERVICE_TIER") {
        if !v.is_empty() && !SERVICE_TIERS.contains(&v.as_str()) {
            errors.push(format!(
                "GEMINI_SERVICE_TIER must be one of flex|priority|standard (got \"{v}\")"
            ));
        }
    }

    if let Ok(v) = std::env::var("GEMINI_TIMEOUT") {
        if !v.trim().is_empty() && v.trim().parse::<f64>().is_err() {
            errors.push(format!("GEMINI_TIMEOUT must be a numeric string (got \"{v}\")"));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
