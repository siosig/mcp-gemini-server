//! Shared schema helpers: LLM-tolerant deserializers (string-or-native coercion),
//! the thinking-level / service-tier enums, safety-setting conversion, and the
//! "pinned default" description wording reused across tools.

use serde::de::{self, Deserializer, Unexpected, Visitor};
use serde::Deserialize;

use crate::services::types::SafetySetting;

// ==================== thinking_level ====================

/// Caller-facing thinking depth. Maps to a token budget (2.5 series) or a level
/// string (3.x series) by [`crate::services::gemini_client::build_thinking_config`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Minimal,
    Low,
    Medium,
    High,
}

impl ThinkingLevel {
    /// Token budget for Gemini 2.5-series models. `minimal=512` aligns with the
    /// flash-lite lower bound (512–24576).
    pub fn budget(self) -> u32 {
        match self {
            ThinkingLevel::Minimal => 512,
            ThinkingLevel::Low => 1024,
            ThinkingLevel::Medium => 8192,
            ThinkingLevel::High => 24576,
        }
    }

    /// Level string for Gemini 3.x-series models.
    pub fn level_str(self) -> &'static str {
        match self {
            ThinkingLevel::Minimal => "MINIMAL",
            ThinkingLevel::Low => "LOW",
            ThinkingLevel::Medium => "MEDIUM",
            ThinkingLevel::High => "HIGH",
        }
    }

    /// Parse from an environment-variable string, falling back to `default` on any
    /// unrecognized value.
    pub fn from_env(var: &str, default: ThinkingLevel) -> ThinkingLevel {
        match std::env::var(var).ok().as_deref() {
            Some("minimal") => ThinkingLevel::Minimal,
            Some("low") => ThinkingLevel::Low,
            Some("medium") => ThinkingLevel::Medium,
            Some("high") => ThinkingLevel::High,
            _ => default,
        }
    }
}

// ==================== service_tier ====================

/// Inference tier accepted as an MCP tool argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ServiceTierValue {
    Flex,
    Priority,
    Standard,
}

impl ServiceTierValue {
    fn parse(s: &str) -> Option<ServiceTierValue> {
        match s {
            "flex" => Some(ServiceTierValue::Flex),
            "priority" => Some(ServiceTierValue::Priority),
            "standard" => Some(ServiceTierValue::Standard),
            _ => None,
        }
    }

    /// API request value. `standard` is omitted (defers to the API default).
    pub fn api_value(self) -> Option<&'static str> {
        match self {
            ServiceTierValue::Flex => Some("FLEX"),
            ServiceTierValue::Priority => Some("PRIORITY"),
            ServiceTierValue::Standard => None,
        }
    }

    /// True for the flex tier (used to select the longer timeout).
    pub fn is_flex(self) -> bool {
        matches!(self, ServiceTierValue::Flex)
    }
}

/// Default service tier from `GEMINI_SERVICE_TIER` (invalid values → `None`).
pub fn default_service_tier() -> Option<ServiceTierValue> {
    std::env::var("GEMINI_SERVICE_TIER")
        .ok()
        .and_then(|v| ServiceTierValue::parse(&v))
}

/// Resolve the effective service tier: tool argument > env var > none.
/// `standard` collapses to `None` (API default behavior).
pub fn resolve_service_tier(tool_arg: Option<ServiceTierValue>) -> Option<ServiceTierValue> {
    let effective = tool_arg.or_else(default_service_tier)?;
    match effective {
        ServiceTierValue::Standard => None,
        other => Some(other),
    }
}

// ==================== safety_settings ====================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HarmCategoryShort {
    Harassment,
    HateSpeech,
    SexuallyExplicit,
    DangerousContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HarmThresholdShort {
    Low,
    Medium,
    High,
    None,
}

/// One entry of the `safety_settings` array (LLM-friendly shorthand).
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SafetySettingInput {
    pub category: HarmCategoryShort,
    pub threshold: HarmThresholdShort,
}

