//! Gemini Generative Language REST API client.
//!
//! Calls `POST /v1beta/models/{model}:generateContent` directly via `reqwest`,
//! layering timeout, retry, and telemetry. Also owns the per-tool default model /
//! thinking-level resolution and the thinking-config builder shared with the tools.

use std::sync::Arc;

use base64::Engine as _;

use crate::schemas::{ServiceTierValue, ThinkingLevel};
use crate::services::types::{
    Content, GenerateContentRequest, GenerateContentResponse, GenerationConfig, Part, SafetySetting,
    ThinkingConfig, Tool,
};
use crate::utils::diagnostics::{extract_response_diagnostics, ResponseDiagnostics};
use crate::utils::env::EnvConfig;
use crate::utils::errors::GeminiError;
use crate::utils::telemetry::{
    with_retry, with_telemetry, with_timeout, TelemetryOptions, BASE_RETRY_DELAY_MS, MAX_RETRIES,
};

const API_BASE: &str = "https://generativelanguage.googleapis.com";

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

// ==================== Per-tool default models ====================

pub fn default_model() -> String {
    env_or("GEMINI_MODEL", "gemini-flash-latest")
}
pub fn default_agent_model() -> String {
    env_or("GEMINI_AGENT_MODEL", "gemini-flash-latest")
}
pub fn default_search_model() -> String {
    env_or("GEMINI_SEARCH_MODEL", "gemini-flash-lite-latest")
}
pub fn default_vision_model() -> String {
    env_or("GEMINI_VISION_MODEL", "gemini-flash-lite-latest")
}
pub fn default_code_model() -> String {
    env_or("GEMINI_CODE_MODEL", "gemini-flash-lite-latest")
}
pub fn default_team_model() -> String {
    std::env::var("GEMINI_TEAM_MODEL").unwrap_or_else(|_| default_agent_model())
}

/// Fixed image model (Nano Banana 2 / Flash Image).
pub const IMAGE_MODEL: &str = "gemini-3.1-flash-image-preview";

// ==================== Per-tool default thinking levels ====================

pub fn default_thinking_level() -> ThinkingLevel {
    ThinkingLevel::from_env("GEMINI_THINKING_LEVEL", ThinkingLevel::Medium)
}
pub fn default_agent_thinking_level() -> ThinkingLevel {
    ThinkingLevel::from_env("GEMINI_AGENT_THINKING_LEVEL", ThinkingLevel::High)
}
pub fn default_vision_thinking_level() -> ThinkingLevel {
    ThinkingLevel::from_env("GEMINI_VISION_THINKING_LEVEL", ThinkingLevel::Medium)
}
pub fn default_code_thinking_level() -> ThinkingLevel {
    ThinkingLevel::from_env("GEMINI_CODE_THINKING_LEVEL", ThinkingLevel::Low)
}
pub fn default_image_thinking_level() -> ThinkingLevel {
    ThinkingLevel::from_env("GEMINI_IMAGE_THINKING_LEVEL", ThinkingLevel::Medium)
}
pub fn default_team_thinking_level() -> ThinkingLevel {
    ThinkingLevel::from_env("GEMINI_TEAM_THINKING_LEVEL", ThinkingLevel::High)
}

// ==================== Thinking config ====================

/// Three-state thinking selection (mirrors the SDK's `level | null | undefined`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThinkingSetting {
    /// Leave unset — defer to the API default.
    #[default]
    Unset,
    /// Suppress thinking (2.5 → budget 0, 3.x → unset).
    Suppress,
    /// Use a specific level.
    Level(ThinkingLevel),
}

impl From<ThinkingLevel> for ThinkingSetting {
    fn from(level: ThinkingLevel) -> Self {
        ThinkingSetting::Level(level)
    }
}

// Note: the `Default for ThinkingSetting` impl is derived (`#[default]` on `Unset`).

/// Whether the model belongs to the Gemini 3-series (`gemini-3.x` / `gemini-3-*`).
pub fn is_gemini3(model: &str) -> bool {
    model
        .strip_prefix("gemini-3")
        .is_some_and(|rest| matches!(rest.chars().next(), Some('.') | Some('-')))
}

