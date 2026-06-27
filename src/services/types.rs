//! Wire types for the Generative Language REST API (`v1beta`).
//!
//! These mirror the request/response JSON shapes used by `generateContent` and the
//! Files API. Response structs use `#[serde(default)]` so that absent fields decode
//! to `None`/empty rather than failing, matching the permissive behavior of the
//! original `@google/genai` SDK consumer code.

use serde::{Deserialize, Serialize};

// ==================== Shared content ====================

/// A single message/turn of content.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub role: Option<String>,
    pub parts: Vec<Part>,
}

impl Content {
    /// Build a user content block from a single text part.
    pub fn user_text(text: impl Into<String>) -> Self {
        Content {
            role: Some("user".to_string()),
            parts: vec![Part::text(text)],
        }
    }

    /// Build a system-instruction content block (role omitted) from text.
    pub fn system_text(text: impl Into<String>) -> Self {
        Content {
            role: None,
            parts: vec![Part::text(text)],
        }
    }
}

/// A content part. Modeled as a struct of mutually-exclusive optional fields
/// (rather than an enum) to match the SDK's `Part` shape for both directions.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub inline_data: Option<Blob>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub file_data: Option<FileDataRef>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub executable_code: Option<CodeBlock>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub code_execution_result: Option<CodeResult>,
}

impl Part {
    pub fn text(text: impl Into<String>) -> Self {
        Part {
            text: Some(text.into()),
            ..Default::default()
        }
    }

    pub fn inline_data(mime_type: impl Into<String>, data: impl Into<String>) -> Self {
        Part {
            inline_data: Some(Blob {
                mime_type: mime_type.into(),
                data: data.into(),
            }),
            ..Default::default()
        }
    }

    pub fn file_uri(mime_type: impl Into<String>, file_uri: impl Into<String>) -> Self {
        Part {
            file_data: Some(FileDataRef {
                mime_type: mime_type.into(),
                file_uri: file_uri.into(),
            }),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Blob {
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileDataRef {
    pub mime_type: String,
    pub file_uri: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodeBlock {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub language: Option<String>,
    pub code: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodeResult {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub outcome: Option<String>,
    pub output: String,
}

// ==================== Request ====================

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentRequest {
    pub contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_settings: Option<Vec<SafetySetting>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_config: Option<ImageConfig>,
}

impl GenerationConfig {
    /// True when no field is set — used to omit `generationConfig` entirely.
    pub fn is_empty(&self) -> bool {
        *self == GenerationConfig::default()
    }
}

/// Thinking configuration. Exactly one of the two fields is populated depending on
/// the model series (2.5 → `thinkingBudget`, 3.x → `thinkingLevel`).
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<String>,
}

impl ThinkingConfig {
    pub fn budget(n: u32) -> Self {
        ThinkingConfig {
            thinking_budget: Some(n),
            thinking_level: None,
        }
    }

    pub fn level(level: impl Into<String>) -> Self {
        ThinkingConfig {
            thinking_budget: None,
            thinking_level: Some(level.into()),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImageConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_size: Option<String>,
}

/// A built-in tool. Exactly one field is set; the value is an empty object.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google_search: Option<EmptyObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_execution: Option<EmptyObject>,
}

impl Tool {
    pub fn google_search() -> Self {
        Tool {
            google_search: Some(EmptyObject {}),
            code_execution: None,
        }
    }

    pub fn code_execution() -> Self {
        Tool {
            google_search: None,
            code_execution: Some(EmptyObject {}),
        }
    }
}

/// Serializes to `{}`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct EmptyObject {}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetySetting {
    pub category: String,
    pub threshold: String,
}

// ==================== Response ====================

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GenerateContentResponse {
    pub candidates: Option<Vec<Candidate>>,
    pub prompt_feedback: Option<PromptFeedback>,
    pub usage_metadata: Option<UsageMetadata>,
}

impl GenerateContentResponse {
    /// Concatenate every `text` part of the first candidate.
    pub fn text(&self) -> String {
        let Some(candidates) = &self.candidates else {
            return String::new();
        };
        let Some(first) = candidates.first() else {
            return String::new();
        };
        let Some(content) = &first.content else {
            return String::new();
        };
        content
            .parts
            .iter()
            .filter_map(|p| p.text.as_deref())
            .collect::<Vec<_>>()
            .concat()
    }

    /// Parts of the first candidate (empty slice when unavailable).
    pub fn first_candidate_parts(&self) -> &[Part] {
        self.candidates
            .as_deref()
            .and_then(|c| c.first())
            .and_then(|c| c.content.as_ref())
            .map(|c| c.parts.as_slice())
            .unwrap_or(&[])
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Candidate {
    pub content: Option<Content>,
    pub finish_reason: Option<String>,
    pub finish_message: Option<String>,
    pub safety_ratings: Option<Vec<SafetyRating>>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PromptFeedback {
    pub block_reason: Option<String>,
    pub safety_ratings: Option<Vec<SafetyRating>>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SafetyRating {
    pub category: Option<String>,
    pub probability: Option<String>,
    pub blocked: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct UsageMetadata {
    pub prompt_token_count: Option<u32>,
    pub candidates_token_count: Option<u32>,
    pub total_token_count: Option<u32>,
    pub thoughts_token_count: Option<u32>,
}
