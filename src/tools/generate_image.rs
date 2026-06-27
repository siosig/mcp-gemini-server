//! `gemini_generate_image` — Flash Image (Nano Banana 2) generation, saved as PNG.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::Deserialize;
use serde_json::json;

use crate::schemas::{resolve_service_tier, ServiceTierValue, ThinkingLevel};
use crate::services::gemini_client::{
    default_image_thinking_level, GenerateImageOptions, GeminiClient, ThinkingSetting, IMAGE_MODEL,
};
use crate::tools::{ToolFailure, ToolResponse};
use crate::utils::diagnostics::build_empty_response_warnings;
use crate::utils::errors::GeminiError;

const ASPECT_RATIOS: &[&str] = &[
    "1:1", "2:3", "3:2", "3:4", "4:3", "4:5", "5:4", "9:16", "16:9", "21:9", "1:4", "4:1", "1:8",
    "8:1",
];
const IMAGE_SIZES: &[&str] = &["0.5K", "1K", "2K", "4K"];

const SYNTH_ID_WARNING: &str = "All generated images carry SynthID watermarking by Google.";

fn default_image_model() -> String {
    IMAGE_MODEL.to_string()
}
fn default_aspect_ratio() -> String {
    "1:1".to_string()
}
fn default_image_size() -> String {
    "1K".to_string()
}
fn default_file_prefix() -> String {
    "imagen".to_string()
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GenerateImageParams {
    pub prompt: String,

    #[serde(default = "default_image_model")]
    #[schemars(description = "[DEFAULT FIXED] gemini_generate_image is optimized for its purpose to run on gemini-3.1-flash-image-preview. Do not override unless there is a clear reason.")]
    pub model: String,

    #[serde(default = "default_aspect_ratio")]
    pub aspect_ratio: String,

    #[serde(default = "default_image_size")]
    pub image_size: String,

    #[serde(default = "default_image_thinking_level")]
    #[schemars(description = "[DEFAULT FIXED] The thinking depth of gemini_generate_image is optimized at medium. Do not override unless there is a clear reason. Values: minimal/low/medium/high.")]
    pub thinking_level: ThinkingLevel,

    #[serde(default)]
    pub output_dir: Option<String>,

    #[serde(default = "default_file_prefix")]
    pub file_prefix: String,

    #[serde(default)]
    pub service_tier: Option<ServiceTierValue>,
}

impl GenerateImageParams {
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
        if self.model != IMAGE_MODEL {
            return Err(format!("model must be {IMAGE_MODEL}"));
        }
        if !ASPECT_RATIOS.contains(&self.aspect_ratio.as_str()) {
            return Err(format!("aspect_ratio must be one of {ASPECT_RATIOS:?}"));
        }
        if !IMAGE_SIZES.contains(&self.image_size.as_str()) {
            return Err(format!("image_size must be one of {IMAGE_SIZES:?}"));
        }
        if let Some(dir) = &self.output_dir {
            if dir.is_empty() {
                return Err("output_dir must not be empty".to_string());
            }
        }
        if self.file_prefix.is_empty()
            || !self
                .file_prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err("Only filename-safe characters (A-Za-z0-9_-) are allowed".to_string());
        }
        Ok(())
    }
}