/// Build a [`ThinkingConfig`] appropriate for the model series.
///
/// - 3-series: `thinkingLevel` string; suppression defers to the API default
///   (some 3.x models reject `MINIMAL`).
/// - 2.5-series: `thinkingBudget`; suppression sends `0`.
pub fn build_thinking_config(model: &str, setting: ThinkingSetting) -> Option<ThinkingConfig> {
    match setting {
        ThinkingSetting::Unset => None,
        ThinkingSetting::Suppress => {
            if is_gemini3(model) {
                None
            } else {
                Some(ThinkingConfig::budget(0))
            }
        }
        ThinkingSetting::Level(level) => {
            if is_gemini3(model) {
                Some(ThinkingConfig::level(level.level_str()))
            } else {
                Some(ThinkingConfig::budget(level.budget()))
            }
        }
    }
}

// ==================== Outcomes ====================

#[derive(Debug, Default)]
pub struct UsageMetadata {
    pub prompt_token_count: u32,
    pub candidates_token_count: u32,
    pub total_token_count: u32,
    pub thoughts_token_count: Option<u32>,
}

#[derive(Debug)]
pub struct ChatOutcome {
    pub text: String,
    pub usage: UsageMetadata,
    pub duration_ms: f64,
    pub actual_service_tier: Option<String>,
    pub diagnostics: ResponseDiagnostics,
}

#[derive(Debug)]
pub struct TextOutcome {
    pub text: String,
    pub diagnostics: ResponseDiagnostics,
}

#[derive(Debug)]
pub struct CodeOutcome {
    pub text: String,
    pub code: Option<String>,
    pub output: Option<String>,
    pub diagnostics: ResponseDiagnostics,
}

#[derive(Debug)]
pub struct ImageOutcome {
    pub image_bytes: Option<String>,
    pub text: String,
    pub diagnostics: ResponseDiagnostics,
}

// ==================== Options ====================

#[derive(Default)]
pub struct ChatOptions {
    pub model: Option<String>,
    pub system_instruction: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub seed: Option<i64>,
    pub stop_sequences: Option<Vec<String>>,
    pub safety_settings: Option<Vec<SafetySetting>>,
    pub json_mode: bool,
    pub grounding: bool,
    pub thinking: ThinkingSetting,
    pub tool_name: String,
    pub service_tier: Option<ServiceTierValue>,
    pub file_path: Option<String>,
}

pub struct AnalyzeMediaOptions {
    pub file_uri: Option<String>,
    pub file_path: Option<String>,
    pub image_url: Option<String>,
    pub image_base64: Option<String>,
    pub model: Option<String>,
    pub service_tier: Option<ServiceTierValue>,
    pub thinking: ThinkingSetting,
}

pub struct GenerateImageOptions {
    pub prompt: String,
    pub model: String,
    pub aspect_ratio: String,
    pub image_size: String,
    pub thinking: ThinkingSetting,
    pub service_tier: Option<ServiceTierValue>,
}

// ==================== Client ====================

/// Result of a single HTTP `generateContent` call, including the served tier header.
struct RawResult {
    response: GenerateContentResponse,
    actual_service_tier: Option<String>,
}

#[derive(Clone)]
pub struct GeminiClient {
    pub(crate) http: reqwest::Client,
    pub(crate) config: Arc<EnvConfig>,
}

impl GeminiClient {
    pub fn new(config: Arc<EnvConfig>) -> Self {
        GeminiClient {
            http: reqwest::Client::new(),
            config,
        }
    }

    pub(crate) fn api_key(&self) -> &str {
        &self.config.gemini_api_key
    }

    /// Standard timeout, doubled `base_multiplier`, unless flex tier selects the
    /// longer flex timeout.
    fn resolve_timeout(&self, tier: Option<ServiceTierValue>, base_multiplier: u64) -> u64 {
        if matches!(tier, Some(t) if t.is_flex()) {
            self.config.flex_timeout_ms
        } else {
            self.config.timeout_ms.saturating_mul(base_multiplier)
        }
    }

