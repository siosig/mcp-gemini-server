//! MCP tool handlers. Each submodule defines a parameter struct (serde + schemars)
//! plus a `handle_*` function returning a [`ToolResponse`] or [`ToolFailure`]. The
//! server layer converts those into `CallToolResult` via [`into_call_result`].

pub mod analyze_media;
pub mod custom_agent;
pub mod execute_code;
pub mod gemini_chat;
pub mod generate_image;
pub mod google_search;
pub mod manage_files;
pub mod team;

use rmcp::model::{CallToolResult, Content};
use serde_json::Value;

use crate::constants::truncate;
use crate::utils::errors::GeminiError;

/// Successful tool output: primary `text` plus optional MCP `structuredContent`.
pub struct ToolResponse {
    pub text: String,
    pub structured: Option<Value>,
}

impl ToolResponse {
    pub fn text(text: impl Into<String>) -> Self {
        ToolResponse {
            text: text.into(),
            structured: None,
        }
    }

    pub fn with_structured(text: impl Into<String>, structured: Value) -> Self {
        ToolResponse {
            text: text.into(),
            structured: Some(structured),
        }
    }
}

/// A failed tool invocation: either a classified API error or a validation message.
pub enum ToolFailure {
    Api(GeminiError),
    Message(String),
}

impl From<GeminiError> for ToolFailure {
    fn from(error: GeminiError) -> Self {
        ToolFailure::Api(error)
    }
}

/// Convert a handler result into the MCP `CallToolResult` (truncating long output,
/// attaching structured content, and formatting errors with `isError: true`).
pub fn into_call_result(result: Result<ToolResponse, ToolFailure>) -> CallToolResult {
    match result {
        Ok(resp) => {
            let mut call = CallToolResult::success(vec![Content::text(truncate(&resp.text))]);
            call.structured_content = resp.structured;
            call
        }
        Err(ToolFailure::Api(error)) => {
            CallToolResult::error(vec![Content::text(error.to_user_message())])
        }
        Err(ToolFailure::Message(message)) => {
            CallToolResult::error(vec![Content::text(format!("Error: {message}"))])
        }
    }
}