/// Resolve the output directory (creating it), priority: argument > `IMAGEN_OUTPUT_DIR`
/// > `<tmpdir>/mcp-gemini/imagen`.
pub fn resolve_output_dir(arg: Option<&str>) -> std::io::Result<PathBuf> {
    let dir = match arg {
        Some(a) => PathBuf::from(a),
        None => match std::env::var("IMAGEN_OUTPUT_DIR") {
            Ok(v) => PathBuf::from(v),
            Err(_) => std::env::temp_dir().join("mcp-gemini").join("imagen"),
        },
    };
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Collision-avoiding filename: `<prefix>-<index>.png`, with an 8-hex suffix appended
/// when that file already exists.
pub fn pick_unique_filename(dir: &Path, prefix: &str, index: u32) -> PathBuf {
    let base = dir.join(format!("{prefix}-{index}.png"));
    if !base.exists() {
        return base;
    }
    let suffix: String = uuid::Uuid::new_v4().simple().to_string().chars().take(8).collect();
    dir.join(format!("{prefix}-{index}-{suffix}.png"))
}

pub async fn handle_generate_image(
    client: &GeminiClient,
    params: GenerateImageParams,
) -> Result<ToolResponse, ToolFailure> {
    params.validate().map_err(ToolFailure::Message)?;

    let output_dir = resolve_output_dir(params.output_dir.as_deref())
        .map_err(|e| ToolFailure::from(GeminiError::Io(e)))?;

    let result = client
        .generate_image_content(GenerateImageOptions {
            prompt: params.prompt.clone(),
            model: params.model.clone(),
            aspect_ratio: params.aspect_ratio.clone(),
            image_size: params.image_size.clone(),
            thinking: ThinkingSetting::Level(params.thinking_level),
            service_tier: resolve_service_tier(params.service_tier),
        })
        .await?;

    let mut warnings: Vec<String> = Vec::new();

    let Some(image_bytes) = result.image_bytes else {
        warnings.extend(build_empty_response_warnings(&result.diagnostics));
        let payload = json!({
            "model": params.model,
            "prompt": params.prompt,
            "count": 0,
            "files": [],
            "text": result.text,
            "warnings": warnings,
        });
        return Ok(ToolResponse::with_structured(payload.to_string(), payload));
    };

    let file_path = pick_unique_filename(&output_dir, &params.file_prefix, 1);
    let mut files: Vec<serde_json::Value> = Vec::new();

    match base64::engine::general_purpose::STANDARD.decode(image_bytes.as_bytes()) {
        Ok(bytes) => match tokio::fs::write(&file_path, &bytes).await {
            Ok(()) => files.push(json!({
                "path": file_path.to_string_lossy(),
                "index": 1,
                "size_bytes": bytes.len(),
            })),
            Err(e) => warnings.push(format!("Failed to write image to {}: {e}", file_path.display())),
        },
        Err(e) => warnings.push(format!("Failed to decode image bytes: {e}")),
    }

    if files.is_empty() {
        return Err(ToolFailure::Message(format!(
            "Failed to write image file. warnings: {}",
            warnings.join(" | ")
        )));
    }

    warnings.push(SYNTH_ID_WARNING.to_string());

    let payload = json!({
        "model": params.model,
        "prompt": params.prompt,
        "count": files.len(),
        "files": files,
        "text": result.text,
        "warnings": warnings,
    });
    Ok(ToolResponse::with_structured(payload.to_string(), payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    mod schema {
        use super::*;

        #[test]
        fn requires_prompt_and_rejects_empty() {
            assert!(GenerateImageParams::parse(json!({})).is_err());
            assert!(GenerateImageParams::parse(json!({ "prompt": "" })).is_err());
        }

        #[test]
        fn applies_fixed_defaults() {
            let p = GenerateImageParams::parse(json!({ "prompt": "Robot" })).unwrap();
            assert_eq!(p.model, "gemini-3.1-flash-image-preview");
            assert_eq!(p.aspect_ratio, "1:1");
            assert_eq!(p.image_size, "1K");
            assert_eq!(p.thinking_level, ThinkingLevel::Medium);
            assert_eq!(p.file_prefix, "imagen");
        }

        #[test]
        fn rejects_invalid_models_with_explanatory_message() {
            for invalid in [
                "imagen-4.0-generate-001",
                "imagen-4.0-ultra-generate-001",
                "gemini-3-pro-image-preview",
                "gemini-3.5-flash",
                "foo",
            ] {
                let err = GenerateImageParams::parse(json!({ "prompt": "p", "model": invalid }))
                    .unwrap_err();
                assert!(err.contains("gemini-3.1-flash-image-preview"));
            }
        }

        #[test]
        fn accepts_only_allowed_model() {
            assert!(GenerateImageParams::parse(
                json!({ "prompt": "p", "model": "gemini-3.1-flash-image-preview" })
            )
            .is_ok());
        }

        #[test]
        fn rejects_out_of_enum_aspect_ratios() {
            for invalid in ["2:1", "1:2", "16:10", "6:5"] {
                assert!(GenerateImageParams::parse(json!({ "prompt": "p", "aspect_ratio": invalid }))
                    .is_err());
            }
        }

        #[test]
        fn accepts_all_14_aspect_ratios() {
            for ar in ASPECT_RATIOS {
                assert!(
                    GenerateImageParams::parse(json!({ "prompt": "p", "aspect_ratio": ar })).is_ok()
                );
            }
        }

        #[test]
        fn rejects_out_of_enum_image_sizes() {
            for invalid in ["8K", "HD", "3K", "0.25K"] {
                assert!(GenerateImageParams::parse(json!({ "prompt": "p", "image_size": invalid }))
                    .is_err());
            }
        }

        #[test]
        fn accepts_four_image_sizes() {
            for size in IMAGE_SIZES {
                assert!(
                    GenerateImageParams::parse(json!({ "prompt": "p", "image_size": size })).is_ok()
                );
            }
        }

        #[test]
        fn rejects_out_of_enum_thinking_level() {
            assert!(GenerateImageParams::parse(json!({ "prompt": "p", "thinking_level": "extra" }))
                .is_err());
        }

        #[test]
        fn accepts_four_thinking_levels() {
            for level in ["minimal", "low", "medium", "high"] {
                assert!(GenerateImageParams::parse(json!({ "prompt": "p", "thinking_level": level }))
                    .is_ok());
            }
        }

        #[test]
        fn rejects_removed_parameters() {
            assert!(GenerateImageParams::parse(json!({ "prompt": "p", "number_of_images": 2 })).is_err());
            assert!(GenerateImageParams::parse(json!({ "prompt": "p", "person_generation": "allow_adult" }))
                .is_err());
        }

        #[test]
        fn rejects_unsafe_file_prefixes() {
            for v in ["../etc", "foo bar", ""] {
                assert!(GenerateImageParams::parse(json!({ "prompt": "p", "file_prefix": v })).is_err());
            }
        }

        #[test]
        fn accepts_safe_file_prefixes() {
            for v in ["demo", "img_01", "my-prefix"] {
                assert!(GenerateImageParams::parse(json!({ "prompt": "p", "file_prefix": v })).is_ok());
            }
        }
    }

    mod pick_unique_filename {
        use super::*;

        #[test]
        fn returns_base_name_in_empty_dir() {
            let dir = tempfile::tempdir().unwrap();
            let p = pick_unique_filename(dir.path(), "demo", 1);
            assert_eq!(p, dir.path().join("demo-1.png"));
        }

        #[test]
        fn returns_suffixed_name_on_collision() {
            let dir = tempfile::tempdir().unwrap();
            let existing = dir.path().join("demo-1.png");
            std::fs::write(&existing, "x").unwrap();
            let p = pick_unique_filename(dir.path(), "demo", 1);
            assert_ne!(p, existing);
            let name = p.file_name().unwrap().to_str().unwrap();
            assert!(name.starts_with("demo-1-") && name.ends_with(".png"));
            let hex = &name["demo-1-".len()..name.len() - ".png".len()];
            assert_eq!(hex.len(), 8);
            assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    mod resolve_output_dir {
        use super::*;

        #[test]
        fn priority_arg_then_env_then_tmpdir() {
            let _lock = ENV_LOCK.lock().unwrap();
            let saved = std::env::var("IMAGEN_OUTPUT_DIR").ok();
            let tmp = tempfile::tempdir().unwrap();

            let arg_dir = tmp.path().join("from-arg");
            let env_dir = tmp.path().join("from-env");
            std::env::set_var("IMAGEN_OUTPUT_DIR", &env_dir);

            assert_eq!(resolve_output_dir(Some(arg_dir.to_str().unwrap())).unwrap(), arg_dir);
            assert_eq!(resolve_output_dir(None).unwrap(), env_dir);

            std::env::remove_var("IMAGEN_OUTPUT_DIR");
            assert_eq!(
                resolve_output_dir(None).unwrap(),
                std::env::temp_dir().join("mcp-gemini").join("imagen")
            );

            match saved {
                Some(v) => std::env::set_var("IMAGEN_OUTPUT_DIR", v),
                None => std::env::remove_var("IMAGEN_OUTPUT_DIR"),
            }
        }

        #[test]
        fn creates_nested_directory() {
            let tmp = tempfile::tempdir().unwrap();
            let target = tmp.path().join("deep").join("nest").join("dir");
            resolve_output_dir(Some(target.to_str().unwrap())).unwrap();
            assert!(target.is_dir());
        }
    }
}