    /// Issue a single (non-retried) HTTP request.
    async fn post_generate_content(
        &self,
        model: &str,
        req: &GenerateContentRequest,
    ) -> Result<RawResult, GeminiError> {
        let url = format!("{API_BASE}/v1beta/models/{model}:generateContent");
        let resp = self
            .http
            .post(&url)
            .header("x-goog-api-key", self.api_key())
            .json(req)
            .send()
            .await?;

        let status = resp.status();
        let actual_service_tier = resp
            .headers()
            .get("x-gemini-service-tier")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);

        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(GeminiError::Http {
                status: status.as_u16(),
                message,
            });
        }

        let response: GenerateContentResponse = resp.json().await?;
        Ok(RawResult {
            response,
            actual_service_tier,
        })
    }

    /// Issue a `generateContent` call wrapped in telemetry + retry + timeout.
    async fn call_generate(
        &self,
        model: &str,
        req: &GenerateContentRequest,
        tool_name: &str,
        thinking_label: &str,
        tier: Option<ServiceTierValue>,
        base_multiplier: u64,
    ) -> Result<RawResult, GeminiError> {
        let timeout = self.resolve_timeout(tier, base_multiplier);
        let (raw, _dur) = with_telemetry(
            TelemetryOptions {
                tool_name,
                model,
                thinking_level: thinking_label,
            },
            with_retry(
                || with_timeout(self.post_generate_content(model, req), timeout),
                MAX_RETRIES,
                BASE_RETRY_DELAY_MS,
            ),
        )
        .await?;
        Ok(raw)
    }

    /// Chat / generate text.
    pub async fn chat(&self, prompt: &str, options: ChatOptions) -> Result<ChatOutcome, GeminiError> {
        let model = options.model.clone().unwrap_or_else(default_model);
        let tool_name = if options.tool_name.is_empty() {
            "gemini_chat".to_string()
        } else {
            options.tool_name.clone()
        };

        let mut config = GenerationConfig::default();
        if let Some(t) = options.temperature {
            config.temperature = Some(t);
        }
        if let Some(m) = options.max_tokens {
            config.max_output_tokens = Some(m);
        }
        config.top_p = options.top_p;
        config.top_k = options.top_k;
        config.seed = options.seed;
        if let Some(stops) = &options.stop_sequences {
            if !stops.is_empty() {
                config.stop_sequences = Some(stops.clone());
            }
        }
        if options.json_mode {
            config.response_mime_type = Some("application/json".to_string());
        }
        config.thinking_config = build_thinking_config(&model, options.thinking);

        let tools = if options.grounding {
            Some(vec![Tool::google_search()])
        } else {
            None
        };

        let contents = match &options.file_path {
            Some(path) => {
                let part = build_file_part(path).await?;
                vec![Content {
                    role: None,
                    parts: vec![part, Part::text(prompt)],
                }]
            }
            None => vec![Content::user_text(prompt)],
        };

        let req = GenerateContentRequest {
            contents,
            system_instruction: options.system_instruction.as_deref().map(Content::system_text),
            generation_config: optional_config(config),
            tools,
            safety_settings: options.safety_settings.clone(),
            service_tier: tier_api_value(options.service_tier),
        };

        let thinking_label = thinking_label(options.thinking);
        let start = std::time::Instant::now();
        let raw = self
            .call_generate(&model, &req, &tool_name, &thinking_label, options.service_tier, 1)
            .await?;
        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

        let usage = raw
            .response
            .usage_metadata
            .as_ref()
            .map(|u| UsageMetadata {
                prompt_token_count: u.prompt_token_count.unwrap_or(0),
                candidates_token_count: u.candidates_token_count.unwrap_or(0),
                total_token_count: u.total_token_count.unwrap_or(0),
                thoughts_token_count: u.thoughts_token_count,
            })
            .unwrap_or_default();

        Ok(ChatOutcome {
            text: raw.response.text(),
            usage,
            duration_ms,
            actual_service_tier: raw.actual_service_tier,
            diagnostics: extract_response_diagnostics(&raw.response),
        })
    }

    /// Google Search grounding. Thinking is suppressed by default (cost/latency).
    pub async fn search(
        &self,
        query: &str,
        limit: u32,
        raw: bool,
        service_tier: Option<ServiceTierValue>,
        model: &str,
    ) -> Result<TextOutcome, GeminiError> {
        let prompt = if limit > 0 {
            format!("{query}\n(Provide top {limit} results)")
        } else {
            query.to_string()
        };

        // Thinking is always suppressed for search (cost/latency); googleSearch is on.
        let config = GenerationConfig {
            thinking_config: build_thinking_config(model, ThinkingSetting::Suppress),
            ..Default::default()
        };

        let req = GenerateContentRequest {
            contents: vec![Content::user_text(&prompt)],
            generation_config: optional_config(config),
            tools: Some(vec![Tool::google_search()]),
            service_tier: tier_api_value(service_tier),
            ..Default::default()
        };

        let result = self
            .call_generate(model, &req, "gemini_search", "", service_tier, 1)
            .await?;

        let text = if raw {
            serde_json::to_string(&serde_json::json!({
                "text": result.response.text(),
            }))
            .unwrap_or_default()
        } else {
            let t = result.response.text();
            if t.is_empty() {
                "No results found.".to_string()
            } else {
                t
            }
        };

        Ok(TextOutcome {
            text,
            diagnostics: extract_response_diagnostics(&result.response),
        })
    }

    /// Multimodal media analysis.
    pub async fn analyze_media(
        &self,
        prompt: &str,
        opts: AnalyzeMediaOptions,
    ) -> Result<TextOutcome, GeminiError> {
        let model = opts.model.clone().unwrap_or_else(default_vision_model);
        let mut parts: Vec<Part> = Vec::new();

        if let Some(b64) = &opts.image_base64 {
            let (mime, data) = parse_data_uri(b64);
            parts.push(Part::inline_data(mime, data));
        } else if let Some(url) = &opts.image_url {
            parts.push(Part::file_uri("image/jpeg", url));
        } else if let Some(uri) = &opts.file_uri {
            let mut file_name = uri.rsplit('/').next().unwrap_or(uri).to_string();
            if !file_name.starts_with("files/") {
                file_name = format!("files/{file_name}");
            }
            let entry = self.get_file_status(&file_name).await?;
            let mime = entry.mime_type.unwrap_or_else(|| "application/octet-stream".to_string());
            parts.push(Part::file_uri(mime, uri));
        } else if let Some(path) = &opts.file_path {
            let bytes = tokio::fs::read(path).await?;
            let mime = binary_mime_for(path);
            let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
            parts.push(Part::inline_data(mime, data));
        }

        parts.push(Part::text(prompt));

        let config = GenerationConfig {
            thinking_config: build_thinking_config(&model, opts.thinking),
            ..Default::default()
        };

        let req = GenerateContentRequest {
            contents: vec![Content { role: None, parts }],
            generation_config: optional_config(config),
            service_tier: tier_api_value(opts.service_tier),
            ..Default::default()
        };

        let result = self
            .call_generate(
                &model,
                &req,
                "gemini_analyze_media",
                &thinking_label(opts.thinking),
                opts.service_tier,
                2,
            )
            .await?;

        Ok(TextOutcome {
            text: result.response.text(),
            diagnostics: extract_response_diagnostics(&result.response),
        })
    }

    /// Image generation via `generateContent` (Flash Image / Nano Banana 2).
    pub async fn generate_image_content(
        &self,
        opts: GenerateImageOptions,
    ) -> Result<ImageOutcome, GeminiError> {
        let mut config = GenerationConfig {
            image_config: Some(crate::services::types::ImageConfig {
                aspect_ratio: Some(opts.aspect_ratio.clone()),
                image_size: Some(opts.image_size.clone()),
            }),
            ..Default::default()
        };
        config.thinking_config = build_thinking_config(&opts.model, opts.thinking);

        let req = GenerateContentRequest {
            contents: vec![Content::user_text(&opts.prompt)],
            generation_config: optional_config(config),
            service_tier: tier_api_value(opts.service_tier),
            ..Default::default()
        };

        let result = self
            .call_generate(
                &opts.model,
                &req,
                "gemini_generate_image",
                &thinking_label(opts.thinking),
                opts.service_tier,
                1,
            )
            .await?;

        let mut image_bytes = None;
        let mut text = String::new();
        for part in result.response.first_candidate_parts() {
            if let Some(blob) = &part.inline_data {
                if image_bytes.is_none() {
                    image_bytes = Some(blob.data.clone());
                }
            } else if let Some(t) = &part.text {
                text.push_str(t);
            }
        }

        Ok(ImageOutcome {
            image_bytes,
            text,
            diagnostics: extract_response_diagnostics(&result.response),
        })
    }

    /// Code execution via the built-in `codeExecution` tool.
    pub async fn execute_code(
        &self,
        prompt: &str,
        model: &str,
        return_code: bool,
        service_tier: Option<ServiceTierValue>,
        thinking: ThinkingSetting,
    ) -> Result<CodeOutcome, GeminiError> {
        let config = GenerationConfig {
            thinking_config: build_thinking_config(model, thinking),
            ..Default::default()
        };

        let req = GenerateContentRequest {
            contents: vec![Content::user_text(prompt)],
            generation_config: optional_config(config),
            tools: Some(vec![Tool::code_execution()]),
            service_tier: tier_api_value(service_tier),
            ..Default::default()
        };

        let result = self
            .call_generate(
                model,
                &req,
                "gemini_execute_code",
                &thinking_label(thinking),
                service_tier,
                2,
            )
            .await?;

        let mut code = None;
        let mut output = None;
        if return_code {
            for part in result.response.first_candidate_parts() {
                if let Some(ec) = &part.executable_code {
                    code = Some(ec.code.clone());
                }
                if let Some(cr) = &part.code_execution_result {
                    output = Some(cr.output.clone());
                }
            }
        }

        Ok(CodeOutcome {
            text: result.response.text(),
            code,
            output,
            diagnostics: extract_response_diagnostics(&result.response),
        })
    }
}

