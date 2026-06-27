//! `gemini_chat` — chat with Gemini (thinking levels, grounding, JSON mode, file input).

use serde::Deserialize;
use serde_json::json;

use crate::schemas::{
    de_bool_like, de_opt_f64_like, de_opt_i64_like, de_opt_u32_like, resolve_service_tier,
    to_api_safety_settings, SafetySettingInput, ServiceTierValue, ThinkingLevel,
};
use crate::services::gemini_client::{default_model, default_thinking_level, ChatOptions, GeminiClient, ThinkingSetting};
use crate::tools::{ToolFailure, ToolResponse};
use crate::utils::diagnostics::{build_empty_response_warnings, is_abnormal_empty};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GeminiChatParams {
    pub prompt: String,

    #[serde(default = "default_model")]
    #[schemars(description = "[DEFAULT FIXED] gemini_chat is optimized for its purpose to run on gemini-flash-latest. Do not override unless there is a clear reason (e.g. cost or a specific task's quality requirement).")]
    pub model: String,

    #[serde(default)]
    pub system_instruction: Option<String>,

    #[serde(default, deserialize_with = "de_opt_f64_like")]
    pub temperature: Option<f64>,

    #[serde(default, deserialize_with = "de_opt_u32_like")]
    pub max_tokens: Option<u32>,

    #[serde(default, deserialize_with = "de_opt_f64_like")]
    pub top_p: Option<f64>,

    #[serde(default, deserialize_with = "de_opt_u32_like")]
    pub top_k: Option<u32>,

    #[serde(default, deserialize_with = "de_opt_i64_like")]
    pub seed: Option<i64>,

    #[serde(default)]
    pub stop_sequences: Option<Vec<String>>,

    #[serde(default)]
    pub safety_settings: Option<Vec<SafetySettingInput>>,

    #[serde(default, deserialize_with = "de_bool_like")]
    pub json_mode: bool,

    #[serde(default, deserialize_with = "de_bool_like")]
    pub grounding: bool,

    #[serde(default = "default_thinking_level")]
    #[schemars(description = "[DEFAULT FIXED] The thinking depth of gemini_chat is optimized at medium. Do not override unless there is a clear reason. Values: minimal/low/medium/high.")]
    pub thinking_level: ThinkingLevel,

    #[serde(default)]
    pub service_tier: Option<ServiceTierValue>,

    #[serde(default)]
    pub file_path: Option<String>,
}

impl GeminiChatParams {
    /// Deserialize + validate (the Rust analogue of zod `safeParse`). Test-only:
    /// the runtime path uses `Deserialize` (rmcp) followed by [`Self::validate`].
    #[cfg(test)]
    pub fn parse(value: serde_json::Value) -> Result<Self, String> {
        let params: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        params.validate()?;
        Ok(params)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.prompt.is_empty() {
            return Err("prompt must not be empty".to_string());
        }
        if let Some(t) = self.temperature {
            if !(0.0..=2.0).contains(&t) {
                return Err("temperature must be between 0 and 2".to_string());
            }
        }
        if matches!(self.max_tokens, Some(0)) {
            return Err("max_tokens must be a positive integer".to_string());
        }
        if let Some(p) = self.top_p {
            if !(0.0..=1.0).contains(&p) {
                return Err("top_p must be between 0 and 1".to_string());
            }
        }
        if matches!(self.top_k, Some(0)) {
            return Err("top_k must be a positive integer".to_string());
        }
        if let Some(stops) = &self.stop_sequences {
            if stops.len() > 5 {
                return Err("stop_sequences allows at most 5 entries".to_string());
            }
            if stops.iter().any(|s| s.is_empty()) {
                return Err("stop_sequences entries must not be empty".to_string());
            }
        }
        Ok(())
    }
}

pub async fn handle_gemini_chat(
    client: &GeminiClient,
    params: GeminiChatParams,
) -> Result<ToolResponse, ToolFailure> {
    params.validate().map_err(ToolFailure::Message)?;

    let service_tier = resolve_service_tier(params.service_tier);
    let safety_settings = to_api_safety_settings(params.safety_settings.as_deref());

    let outcome = client
        .chat(
            &params.prompt,
            ChatOptions {
                model: Some(params.model.clone()),
                system_instruction: params.system_instruction.clone(),
                temperature: params.temperature,
                max_tokens: params.max_tokens,
                top_p: params.top_p,
                top_k: params.top_k,
                seed: params.seed,
                stop_sequences: params.stop_sequences.clone(),
                safety_settings,
                json_mode: params.json_mode,
                grounding: params.grounding,
                thinking: ThinkingSetting::Level(params.thinking_level),
                tool_name: "gemini_chat".to_string(),
                service_tier,
                file_path: params.file_path.clone(),
            },
        )
        .await?;

    let display_text = match &outcome.actual_service_tier {
        Some(tier) => format!("{}\n\n[Service Tier: {tier}]", outcome.text),
        None => outcome.text.clone(),
    };

    let warnings = if is_abnormal_empty(&outcome.text, &outcome.diagnostics) {
        Some(build_empty_response_warnings(&outcome.diagnostics))
    } else {
        None
    };

    let mut structured = json!({
        "model": params.model,
        "text": outcome.text,
        "usage": {
            "promptTokenCount": outcome.usage.prompt_token_count,
            "candidatesTokenCount": outcome.usage.candidates_token_count,
            "totalTokenCount": outcome.usage.total_token_count,
            "thoughtsTokenCount": outcome.usage.thoughts_token_count,
        },
        "duration_ms": outcome.duration_ms.round() as i64,
    });
    if let Some(tier) = &outcome.actual_service_tier {
        structured["service_tier"] = json!(tier);
    }
    if let Some(w) = &warnings {
        structured["warnings"] = json!(w);
    }

    Ok(ToolResponse::with_structured(display_text, structured))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema_description(field: &str) -> String {
        let schema = serde_json::to_value(schemars::schema_for!(GeminiChatParams)).unwrap();
        schema["properties"][field]["description"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn accepts_prompt_without_file_path() {
        let p = GeminiChatParams::parse(json!({ "prompt": "hello" })).unwrap();
        assert!(p.file_path.is_none());
    }

    #[test]
    fn accepts_prompt_with_valid_file_path() {
        let p = GeminiChatParams::parse(json!({ "prompt": "review", "file_path": "/tmp/file.ts" }))
            .unwrap();
        assert_eq!(p.file_path.as_deref(), Some("/tmp/file.ts"));
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(GeminiChatParams::parse(json!({ "prompt": "hi", "unknown_field": "x" })).is_err());
    }

    #[test]
    fn rejects_non_string_file_path() {
        assert!(GeminiChatParams::parse(json!({ "prompt": "hi", "file_path": 42 })).is_err());
    }

    #[test]
    fn applies_pinned_defaults() {
        let p = GeminiChatParams::parse(json!({ "prompt": "x" })).unwrap();
        assert_eq!(p.model, "gemini-flash-latest");
        assert_eq!(p.thinking_level, ThinkingLevel::Medium);
    }

    #[test]
    fn model_and_thinking_descriptions_have_default_fixed_marker() {
        assert!(schema_description("model").contains("[DEFAULT FIXED]"));
        assert!(schema_description("thinking_level").contains("[DEFAULT FIXED]"));
    }
}
