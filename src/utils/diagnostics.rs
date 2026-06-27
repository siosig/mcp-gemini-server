//! Diagnostic metadata extraction from Gemini API responses (L1 extraction layer).
//!
//! Makes "completed normally but the body is empty" cases (Safety / MAX_TOKENS /
//! RECITATION, etc.) traceable after the fact. Pure functions only: no side effects,
//! no panics, and finish reasons are kept as raw strings so newly added API values do
//! not cause enumeration-gap regressions (spec 020).

use crate::services::types::{GenerateContentResponse, SafetyRating};

/// A safety rating reduced to the fields that survive logging/serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafetyRatingLite {
    pub category: Option<String>,
    pub probability: Option<String>,
}

/// Response diagnostic information.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResponseDiagnostics {
    /// `candidates[0].finishReason`. `None` if unavailable.
    pub finish_reason: Option<String>,
    /// `promptFeedback.blockReason` (prompt-side block). `None` if unavailable.
    pub block_reason: Option<String>,
    /// Candidate or prompt safety ratings. An empty list is normalized to `None`.
    pub safety_ratings: Option<Vec<SafetyRatingLite>>,
    /// Candidate count (0 is a strong signal of a block-induced empty response).
    pub candidate_count: usize,
    /// Thinking tokens consumed (used to detect MAX_TOKENS exhaustion).
    pub thoughts_token_count: Option<u32>,
    /// Human-readable finish-reason explanation (supplementary).
    pub finish_message: Option<String>,
}

/// Normalize an empty list to `None` (`[]` is treated as equivalent to "none").
fn normalize_ratings(ratings: Option<&Vec<SafetyRating>>) -> Option<Vec<SafetyRatingLite>> {
    let ratings = ratings?;
    if ratings.is_empty() {
        return None;
    }
    Some(
        ratings
            .iter()
            .map(|r| SafetyRatingLite {
                category: r.category.clone(),
                probability: r.probability.clone(),
            })
            .collect(),
    )
}

/// Extract diagnostic metadata from a [`GenerateContentResponse`].
///
/// Total: returns a [`ResponseDiagnostics`] for any input (including all-empty).
pub fn extract_response_diagnostics(response: &GenerateContentResponse) -> ResponseDiagnostics {
    let candidates = response.candidates.as_deref();
    let first = candidates.and_then(|c| c.first());

    // Prefer candidate-side safety ratings; fall back to prompt-side.
    let safety_ratings = normalize_ratings(first.and_then(|c| c.safety_ratings.as_ref()))
        .or_else(|| {
            normalize_ratings(
                response
                    .prompt_feedback
                    .as_ref()
                    .and_then(|pf| pf.safety_ratings.as_ref()),
            )
        });

    ResponseDiagnostics {
        finish_reason: first.and_then(|c| c.finish_reason.clone()),
        block_reason: response
            .prompt_feedback
            .as_ref()
            .and_then(|pf| pf.block_reason.clone()),
        safety_ratings,
        candidate_count: candidates.map(<[_]>::len).unwrap_or(0),
        thoughts_token_count: response
            .usage_metadata
            .as_ref()
            .and_then(|u| u.thoughts_token_count),
        finish_message: first.and_then(|c| c.finish_message.clone()),
    }
}

/// Whether the response is an *abnormal* empty response (empty body that did not
/// finish normally).
///
/// - Always `false` when there is body text (whitespace-only is treated as empty).
/// - When `finish_reason` is unavailable, returns `false` to avoid false positives.
pub fn is_abnormal_empty(text: &str, d: &ResponseDiagnostics) -> bool {
    if !text.trim().is_empty() {
        return false;
    }
    if d.block_reason.is_some() {
        return true;
    }
    if d.candidate_count == 0 {
        return true;
    }
    matches!(&d.finish_reason, Some(r) if r != "STOP")
}