// ==================== Free helpers ====================

fn optional_config(config: GenerationConfig) -> Option<GenerationConfig> {
    if config.is_empty() {
        None
    } else {
        Some(config)
    }
}

fn tier_api_value(tier: Option<ServiceTierValue>) -> Option<String> {
    tier.and_then(|t| t.api_value()).map(str::to_string)
}

fn thinking_label(setting: ThinkingSetting) -> String {
    match setting {
        ThinkingSetting::Level(level) => match level {
            ThinkingLevel::Minimal => "minimal",
            ThinkingLevel::Low => "low",
            ThinkingLevel::Medium => "medium",
            ThinkingLevel::High => "high",
        }
        .to_string(),
        _ => String::new(),
    }
}

/// Split a `data:<mime>;base64,<data>` URI, normalizing the payload. Falls back to
/// `image/jpeg` and the raw string when the input is not a data URI.
fn parse_data_uri(input: &str) -> (String, String) {
    if let Some(rest) = input.strip_prefix("data:") {
        if let Some((mime, data)) = rest.split_once(";base64,") {
            return (mime.to_string(), data.to_string());
        }
    }
    ("image/jpeg".to_string(), input.to_string())
}

/// Text extensions sent as plain-text parts.
const TEXT_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "rb", "go", "rs", "java", "c", "cpp", "cc", "cxx",
    "h", "hpp", "cs", "php", "swift", "kt", "kts", "scala", "md", "mdx", "txt", "json", "yaml",
    "yml", "toml", "xml", "html", "htm", "css", "scss", "sass", "less", "sh", "bash", "zsh", "sql",
    "graphql", "proto", "tf", "hcl", "ini", "cfg", "conf", "gitignore", "env", "svelte", "vue", "r",
    "m",
];

