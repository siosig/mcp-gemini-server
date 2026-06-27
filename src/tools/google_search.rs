//! `gemini_search` — Google Search grounding (no thinking; budget suppressed to 0).

use serde::Deserialize;
use serde_json::json;

use crate::schemas::{
    de_bool_like, de_u32_like, resolve_service_tier, ServiceTierValue,
};
use crate::services::gemini_client::{default_search_model, GeminiClient};
use crate::tools::{ToolFailure, ToolResponse};
use crate::utils::diagnostics::{build_empty_response_warnings, is_abnormal_empty};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GoogleSearchParams {
    pub query: String,

    #[serde(default, deserialize_with = "de_u32_like")]
    pub limit: u32,

    #[serde(default, deserialize_with = "de_bool_like")]
    pub raw: bool,

    #[serde(default = "default_search_model")]
    #[schemars(description = "[DEFAULT FIXED] gemini_search is optimized for its purpose to run on gemini-flash-lite-latest. Do not override unless there is a clear reason (e.g. cost or a specific task's quality requirement).")]
    pub model: String,

    #[serde(default)]
    pub service_tier: Option<ServiceTierValue>,
}

impl GoogleSearchParams {
    #[cfg(test)]
    pub fn parse(value: serde_json::Value) -> Result<Self, String> {
        let params: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        params.validate()?;
        Ok(params)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.query.is_empty() {
            return Err("query must not be empty".to_string());
        }
        Ok(())
    }
}

pub async fn handle_google_search(
    client: &GeminiClient,
    params: GoogleSearchParams,
) -> Result<ToolResponse, ToolFailure> {
    params.validate().map_err(ToolFailure::Message)?;

    let outcome = client
        .search(
            &params.query,
            params.limit,
            params.raw,
            resolve_service_tier(params.service_tier),
            &params.model,
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
    fn default_model_is_flash_lite() {
        let p = GoogleSearchParams::parse(json!({ "query": "x" })).unwrap();
        assert_eq!(p.model, "gemini-flash-lite-latest");
    }

    #[test]
    fn thinking_level_is_not_accepted() {
        assert!(GoogleSearchParams::parse(json!({ "query": "x", "thinking_level": "medium" })).is_err());
    }

    #[test]
    fn model_description_has_default_fixed_marker() {
        let schema = serde_json::to_value(schemars::schema_for!(GoogleSearchParams)).unwrap();
        assert!(schema["properties"]["model"]["description"]
            .as_str()
            .unwrap_or_default()
            .contains("[DEFAULT FIXED]"));
    }
}
