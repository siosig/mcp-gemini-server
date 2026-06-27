//! Typed errors for Gemini API interactions plus the user-facing classifier.
//!
//! Library modules return [`GeminiError`]; tool handlers convert it into a clear,
//! English message via [`GeminiError::to_user_message`] (the Rust analogue of the
//! original `handleApiError`).

/// Errors raised while talking to the Gemini REST API or handling local I/O.
#[derive(thiserror::Error, Debug)]
pub enum GeminiError {
    #[error("GEMINI_API_KEY environment variable is not set")]
    MissingApiKey,

    #[error("HTTP {status}: {message}")]
    Http { status: u16, message: String },

    #[error("Request timed out after {}s", timeout_ms / 1000)]
    Timeout { timeout_ms: u64 },

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Response deserialization failed: {0}")]
    Deserialize(#[from] serde_json::Error),

    #[error("File system error: {0}")]
    Io(#[from] std::io::Error),
}

impl GeminiError {
    /// Short tag used in telemetry (`error_type`).
    pub fn error_type(&self) -> String {
        match self {
            GeminiError::MissingApiKey => "missing_api_key".to_string(),
            GeminiError::Http { status, .. } => format!("http_{status}"),
            GeminiError::Timeout { .. } => "timeout".to_string(),
            GeminiError::Network(_) => "network".to_string(),
            GeminiError::Deserialize(_) => "deserialize".to_string(),
            GeminiError::Io(_) => "io".to_string(),
        }
    }

    /// Translate into an explanatory English message handed back to the LLM.
    ///
    /// Mirrors the status-code switch of the original `handleApiError`.
    pub fn to_user_message(&self) -> String {
        match self {
            GeminiError::Http { status: 400, message } => {
                format!("Error: The request is invalid. Please check the parameters. ({message})")
            }
            GeminiError::Http { status: 401, .. } => {
                "Error: Authentication failed. Please check the API key.".to_string()
            }
            GeminiError::Http { status: 403, .. } => {
                "Error: Access denied. Please check the credentials or scopes.".to_string()
            }
            GeminiError::Http { status: 404, .. } => {
                "Error: Resource not found. Please check the ID or path.".to_string()
            }
            GeminiError::Http { status: 429, .. } => {
                "Error: Rate limit reached. Please wait a moment and try again.".to_string()
            }
            GeminiError::Http { status: status @ (500 | 502 | 503), .. } => {
                format!("Error: A server error occurred (status: {status}). Please wait a moment and try again.")
            }
            GeminiError::Timeout { .. } => {
                format!("Error: The request timed out. The operation may be too complex. ({self})")
            }
            GeminiError::MissingApiKey => "Error: Authentication failed. Please check the API key.".to_string(),
            other => format!("Error: {other}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_http_status_to_user_message() {
        let e = GeminiError::Http { status: 401, message: "x".into() };
        assert!(e.to_user_message().contains("API key"));
        let e = GeminiError::Http { status: 429, message: "x".into() };
        assert!(e.to_user_message().contains("Rate limit"));
        let e = GeminiError::Http { status: 503, message: "x".into() };
        assert!(e.to_user_message().contains("status: 503"));
    }

    #[test]
    fn timeout_message_mentions_timed_out() {
        let e = GeminiError::Timeout { timeout_ms: 360_000 };
        assert!(e.to_user_message().contains("timed out"));
        assert_eq!(e.error_type(), "timeout");
    }

    #[test]
    fn http_error_type_includes_status() {
        let e = GeminiError::Http { status: 404, message: "x".into() };
        assert_eq!(e.error_type(), "http_404");
    }
}
