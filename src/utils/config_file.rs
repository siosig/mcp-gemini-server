//! Config-file fallback for environment variables.
//!
//! Some MCP clients do not propagate the `env` block of `.mcp.json` to the spawned
//! process. To stay usable there, the server can read configuration (most importantly
//! `GEMINI_API_KEY`) from a JSON file in the user's home directory and use it to fill
//! in *unset* environment variables. Real environment variables ALWAYS win.
//!
//! Must run before env validation so Fail-Fast sees the fully-resolved environment.

use std::path::PathBuf;

/// Resolve the config-file path: `$GEMINI_MCP_CONFIG`, else `~/.gemini-mcp.json`.
pub fn resolve_config_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("GEMINI_MCP_CONFIG") {
        if !override_path.trim().is_empty() {
            return PathBuf::from(override_path);
        }
    }
    let home = dirs::home_dir().unwrap_or_default();
    home.join(".gemini-mcp.json")
}

/// Populate any unset environment variables from the config file.
///
/// Best-effort: a missing file is a silent no-op; read/parse problems produce a
/// warning and are otherwise ignored. Returns the warnings (also written to stderr)
/// so callers/tests can observe them.
pub fn load_config_file_into_env() -> Vec<String> {
    let warnings = load_inner();
    for w in &warnings {
        eprintln!("{w}");
    }
    warnings
}

fn load_inner() -> Vec<String> {
    let mut warnings = Vec::new();
    let path = resolve_config_path();

    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(err) => {
            // ENOENT (file absent) is the normal case — say nothing.
            if err.kind() != std::io::ErrorKind::NotFound {
                warnings.push(format!(
                    "[mcp-gemini-server] Could not read config file {}: {err}",
                    path.display()
                ));
            }
            return warnings;
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(err) => {
            warnings.push(format!(
                "[mcp-gemini-server] Ignoring malformed config file {}: {err}",
                path.display()
            ));
            return warnings;
        }
    };

    let serde_json::Value::Object(map) = parsed else {
        warnings.push(format!(
            "[mcp-gemini-server] Ignoring config file {}: expected a JSON object",
            path.display()
        ));
        return warnings;
    };

    for (key, value) in map {
        let serde_json::Value::String(value) = value else {
            warnings.push(format!(
                "[mcp-gemini-server] Skipping config key \"{key}\": value is not a string"
            ));
            continue;
        };
        let current = std::env::var(&key).ok();
        if current.as_deref().unwrap_or("").is_empty() {
            // SAFETY (edition 2021): set_var is safe; startup is single-threaded here.
            std::env::set_var(&key, &value);
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes env-mutating tests (the process environment is global state).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const TOUCHED: &[&str] = &["GEMINI_MCP_CONFIG", "GEMINI_API_KEY", "LOG_LEVEL", "GEMINI_TIMEOUT"];

    struct EnvGuard {
        saved: Vec<(String, Option<String>)>,
    }

    impl EnvGuard {
        fn new() -> Self {
            let saved = TOUCHED
                .iter()
                .map(|k| {
                    let prev = std::env::var(k).ok();
                    std::env::remove_var(k);
                    ((*k).to_string(), prev)
                })
                .collect();
            EnvGuard { saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, v) in &self.saved {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
    }

    fn write_config(dir: &std::path::Path, body: &str) -> PathBuf {
        let p = dir.join("config.json");
        std::fs::write(&p, body).unwrap();
        std::env::set_var("GEMINI_MCP_CONFIG", &p);
        p
    }

    #[test]
    fn resolve_uses_override_when_set() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new();
        std::env::set_var("GEMINI_MCP_CONFIG", "/custom/path.json");
        assert_eq!(resolve_config_path(), PathBuf::from("/custom/path.json"));
    }

    #[test]
    fn resolve_falls_back_when_override_empty() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new();
        std::env::set_var("GEMINI_MCP_CONFIG", "");
        assert!(resolve_config_path().ends_with(".gemini-mcp.json"));
    }

    #[test]
    fn absent_file_leaves_env_unchanged() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("GEMINI_MCP_CONFIG", dir.path().join("nope.json"));
        let w = load_config_file_into_env();
        assert!(w.is_empty());
        assert!(std::env::var("GEMINI_API_KEY").is_err());
    }

    #[test]
    fn valid_json_populates_unset_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new();
        let dir = tempfile::tempdir().unwrap();
        write_config(dir.path(), r#"{"GEMINI_API_KEY":"from-file","LOG_LEVEL":"debug"}"#);
        load_config_file_into_env();
        assert_eq!(std::env::var("GEMINI_API_KEY").unwrap(), "from-file");
        assert_eq!(std::env::var("LOG_LEVEL").unwrap(), "debug");
    }

    #[test]
    fn real_env_wins_over_file() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("GEMINI_API_KEY", "from-env");
        write_config(dir.path(), r#"{"GEMINI_API_KEY":"from-file"}"#);
        load_config_file_into_env();
        assert_eq!(std::env::var("GEMINI_API_KEY").unwrap(), "from-env");
    }

    #[test]
    fn empty_string_env_is_treated_as_unset() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("GEMINI_API_KEY", "");
        write_config(dir.path(), r#"{"GEMINI_API_KEY":"from-file"}"#);
        load_config_file_into_env();
        assert_eq!(std::env::var("GEMINI_API_KEY").unwrap(), "from-file");
    }

    #[test]
    fn malformed_json_warns_and_continues() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new();
        let dir = tempfile::tempdir().unwrap();
        write_config(dir.path(), "{ not valid json ");
        let w = load_config_file_into_env();
        assert!(std::env::var("GEMINI_API_KEY").is_err());
        assert!(!w.is_empty());
    }

    #[test]
    fn config_path_override_is_honored() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new();
        let dir = tempfile::tempdir().unwrap();
        let p = write_config(dir.path(), r#"{"GEMINI_API_KEY":"via-override"}"#);
        assert_eq!(resolve_config_path(), p);
        load_config_file_into_env();
        assert_eq!(std::env::var("GEMINI_API_KEY").unwrap(), "via-override");
    }

    #[test]
    fn non_string_values_are_skipped_with_warning() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new();
        let dir = tempfile::tempdir().unwrap();
        write_config(dir.path(), r#"{"GEMINI_API_KEY":"ok","GEMINI_TIMEOUT":360}"#);
        let w = load_config_file_into_env();
        assert_eq!(std::env::var("GEMINI_API_KEY").unwrap(), "ok");
        assert!(std::env::var("GEMINI_TIMEOUT").is_err());
        assert!(!w.is_empty());
    }

    #[test]
    fn non_object_json_array_is_ignored_with_warning() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::new();
        let dir = tempfile::tempdir().unwrap();
        write_config(dir.path(), r#"["a","b"]"#);
        let w = load_config_file_into_env();
        assert!(!w.is_empty());
    }
}
