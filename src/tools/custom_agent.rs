//! `gemini_custom_agent` — one-shot Gemini agent with a specialized role.

use serde::Deserialize;

use crate::schemas::{resolve_service_tier, ServiceTierValue, ThinkingLevel};
use crate::services::gemini_client::{
    default_agent_model, default_agent_thinking_level, ChatOptions, GeminiClient, ThinkingSetting,
};
use crate::tools::{ToolFailure, ToolResponse};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CustomAgentParams {
    pub task: String,
    pub role: String,

    #[serde(default)]
    pub persona: Option<String>,

    #[serde(default = "default_agent_model")]
    #[schemars(description = "[DEFAULT FIXED] gemini_custom_agent is optimized for its purpose to run on gemini-flash-latest. Do not override unless there is a clear reason (e.g. cost or a specific task's quality requirement).")]
    pub model: String,

    #[serde(default = "default_agent_thinking_level")]
    #[schemars(description = "[DEFAULT FIXED] The thinking depth of gemini_custom_agent is optimized at high. Do not override unless there is a clear reason. Values: minimal/low/medium/high.")]
    pub thinking_level: ThinkingLevel,

    #[serde(default)]
    pub service_tier: Option<ServiceTierValue>,

    #[serde(default)]
    pub file_path: Option<String>,
}

impl CustomAgentParams {
    #[cfg(test)]
    pub fn parse(value: serde_json::Value) -> Result<Self, String> {
        let params: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        params.validate()?;
        Ok(params)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.task.is_empty() {
            return Err("task must not be empty".to_string());
        }
        if self.role.is_empty() {
            return Err("role must not be empty".to_string());
        }
        Ok(())
    }
}

pub async fn handle_custom_agent(
    client: &GeminiClient,
    params: CustomAgentParams,
) -> Result<ToolResponse, ToolFailure> {
    params.validate().map_err(ToolFailure::Message)?;

    let mut system = format!("You are a {}.", params.role);
    if let Some(persona) = &params.persona {
        system.push_str(&format!("\nPersonality/style: {persona}"));
    }
    system.push_str("\n\nApply your expertise to respond to the task.");

    let outcome = client
        .chat(
            &params.task,
            ChatOptions {
                model: Some(params.model.clone()),
                system_instruction: Some(system),
                temperature: Some(0.7),
                thinking: ThinkingSetting::Level(params.thinking_level),
                tool_name: "gemini_custom_agent".to_string(),
                service_tier: resolve_service_tier(params.service_tier),
                file_path: params.file_path.clone(),
                ..Default::default()
            },
        )
        .await?;

    Ok(ToolResponse::text(outcome.text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_task_and_role_without_file_path() {
        let p = CustomAgentParams::parse(json!({ "task": "review this", "role": "reviewer" })).unwrap();
        assert!(p.file_path.is_none());
    }

    #[test]
    fn accepts_valid_file_path() {
        let p = CustomAgentParams::parse(json!({
            "task": "find bugs", "role": "developer", "file_path": "/home/user/src/main.py"
        }))
        .unwrap();
        assert_eq!(p.file_path.as_deref(), Some("/home/user/src/main.py"));
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(CustomAgentParams::parse(json!({ "task": "t", "role": "r", "unknown_field": "x" })).is_err());
    }

    #[test]
    fn rejects_non_string_file_path() {
        assert!(CustomAgentParams::parse(json!({ "task": "t", "role": "r", "file_path": true })).is_err());
    }

    #[test]
    fn applies_pinned_defaults() {
        let p = CustomAgentParams::parse(json!({ "task": "x", "role": "developer" })).unwrap();
        assert_eq!(p.model, "gemini-flash-latest");
        assert_eq!(p.thinking_level, ThinkingLevel::High);
    }

    #[test]
    fn descriptions_have_default_fixed_marker() {
        let schema = serde_json::to_value(schemars::schema_for!(CustomAgentParams)).unwrap();
        for field in ["model", "thinking_level"] {
            assert!(
                schema["properties"][field]["description"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("[DEFAULT FIXED]"),
                "{field} should carry the marker"
            );
        }
    }
}
