//! `gemini_execute_code` — run Python in Gemini's built-in code-execution sandbox.

use serde::Deserialize;
use serde_json::json;

use crate::schemas::{de_bool_like, resolve_service_tier, ServiceTierValue, ThinkingLevel};
use crate::services::gemini_client::{
    default_code_model, default_code_thinking_level, GeminiClient, ThinkingSetting,
};
use crate::tools::{ToolFailure, ToolResponse};
use crate::utils::diagnostics::{build_empty_response_warnings, is_abnormal_empty};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecuteCodeParams {
    pub prompt: String,

    #[serde(default = "default_code_model")]
    #[schemars(description = "[DEFAULT FIXED] gemini_execute_code is optimized for its purpose to run on gemini-flash-lite-latest. Do not override unless there is a clear reason (e.g. cost or a specific task's quality requirement).")]
    pub model: String,

    #[serde(default = "default_code_thinking_level")]
    #[schemars(description = "[DEFAULT FIXED] The thinking depth of gemini_execute_code is optimized at low. Do not override unless there is a clear reason. Values: minimal/low/medium/high.")]
    pub thinking_level: ThinkingLevel,

    #[serde(default, deserialize_with = "de_bool_like")]
    pub return_code: bool,

    #[serde(default)]
    pub service_tier: Option<ServiceTierValue>,
}

impl ExecuteCodeParams {
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

pub async fn handle_execute_code(
    client: &GeminiClient,
    params: ExecuteCodeParams,
) -> Result<ToolResponse, ToolFailure> {
    params.validate().map_err(ToolFailure::Message)?;

    let outcome = client
        .execute_code(
            &params.prompt,
            &params.model,
            params.return_code,
            resolve_service_tier(params.service_tier),
            ThinkingSetting::Level(params.thinking_level),
        )
        .await?;

    let warnings = if is_abnormal_empty(&outcome.text, &outcome.diagnostics) {
        Some(build_empty_response_warnings(&outcome.diagnostics))
    } else {
        None
    };

    let mut payload = json!({ "text": outcome.text });
    if let Some(code) = &outcome.code {
        payload["code"] = json!(code);
    }
    if let Some(output) = &outcome.output {
        payload["output"] = json!(output);
    }
    if let Some(w) = &warnings {
        payload["warnings"] = json!(w);
    }

    let text = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| outcome.text.clone());
    Ok(ToolResponse::text(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn applies_pinned_defaults() {
        let p = ExecuteCodeParams::parse(json!({ "prompt": "x" })).unwrap();
        assert_eq!(p.model, "gemini-flash-lite-latest");
        assert_eq!(p.thinking_level, ThinkingLevel::Low);
    }

    #[test]
    fn descriptions_have_default_fixed_marker() {
        let schema = serde_json::to_value(schemars::schema_for!(ExecuteCodeParams)).unwrap();
        for field in ["model", "thinking_level"] {
            assert!(schema["properties"][field]["description"]
                .as_str()
                .unwrap_or_default()
                .contains("[DEFAULT FIXED]"));
        }
    }
}
