//! `gemini_team` — server-side multi-agent orchestration (mul / it / mulit).

use std::time::Instant;

use futures::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::schemas::{
    de_bool_like, de_i64_like, resolve_service_tier, ServiceTierValue, ThinkingLevel,
};
use crate::services::gemini_client::{
    default_team_model, default_team_thinking_level, ChatOptions, GeminiClient, ThinkingSetting,
};
use crate::tools::{ToolFailure, ToolResponse};

const DEFAULT_ROLES: &[&str] = &["analyst", "architect", "developer", "reviewer", "critic"];

fn default_roles() -> Vec<String> {
    DEFAULT_ROLES.iter().map(|s| (*s).to_string()).collect()
}
fn default_max_iterations() -> i64 {
    2
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TeamMode {
    Mul,
    It,
    Mulit,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TeamParams {
    pub task: String,
    pub mode: TeamMode,

    #[serde(default)]
    pub file_paths: Option<Vec<String>>,

    #[serde(default = "default_roles")]
    pub roles: Vec<String>,

    #[serde(default = "default_max_iterations", deserialize_with = "de_i64_like")]
    pub max_iterations: i64,

    #[serde(default, deserialize_with = "de_bool_like")]
    pub intermediate_results: bool,

    #[serde(default = "default_team_model")]
    #[schemars(description = "[DEFAULT FIXED] gemini_team is optimized for its purpose to run on gemini-flash-latest. Do not override unless there is a clear reason (e.g. cost or a specific task's quality requirement).")]
    pub model: String,

    #[serde(default = "default_team_thinking_level")]
    #[schemars(description = "[DEFAULT FIXED] The thinking depth of gemini_team is optimized at high. Do not override unless there is a clear reason. Values: minimal/low/medium/high.")]
    pub thinking_level: ThinkingLevel,

    #[serde(default)]
    pub service_tier: Option<ServiceTierValue>,
}

impl TeamParams {
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
        if !(0..=10).contains(&self.max_iterations) {
            return Err("max_iterations must be between 0 and 10".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentResult {
    role: String,
    output: String,
    duration_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// ==================== Single-agent helper ====================

async fn team_agent(
    client: &GeminiClient,
    params: &TeamParams,
    tier: Option<ServiceTierValue>,
    system: String,
    temperature: f64,
    prompt: &str,
) -> Result<String, ToolFailure> {
    let outcome = client
        .chat(
            prompt,
            ChatOptions {
                model: Some(params.model.clone()),
                system_instruction: Some(system),
                temperature: Some(temperature),
                thinking: ThinkingSetting::Level(params.thinking_level),
                tool_name: "gemini_team".to_string(),
                service_tier: tier,
                ..Default::default()
            },
        )
        .await?;
    Ok(outcome.text)
}

// ==================== File context ====================

async fn build_file_context(file_paths: &[String]) -> Result<String, ToolFailure> {
    let mut parts = Vec::with_capacity(file_paths.len());
    for fp in file_paths {
        let content = tokio::fs::read_to_string(fp)
            .await
            .map_err(|e| ToolFailure::Message(format!("Failed to read {fp}: {e}")))?;
        let name = std::path::Path::new(fp)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(fp);
        parts.push(format!("--- File: {name} ({fp}) ---\n{content}"));
    }
    Ok(parts.join("\n\n"))
}

// ==================== Phase 1 ====================

async fn run_phase1_agents(
    client: &GeminiClient,
    params: &TeamParams,
    tier: Option<ServiceTierValue>,
    full_task: &str,
) -> Result<(Vec<AgentResult>, Vec<String>), ToolFailure> {
    let futures = params.roles.iter().map(|role| async move {
        let start = Instant::now();
        let system = format!(
            "You are a {role}. Apply your expertise to analyze the task and provide your perspective."
        );
        match team_agent(client, params, tier, system, 0.7, full_task).await {
            Ok(output) => AgentResult {
                role: role.clone(),
                output,
                duration_ms: start.elapsed().as_secs_f64() * 1000.0,
                error: None,
            },
            Err(failure) => AgentResult {
                role: role.clone(),
                output: String::new(),
                duration_ms: 0.0,
                error: Some(failure_message(&failure)),
            },
        }
    });

    let results: Vec<AgentResult> = join_all(futures).await;
    let failed_roles: Vec<String> = results
        .iter()
        .filter(|r| r.error.is_some())
        .map(|r| r.role.clone())
        .collect();

    if results.iter().all(|r| r.error.is_some()) {
        let last = results
            .last()
            .and_then(|r| r.error.clone())
            .unwrap_or_else(|| "all agents failed".to_string());
        return Err(ToolFailure::Message(format!(
            "gemini_team: All specialist agents failed. Last error: {last}"
        )));
    }

    Ok((results, failed_roles))
}

async fn aggregate_phase1(
    client: &GeminiClient,
    params: &TeamParams,
    tier: Option<ServiceTierValue>,
    agent_results: &[AgentResult],
    task: &str,
) -> Result<String, ToolFailure> {
    let combined = agent_results
        .iter()
        .filter(|r| r.error.is_none())
        .map(|r| format!("--- {} ---\n{}", r.role.to_uppercase(), r.output))
        .collect::<Vec<_>>()
        .join("\n\n");

    let prompt = format!("{combined}\n\n--- Original Task ---\n{task}");
    let system = "You are a coordinator. Synthesize the multiple specialist perspectives above into a unified recommendation. Identify consensus and explicitly flag any unresolved conflicts with each side's rationale.".to_string();

    team_agent(client, params, tier, system, 0.5, &prompt).await
}

// ==================== it mode ====================

async fn generate_initial_draft(
    client: &GeminiClient,
    params: &TeamParams,
    tier: Option<ServiceTierValue>,
    full_task: &str,
) -> Result<String, ToolFailure> {
    let system =
        "You are a writer. Generate an initial draft based on the following task description."
            .to_string();
    team_agent(client, params, tier, system, 0.7, full_task).await
}

fn extract_rubric_score(text: &str) -> Option<f64> {
    let lower = text.to_lowercase();
    let idx = ["overall", "average", "score"]
        .iter()
        .filter_map(|kw| lower.find(kw))
        .min()?;
    let rest = &lower.as_bytes()[idx..];
    let mut i = 0;
    while i < rest.len() && !rest[i].is_ascii_digit() {
        i += 1;
    }
    let start = i;
    while i < rest.len() && (rest[i].is_ascii_digit() || rest[i] == b'.') {
        i += 1;
    }
    std::str::from_utf8(&rest[start..i]).ok()?.parse::<f64>().ok()
}

async fn run_it_loop(
    client: &GeminiClient,
    params: &TeamParams,
    tier: Option<ServiceTierValue>,
    initial_draft: String,
    full_task: &str,
) -> Result<(String, i64), ToolFailure> {
    let mut draft = initial_draft;
    let mut actual_iterations = 0;

    for i in 1..=params.max_iterations {
        let critique = team_agent(
            client,
            params,
            tier,
            "You are a critic. Evaluate the draft against the task requirements. Provide specific, actionable feedback. End your response with: 'Overall score: X/5' where X is an average quality score (1-5).".to_string(),
            0.3,
            &format!("--- Draft ---\n{draft}\n\n--- Original Task ---\n{full_task}"),
        )
        .await?;

        actual_iterations = i;
        if let Some(score) = extract_rubric_score(&critique) {
            if score >= 4.0 {
                break;
            }
        }

        if i < params.max_iterations {
            draft = team_agent(
                client,
                params,
                tier,
                "You are a writer. Improve the draft based on the critic's feedback while preserving its strengths.".to_string(),
                0.7,
                &format!("--- Current Draft ---\n{draft}\n\n--- Critic Feedback ---\n{critique}\n\n--- Original Task ---\n{full_task}"),
            )
            .await?;
        }
    }

    Ok((draft, actual_iterations))
}

async fn run_it(
    client: &GeminiClient,
    params: &TeamParams,
    tier: Option<ServiceTierValue>,
    full_task: &str,
) -> Result<(String, i64), ToolFailure> {
    if params.max_iterations == 0 {
        let text = generate_initial_draft(client, params, tier, full_task).await?;
        return Ok((text, 0));
    }
    let initial = generate_initial_draft(client, params, tier, full_task).await?;
    run_it_loop(client, params, tier, initial, full_task).await
}

// ==================== mulit mode ====================

async fn run_mulit(
    client: &GeminiClient,
    params: &TeamParams,
    tier: Option<ServiceTierValue>,
    full_task: &str,
    original_task: &str,
) -> Result<(String, Vec<AgentResult>, Vec<String>, i64), ToolFailure> {
    // Phase1 specialists and a speculative initial draft run concurrently.
    let phase1 = run_phase1_agents(client, params, tier, full_task);
    let speculative = generate_initial_draft(client, params, tier, original_task);
    let (phase1_res, speculative_draft) = futures::future::join(phase1, speculative).await;

    let (agent_results, failed_roles) = phase1_res?;
    let speculative_draft = speculative_draft?;

    let aggregated = aggregate_phase1(client, params, tier, &agent_results, full_task).await?;
    let combined_task = format!("{aggregated}\n\n--- Original Task ---\n{original_task}");
    let (text, iterations) =
        run_it_loop(client, params, tier, speculative_draft, &combined_task).await?;

    Ok((text, agent_results, failed_roles, iterations))
}

// ==================== Handler ====================

pub async fn handle_team(
    client: &GeminiClient,
    params: TeamParams,
) -> Result<ToolResponse, ToolFailure> {
    params.validate().map_err(ToolFailure::Message)?;

    let tier = resolve_service_tier(params.service_tier);
    let start = Instant::now();

    let file_count = params.file_paths.as_ref().map_or(0, Vec::len);
    let file_context = match &params.file_paths {
        Some(paths) if !paths.is_empty() => build_file_context(paths).await?,
        _ => String::new(),
    };
    let full_task = if file_context.is_empty() {
        params.task.clone()
    } else {
        format!("{file_context}\n\n--- Task ---\n{}", params.task)
    };

    match params.mode {
        TeamMode::Mul => {
            let (results, failed_roles) =
                run_phase1_agents(client, &params, tier, &full_task).await?;
            let text = aggregate_phase1(client, &params, tier, &results, &full_task).await?;
            if !params.intermediate_results {
                return Ok(ToolResponse::text(text));
            }
            let metadata = team_metadata("mul", count_ok(&results), 0, file_count, &failed_roles, start);
            Ok(ToolResponse::with_structured(
                text,
                json!({ "phases": results, "metadata": metadata }),
            ))
        }
        TeamMode::It => {
            let (text, iterations) = run_it(client, &params, tier, &full_task).await?;
            if !params.intermediate_results {
                return Ok(ToolResponse::text(text));
            }
            let metadata = team_metadata("it", 0, iterations, file_count, &[], start);
            Ok(ToolResponse::with_structured(
                text,
                json!({ "phases": [], "metadata": metadata }),
            ))
        }
        TeamMode::Mulit => {
            let (text, results, failed_roles, iterations) =
                run_mulit(client, &params, tier, &full_task, &params.task).await?;
            if !params.intermediate_results {
                return Ok(ToolResponse::text(text));
            }
            let metadata =
                team_metadata("mulit", count_ok(&results), iterations, file_count, &failed_roles, start);
            Ok(ToolResponse::with_structured(
                text,
                json!({ "phases": results, "metadata": metadata }),
            ))
        }
    }
}

fn count_ok(results: &[AgentResult]) -> usize {
    results.iter().filter(|r| r.error.is_none()).count()
}

fn failure_message(failure: &ToolFailure) -> String {
    match failure {
        ToolFailure::Api(e) => e.to_string(),
        ToolFailure::Message(m) => m.clone(),
    }
}

fn team_metadata(
    mode: &str,
    agent_count: usize,
    iterations: i64,
    file_count: usize,
    failed_agents: &[String],
    start: Instant,
) -> serde_json::Value {
    json!({
        "mode": mode,
        "agentCount": agent_count,
        "iterations": iterations,
        "fileCount": file_count,
        "failedAgents": failed_agents,
        "totalDurationMs": start.elapsed().as_secs_f64() * 1000.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(v: serde_json::Value) -> Result<TeamParams, String> {
        TeamParams::parse(v)
    }

    #[test]
    fn mul_defaults_applied() {
        let r = parse(json!({ "task": "analyze this", "mode": "mul" })).unwrap();
        assert_eq!(r.model, "gemini-flash-latest");
        assert_eq!(r.thinking_level, ThinkingLevel::High);
        assert_eq!(r.roles, vec!["analyst", "architect", "developer", "reviewer", "critic"]);
        assert_eq!(r.max_iterations, 2);
        assert!(!r.intermediate_results);
    }

    #[test]
    fn it_defaults_applied() {
        let r = parse(json!({ "task": "write a proposal", "mode": "it" })).unwrap();
        assert_eq!(r.max_iterations, 2);
        assert!(!r.intermediate_results);
    }

    #[test]
    fn mulit_mode_parses() {
        let r = parse(json!({ "task": "design this", "mode": "mulit" })).unwrap();
        assert_eq!(r.mode, TeamMode::Mulit);
    }

    #[test]
    fn strict_rejects_unknown_fields() {
        assert!(parse(json!({ "task": "x", "mode": "mul", "unknown_field": true })).is_err());
    }

    #[test]
    fn invalid_mode_rejected() {
        assert!(parse(json!({ "task": "x", "mode": "invalid" })).is_err());
    }

    #[test]
    fn valid_modes_accepted() {
        for mode in ["mul", "it", "mulit"] {
            assert!(parse(json!({ "task": "x", "mode": mode })).is_ok());
        }
    }

    #[test]
    fn max_iterations_bounds() {
        assert_eq!(parse(json!({ "task": "x", "mode": "it", "max_iterations": 0 })).unwrap().max_iterations, 0);
        assert_eq!(parse(json!({ "task": "x", "mode": "it", "max_iterations": 10 })).unwrap().max_iterations, 10);
        assert!(parse(json!({ "task": "x", "mode": "it", "max_iterations": 11 })).is_err());
        assert!(parse(json!({ "task": "x", "mode": "it", "max_iterations": -1 })).is_err());
    }

    #[test]
    fn intermediate_results_bool_and_string() {
        assert!(parse(json!({ "task": "x", "mode": "mul", "intermediate_results": true })).unwrap().intermediate_results);
        assert!(parse(json!({ "task": "x", "mode": "mul", "intermediate_results": "true" })).unwrap().intermediate_results);
        assert!(!parse(json!({ "task": "x", "mode": "mul" })).unwrap().intermediate_results);
    }

    #[test]
    fn roles_override() {
        let r = parse(json!({ "task": "x", "mode": "mul", "roles": ["security_expert", "performance_engineer"] })).unwrap();
        assert_eq!(r.roles, vec!["security_expert", "performance_engineer"]);
    }

    #[test]
    fn file_paths_optional() {
        let with = parse(json!({ "task": "x", "mode": "mul", "file_paths": ["/a.md", "/b.md"] })).unwrap();
        assert_eq!(with.file_paths.unwrap(), vec!["/a.md", "/b.md"]);
        let without = parse(json!({ "task": "x", "mode": "mul" })).unwrap();
        assert!(without.file_paths.is_none());
    }

    #[test]
    fn service_tier_accepts_flex() {
        let r = parse(json!({ "task": "x", "mode": "mul", "service_tier": "flex" })).unwrap();
        assert_eq!(r.service_tier, Some(ServiceTierValue::Flex));
    }

    #[test]
    fn task_must_not_be_empty() {
        assert!(parse(json!({ "task": "", "mode": "mul" })).is_err());
    }

    #[test]
    fn descriptions_have_default_fixed_marker() {
        let schema = serde_json::to_value(schemars::schema_for!(TeamParams)).unwrap();
        for field in ["model", "thinking_level"] {
            assert!(schema["properties"][field]["description"]
                .as_str()
                .unwrap_or_default()
                .contains("[DEFAULT FIXED]"));
        }
    }
}