impl SafetySettingInput {
    fn category_api(&self) -> &'static str {
        match self.category {
            HarmCategoryShort::Harassment => "HARM_CATEGORY_HARASSMENT",
            HarmCategoryShort::HateSpeech => "HARM_CATEGORY_HATE_SPEECH",
            HarmCategoryShort::SexuallyExplicit => "HARM_CATEGORY_SEXUALLY_EXPLICIT",
            HarmCategoryShort::DangerousContent => "HARM_CATEGORY_DANGEROUS_CONTENT",
        }
    }

    fn threshold_api(&self) -> &'static str {
        match self.threshold {
            HarmThresholdShort::Low => "BLOCK_LOW_AND_ABOVE",
            HarmThresholdShort::Medium => "BLOCK_MEDIUM_AND_ABOVE",
            HarmThresholdShort::High => "BLOCK_ONLY_HIGH",
            HarmThresholdShort::None => "BLOCK_NONE",
        }
    }
}

/// Convert shorthand safety settings into the API representation.
pub fn to_api_safety_settings(input: Option<&[SafetySettingInput]>) -> Option<Vec<SafetySetting>> {
    let input = input?;
    if input.is_empty() {
        return None;
    }
    Some(
        input
            .iter()
            .map(|s| SafetySetting {
                category: s.category_api().to_string(),
                threshold: s.threshold_api().to_string(),
            })
            .collect(),
    )
}

// ==================== string-or-native coercion (zod booleanLike/numberLike) ====================

/// Deserialize a `bool` from either a native boolean or the strings `"true"`/`"false"`.
pub fn de_bool_like<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    struct BoolLike;
    impl Visitor<'_> for BoolLike {
        type Value = bool;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a boolean or the string \"true\"/\"false\"")
        }
        fn visit_bool<E: de::Error>(self, v: bool) -> Result<bool, E> {
            Ok(v)
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<bool, E> {
            match v {
                "true" => Ok(true),
                "false" => Ok(false),
                other => Err(E::invalid_value(Unexpected::Str(other), &self)),
            }
        }
    }
    deserializer.deserialize_any(BoolLike)
}

/// Parse a `f64` from a native number or a numeric string.
fn number_like<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    struct NumberLike;
    impl Visitor<'_> for NumberLike {
        type Value = f64;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a number or a numeric string")
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<f64, E> {
            Ok(v)
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<f64, E> {
            Ok(v as f64)
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<f64, E> {
            Ok(v as f64)
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<f64, E> {
            v.trim()
                .parse::<f64>()
                .map_err(|_| E::invalid_value(Unexpected::Str(v), &self))
        }
    }
    deserializer.deserialize_any(NumberLike)
}

/// Optional `f64` accepting number-or-string.
pub fn de_opt_f64_like<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    number_like(deserializer).map(Some)
}

/// Optional `u32` accepting number-or-string; rejects non-integer / negative.
pub fn de_opt_u32_like<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let n = number_like(deserializer)?;
    to_u32::<D>(n).map(Some)
}

/// Optional `i64` accepting number-or-string; rejects non-integer.
pub fn de_opt_i64_like<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let n = number_like(deserializer)?;
    to_i64::<D>(n).map(Some)
}

/// `i64` accepting number-or-string (for fields with a numeric default).
pub fn de_i64_like<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let n = number_like(deserializer)?;
    to_i64::<D>(n)
}

/// `u32` accepting number-or-string (for fields with a numeric default).
pub fn de_u32_like<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let n = number_like(deserializer)?;
    to_u32::<D>(n)
}

/// Serde `default` helper for boolean fields whose default is `true`.
pub fn default_true() -> bool {
    true
}

fn to_u32<'de, D>(n: f64) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    if n.fract() != 0.0 || n < 0.0 || n > f64::from(u32::MAX) {
        return Err(de::Error::custom(format!("expected a non-negative integer, got {n}")));
    }
    Ok(n as u32)
}

fn to_i64<'de, D>(n: f64) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    if n.fract() != 0.0 {
        return Err(de::Error::custom(format!("expected an integer, got {n}")));
    }
    Ok(n as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_service_tier_collapses_standard_to_none() {
        assert_eq!(resolve_service_tier(Some(ServiceTierValue::Standard)), None);
        assert_eq!(
            resolve_service_tier(Some(ServiceTierValue::Flex)),
            Some(ServiceTierValue::Flex)
        );
    }

    #[test]
    fn thinking_level_budget_mapping() {
        assert_eq!(ThinkingLevel::Minimal.budget(), 512);
        assert_eq!(ThinkingLevel::Low.budget(), 1024);
        assert_eq!(ThinkingLevel::Medium.budget(), 8192);
        assert_eq!(ThinkingLevel::High.budget(), 24576);
    }
}
