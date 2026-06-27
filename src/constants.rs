//! Constants used throughout the application.

/// Response character limit to protect the LLM context.
pub const CHARACTER_LIMIT: usize = 25_000;

/// Message appended when the character limit is exceeded.
pub const TRUNCATION_SUFFIX: &str = "\n\n... (Response truncated because it exceeded the limit. Add filter conditions or use a more specific query.)";

/// Truncate the trailing portion of a response string if it exceeds [`CHARACTER_LIMIT`].
///
/// Length is measured in Unicode scalar values (`char`s), mirroring the original
/// implementation's intent of bounding the payload handed back to the model.
pub fn truncate(text: &str) -> String {
    if text.chars().count() <= CHARACTER_LIMIT {
        return text.to_string();
    }
    let head: String = text.chars().take(CHARACTER_LIMIT).collect();
    format!("{head}{TRUNCATION_SUFFIX}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_input_unchanged_when_within_limit() {
        assert_eq!(truncate("hello"), "hello");
    }

    #[test]
    fn appends_suffix_when_over_limit() {
        let long = "a".repeat(CHARACTER_LIMIT + 10);
        let out = truncate(&long);
        assert!(out.ends_with(TRUNCATION_SUFFIX));
        assert_eq!(out.chars().count(), CHARACTER_LIMIT + TRUNCATION_SUFFIX.chars().count());
    }
}