fn ext_lower(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default()
}

/// MIME type for a binary media file extension.
fn binary_mime_for(path: &str) -> String {
    match ext_lower(path).as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "heif" => "image/heif",
        "pdf" => "application/pdf",
        "mp4" => "video/mp4",
        "mpeg" => "video/mpeg",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Build a content part from a local file: text files inline as UTF-8 text, binary
/// files as base64 inline data.
async fn build_file_part(path: &str) -> Result<Part, GeminiError> {
    let bytes = tokio::fs::read(path).await?;
    let ext = ext_lower(path);
    if TEXT_EXTENSIONS.contains(&ext.as_str()) {
        Ok(Part::text(String::from_utf8_lossy(&bytes).into_owned()))
    } else {
        let mime = binary_mime_for(path);
        let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(Part::inline_data(mime, data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod is_gemini3 {
        use super::*;

        #[test]
        fn three_series_models_are_detected() {
            for m in [
                "gemini-3.5-flash",
                "gemini-3.1-flash-lite",
                "gemini-3-pro-image-preview",
                "gemini-3.1-flash-image-preview",
                "gemini-3.1-pro-preview",
                "gemini-3-flash-preview",
                "gemini-3.1-flash-lite-preview",
            ] {
                assert!(is_gemini3(m), "{m} should be gemini-3");
            }
        }

        #[test]
        fn non_three_series_models_are_rejected() {
            for m in [
                "gemini-2.5-pro",
                "gemini-2.5-flash",
                "gemini-2.5-flash-lite",
                "gemini-1.5-pro",
                "",
                "gemini-3",
            ] {
                assert!(!is_gemini3(m), "{m} should not be gemini-3");
            }
        }
    }

    mod build_thinking_config {
        use super::*;

        #[test]
        fn three_series_returns_level() {
            assert_eq!(
                build_thinking_config("gemini-3.1-pro-preview", ThinkingLevel::High.into()),
                Some(ThinkingConfig::level("HIGH"))
            );
            assert_eq!(
                build_thinking_config("gemini-3-flash-preview", ThinkingLevel::Minimal.into()),
                Some(ThinkingConfig::level("MINIMAL"))
            );
            assert_eq!(
                build_thinking_config("gemini-3.1-pro-preview", ThinkingLevel::Low.into()),
                Some(ThinkingConfig::level("LOW"))
            );
            assert_eq!(
                build_thinking_config("gemini-3.5-flash", ThinkingLevel::Medium.into()),
                Some(ThinkingConfig::level("MEDIUM"))
            );
        }

        #[test]
        fn three_series_suppress_returns_none() {
            assert_eq!(
                build_thinking_config("gemini-3.1-pro-preview", ThinkingSetting::Suppress),
                None
            );
            assert_eq!(
                build_thinking_config("gemini-3-flash-preview", ThinkingSetting::Suppress),
                None
            );
        }

        #[test]
        fn two_five_series_returns_budget() {
            assert_eq!(
                build_thinking_config("gemini-2.5-pro", ThinkingLevel::High.into()),
                Some(ThinkingConfig::budget(24576))
            );
            assert_eq!(
                build_thinking_config("gemini-2.5-flash", ThinkingLevel::Medium.into()),
                Some(ThinkingConfig::budget(8192))
            );
            assert_eq!(
                build_thinking_config("gemini-2.5-flash-lite", ThinkingLevel::Low.into()),
                Some(ThinkingConfig::budget(1024))
            );
            assert_eq!(
                build_thinking_config("gemini-2.5-pro", ThinkingLevel::Minimal.into()),
                Some(ThinkingConfig::budget(512))
            );
        }

        #[test]
        fn two_five_series_suppress_returns_budget_zero() {
            assert_eq!(
                build_thinking_config("gemini-2.5-flash-lite", ThinkingSetting::Suppress),
                Some(ThinkingConfig::budget(0))
            );
        }

        #[test]
        fn unset_returns_none_for_both_series() {
            assert_eq!(
                build_thinking_config("gemini-3.1-pro-preview", ThinkingSetting::Unset),
                None
            );
            assert_eq!(build_thinking_config("gemini-2.5-pro", ThinkingSetting::Unset), None);
        }
    }
}
