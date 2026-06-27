//! `gemini_analyze_media` — multimodal analysis of images / PDF / video / audio.

use serde::Deserialize;
use serde_json::json;

use crate::schemas::{
    de_bool_like, default_true, resolve_service_tier, ServiceTierValue, ThinkingLevel,
};
use crate::services::gemini_client::{
    default_vision_model, default_vision_thinking_level, AnalyzeMediaOptions, GeminiClient,
    ThinkingSetting,
};
use crate::tools::{ToolFailure, ToolResponse};
use crate::utils::diagnostics::{build_empty_response_warnings, is_abnormal_empty};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnalyzeMediaParams {
    pub prompt: String,

    #[serde(default)]
    pub file_uri: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub image_base64: Option<String>,

    #[serde(default = "default_vision_model")]
    #[schemars(description = "[DEFAULT FIXED] gemini_analyze_media is optimized for its purpose to run on gemini-flash-lite-latest. Do not override unless there is a clear reason (e.g. cost or a specific task's quality requirement).")]
    pub model: String,

    #[serde(default = "default_vision_thinking_level")]
    #[schemars(description = "[DEFAULT FIXED] The thinking depth of gemini_analyze_media is optimized at medium. Do not override unless there is a clear reason. Values: minimal/low/medium/high.")]
    pub thinking_level: ThinkingLevel,

    #[serde(default = "default_true", deserialize_with = "de_bool_like")]
    pub wait_for_processing: bool,

    #[serde(default)]
    pub service_tier: Option<ServiceTierValue>,
}

impl AnalyzeMediaParams {
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
        Ok(())
    }
}

pub async fn handle_analyze_media(
    client: &GeminiClient,
    params: AnalyzeMediaParams,
) -> Result<ToolResponse, ToolFailure> {
    params.validate().map_err(ToolFailure::Message)?;

    if params.file_uri.is_none()
        && params.file_path.is_none()
        && params.image_url.is_none()
        && params.image_base64.is_none()
    {
        return Err(ToolFailure::Message(
            "Either file_uri, file_path, image_url, or image_base64 must be provided".to_string(),
        ));
    }

    // Files API base64 inline path never enters a PROCESSING state, so this flag is a
    // no-op here (kept for schema compatibility with the original tool).
    let _ = params.wait_for_processing;

    let outcome = client
        .analyze_media(
            &params.prompt,
            AnalyzeMediaOptions {
                file_uri: params.file_uri.clone(),
                file_path: params.file_path.clone(),
                image_url: params.image_url.clone(),
                image_base64: params.image_base64.clone(),
                model: Some(params.model.clone()),
                service_tier: resolve_service_tier(params.service_tier),
                thinking: ThinkingSetting::Level(params.thinking_level),
            },
        )
        .await?;

    if is_abnormal_empty(&outcome.text, &outcome.diagnostics) {
        let structured = json!({
            "text": outcome.text,
            "warnings": build_empty_response_warnings(&outcome.diagnostics),
        });
        Ok(ToolResponse::with_structured(outcome.text, structured))
    } else {
        Ok(ToolResponse::text(outcome.text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn applies_pinned_defaults() {
        let p = AnalyzeMediaParams::parse(json!({ "prompt": "x", "file_path": "/tmp/x.png" })).unwrap();
        assert_eq!(p.model, "gemini-flash-lite-latest");
        assert_eq!(p.thinking_level, ThinkingLevel::Medium);
        assert!(p.wait_for_processing);
    }

    #[test]
    fn descriptions_have_default_fixed_marker() {
        let schema = serde_json::to_value(schemars::schema_for!(AnalyzeMediaParams)).unwrap();
        for field in ["model", "thinking_level"] {
            assert!(schema["properties"][field]["description"]
                .as_str()
                .unwrap_or_default()
                .contains("[DEFAULT FIXED]"));
        }
    }
}
