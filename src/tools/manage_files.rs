//! `gemini_manage_files` — upload / list / status / delete in the Gemini Files API.

use serde::Deserialize;
use serde_json::json;

use crate::services::gemini_client::GeminiClient;
use crate::tools::{ToolFailure, ToolResponse};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ManageFilesAction {
    Upload,
    List,
    Status,
    Delete,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManageFilesParams {
    pub action: ManageFilesAction,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}

impl ManageFilesParams {
    #[cfg(test)]
    pub fn parse(value: serde_json::Value) -> Result<Self, String> {
        serde_json::from_value(value).map_err(|e| e.to_string())
    }
}

pub async fn handle_manage_files(
    client: &GeminiClient,
    params: ManageFilesParams,
) -> Result<ToolResponse, ToolFailure> {
    match params.action {
        ManageFilesAction::Upload => {
            let path = params
                .file_path
                .as_deref()
                .ok_or_else(|| ToolFailure::Message("file_path is required for upload action".to_string()))?;
            let entry = client.upload_file(path, params.display_name.as_deref()).await?;
            let text = serde_json::to_string(&entry).unwrap_or_default();
            Ok(ToolResponse::text(text))
        }
        ManageFilesAction::List => {
            let entries = client.list_files().await?;
            let text = serde_json::to_string_pretty(&entries).unwrap_or_default();
            Ok(ToolResponse::text(text))
        }
        ManageFilesAction::Status => {
            let name = params
                .file_name
                .as_deref()
                .ok_or_else(|| ToolFailure::Message("file_name is required for status action".to_string()))?;
            let entry = client.get_file_status(name).await?;
            let text = serde_json::to_string(&entry).unwrap_or_default();
            Ok(ToolResponse::text(text))
        }
        ManageFilesAction::Delete => {
            let name = params
                .file_name
                .as_deref()
                .ok_or_else(|| ToolFailure::Message("file_name is required for delete action".to_string()))?;
            client.delete_file(name).await?;
            let text = json!({
                "success": true,
                "message": format!("File {name} deleted successfully."),
            })
            .to_string();
            Ok(ToolResponse::text(text))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_list_action() {
        let p = ManageFilesParams::parse(json!({ "action": "list" })).unwrap();
        assert_eq!(p.action, ManageFilesAction::List);
    }

    #[test]
    fn rejects_model_and_thinking_level_fields() {
        assert!(ManageFilesParams::parse(json!({ "action": "list", "model": "x" })).is_err());
        assert!(ManageFilesParams::parse(json!({ "action": "list", "thinking_level": "medium" })).is_err());
    }

    #[test]
    fn rejects_invalid_action() {
        assert!(ManageFilesParams::parse(json!({ "action": "frobnicate" })).is_err());
    }
}