/// Build the warnings text for an abnormal empty response (cause included).
pub fn build_empty_response_warnings(d: &ResponseDiagnostics) -> Vec<String> {
    let mut warnings = Vec::new();

    let reason = if let Some(block) = &d.block_reason {
        format!("prompt blocked (blockReason={block})")
    } else if let Some(finish) = &d.finish_reason {
        format!("finishReason={finish}")
    } else {
        "unknown cause".to_string()
    };
    warnings.push(format!("Empty response from Gemini: {reason}."));

    if d.finish_reason.as_deref() == Some("MAX_TOKENS") && d.thoughts_token_count.unwrap_or(0) > 0 {
        let tokens = d.thoughts_token_count.unwrap_or(0);
        warnings.push(format!(
            "Output truncated at MAX_TOKENS after consuming {tokens} thinking tokens (thinking budget likely exhausted before any answer). Consider raising max_tokens or lowering thinking_level."
        ));
    }

    if let Some(ratings) = &d.safety_ratings {
        if !ratings.is_empty() {
            let cats = ratings
                .iter()
                .map(|r| {
                    format!(
                        "{}:{}",
                        r.category.as_deref().unwrap_or("?"),
                        r.probability.as_deref().unwrap_or("?")
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            warnings.push(format!("Safety ratings: {cats}."));
        }
    }

    if let Some(msg) = &d.finish_message {
        warnings.push(format!("Model message: {msg}"));
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::types::{Candidate, PromptFeedback, UsageMetadata};

    fn response(
        candidates: Option<Vec<Candidate>>,
        prompt_feedback: Option<PromptFeedback>,
        usage_metadata: Option<UsageMetadata>,
    ) -> GenerateContentResponse {
        GenerateContentResponse {
            candidates,
            prompt_feedback,
            usage_metadata,
        }
    }

    fn rating(category: &str, probability: &str) -> SafetyRating {
        SafetyRating {
            category: Some(category.to_string()),
            probability: Some(probability.to_string()),
            blocked: None,
        }
    }

    mod extract_response_diagnostics {
        use super::*;

        #[test]
        fn normal_stop_extracts_reason_count_and_ratings() {
            let d = extract_response_diagnostics(&response(
                Some(vec![Candidate {
                    finish_reason: Some("STOP".into()),
                    safety_ratings: Some(vec![rating("HARM_CATEGORY_HARASSMENT", "NEGLIGIBLE")]),
                    ..Default::default()
                }]),
                None,
                Some(UsageMetadata {
                    thoughts_token_count: Some(0),
                    ..Default::default()
                }),
            ));
            assert_eq!(d.finish_reason.as_deref(), Some("STOP"));
            assert_eq!(d.candidate_count, 1);
            assert_eq!(d.block_reason, None);
            assert_eq!(
                d.safety_ratings,
                Some(vec![SafetyRatingLite {
                    category: Some("HARM_CATEGORY_HARASSMENT".into()),
                    probability: Some("NEGLIGIBLE".into()),
                }])
            );
            assert_eq!(d.thoughts_token_count, Some(0));
        }

        #[test]
        fn safety_candidate_side() {
            let d = extract_response_diagnostics(&response(
                Some(vec![Candidate {
                    finish_reason: Some("SAFETY".into()),
                    safety_ratings: Some(vec![rating("HARM_CATEGORY_DANGEROUS_CONTENT", "HIGH")]),
                    ..Default::default()
                }]),
                None,
                None,
            ));
            assert_eq!(d.finish_reason.as_deref(), Some("SAFETY"));
            assert_eq!(d.candidate_count, 1);
            assert_eq!(d.safety_ratings.unwrap()[0].probability.as_deref(), Some("HIGH"));
        }

        #[test]
        fn safety_prompt_side_with_zero_candidates() {
            let d = extract_response_diagnostics(&response(
                Some(vec![]),
                Some(PromptFeedback {
                    block_reason: Some("SAFETY".into()),
                    safety_ratings: Some(vec![rating("HARM_CATEGORY_HATE_SPEECH", "MEDIUM")]),
                }),
                None,
            ));
            assert_eq!(d.candidate_count, 0);
            assert_eq!(d.block_reason.as_deref(), Some("SAFETY"));
            assert_eq!(d.finish_reason, None);
            assert_eq!(
                d.safety_ratings.unwrap()[0].category.as_deref(),
                Some("HARM_CATEGORY_HATE_SPEECH")
            );
        }

        #[test]
        fn max_tokens_records_thoughts_token_count() {
            let d = extract_response_diagnostics(&response(
                Some(vec![Candidate {
                    finish_reason: Some("MAX_TOKENS".into()),
                    ..Default::default()
                }]),
                None,
                Some(UsageMetadata {
                    thoughts_token_count: Some(24000),
                    ..Default::default()
                }),
            ));
            assert_eq!(d.finish_reason.as_deref(), Some("MAX_TOKENS"));
            assert_eq!(d.thoughts_token_count, Some(24000));
        }

        #[test]
        fn recitation_finish_reason() {
            let d = extract_response_diagnostics(&response(
                Some(vec![Candidate {
                    finish_reason: Some("RECITATION".into()),
                    ..Default::default()
                }]),
                None,
                None,
            ));
            assert_eq!(d.finish_reason.as_deref(), Some("RECITATION"));
        }

        #[test]
        fn all_missing_fields_tolerated() {
            let d = extract_response_diagnostics(&GenerateContentResponse::default());
            assert_eq!(d.finish_reason, None);
            assert_eq!(d.block_reason, None);
            assert_eq!(d.safety_ratings, None);
            assert_eq!(d.candidate_count, 0);
            assert_eq!(d.thoughts_token_count, None);
        }

        #[test]
        fn empty_safety_ratings_normalized_to_none() {
            let d = extract_response_diagnostics(&response(
                Some(vec![Candidate {
                    finish_reason: Some("STOP".into()),
                    safety_ratings: Some(vec![]),
                    ..Default::default()
                }]),
                Some(PromptFeedback {
                    safety_ratings: Some(vec![]),
                    ..Default::default()
                }),
                None,
            ));
            assert_eq!(d.safety_ratings, None);
        }

        #[test]
        fn extracts_finish_message() {
            let d = extract_response_diagnostics(&response(
                Some(vec![Candidate {
                    finish_reason: Some("OTHER".into()),
                    finish_message: Some("stopped for other reason".into()),
                    ..Default::default()
                }]),
                None,
                None,
            ));
            assert_eq!(d.finish_message.as_deref(), Some("stopped for other reason"));
        }
    }

    mod is_abnormal_empty {
        use super::*;

        fn base() -> ResponseDiagnostics {
            ResponseDiagnostics {
                candidate_count: 1,
                ..Default::default()
            }
        }

        #[test]
        fn false_when_body_text_present_even_with_abnormal_reason() {
            let d = ResponseDiagnostics {
                finish_reason: Some("SAFETY".into()),
                ..base()
            };
            assert!(!is_abnormal_empty("hello", &d));
        }

        #[test]
        fn whitespace_only_with_block_reason_is_true() {
            let d = ResponseDiagnostics {
                candidate_count: 0,
                block_reason: Some("SAFETY".into()),
                ..base()
            };
            assert!(is_abnormal_empty("   \n\t ", &d));
        }

        #[test]
        fn empty_with_zero_candidates_is_true() {
            let d = ResponseDiagnostics {
                candidate_count: 0,
                ..Default::default()
            };
            assert!(is_abnormal_empty("", &d));
        }

        #[test]
        fn empty_with_non_stop_finish_reason_is_true() {
            let d = ResponseDiagnostics {
                finish_reason: Some("MAX_TOKENS".into()),
                ..base()
            };
            assert!(is_abnormal_empty("", &d));
        }

        #[test]
        fn empty_with_stop_is_false() {
            let d = ResponseDiagnostics {
                finish_reason: Some("STOP".into()),
                ..base()
            };
            assert!(!is_abnormal_empty("", &d));
        }

        #[test]
        fn empty_with_unobtainable_reason_is_false() {
            assert!(!is_abnormal_empty("", &base()));
        }
    }

    mod build_empty_response_warnings {
        use super::*;

        #[test]
        fn contains_finish_reason() {
            let w = build_empty_response_warnings(&ResponseDiagnostics {
                candidate_count: 1,
                finish_reason: Some("SAFETY".into()),
                ..Default::default()
            });
            assert!(!w.is_empty());
            assert!(w.join(" ").contains("SAFETY"));
        }

        #[test]
        fn contains_block_reason() {
            let w = build_empty_response_warnings(&ResponseDiagnostics {
                candidate_count: 0,
                block_reason: Some("PROHIBITED_CONTENT".into()),
                ..Default::default()
            });
            assert!(w.join(" ").contains("PROHIBITED_CONTENT"));
        }

        #[test]
        fn max_tokens_with_thinking_tokens_adds_exhaustion_warning() {
            let w = build_empty_response_warnings(&ResponseDiagnostics {
                candidate_count: 1,
                finish_reason: Some("MAX_TOKENS".into()),
                thoughts_token_count: Some(24000),
                ..Default::default()
            });
            let joined = w.join(" ");
            assert!(joined.contains("MAX_TOKENS"));
            assert!(joined.contains("24000"));
            assert!(joined.contains("thinking"));
        }

        #[test]
        fn includes_safety_ratings_and_finish_message() {
            let w = build_empty_response_warnings(&ResponseDiagnostics {
                candidate_count: 1,
                finish_reason: Some("SAFETY".into()),
                safety_ratings: Some(vec![SafetyRatingLite {
                    category: Some("HARM_CATEGORY_HARASSMENT".into()),
                    probability: Some("HIGH".into()),
                }]),
                finish_message: Some("blocked by safety".into()),
                ..Default::default()
            });
            let joined = w.join(" ");
            assert!(joined.contains("HARM_CATEGORY_HARASSMENT:HIGH"));
            assert!(joined.contains("blocked by safety"));
        }

        #[test]
        fn unknown_cause_when_no_reason() {
            let w = build_empty_response_warnings(&ResponseDiagnostics {
                candidate_count: 1,
                ..Default::default()
            });
            assert!(w.join(" ").contains("unknown cause"));
        }
    }
}
