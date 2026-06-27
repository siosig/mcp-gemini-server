//! mcp-gemini-server — MCP stdio server bridging Claude to the Gemini API.
//!
//! The MCP client spawns this process and communicates over stdin/stdout; all logs go
//! to stderr (stdout is reserved for the JSON-RPC protocol).

mod constants;
mod schemas;
mod server;
mod services;
mod tools;
mod utils;

use std::sync::Arc;

use rmcp::transport::stdio;
use rmcp::ServiceExt;

use crate::server::GeminiServer;
use crate::services::gemini_client::GeminiClient;
use crate::utils::config_file::load_config_file_into_env;
use crate::utils::env::{validate_env, EnvConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Minimal CLI handling so `--version` / `--help` do not fall through into the
    // stdio serve loop (which would block waiting on stdin).
    if let Some(arg) = std::env::args().nth(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("mcp-gemini-server {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "mcp-gemini-server {} — MCP stdio server for the Gemini API\n\nUsage: mcp-gemini-server\n  Reads MCP JSON-RPC from stdin, writes responses to stdout, logs to stderr.\n  Requires GEMINI_API_KEY in the environment or ~/.gemini-mcp.json.",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(());
            }
            _ => {}
        }
    }

    // Fill unset env vars from ~/.gemini-mcp.json (or $GEMINI_MCP_CONFIG) for clients
    // that do not propagate the .mcp.json env block. Real env vars always win.
    load_config_file_into_env();

    // Fail-Fast: exit(1) with a clear stderr message if the environment is invalid.
    if let Err(message) = validate_env() {
        eprintln!("[mcp-gemini-server] Environment validation failed: {message}");
        std::process::exit(1);
    }

    crate::utils::logger::init_tracing();

    let config = Arc::new(EnvConfig::from_env()?);
    let client = GeminiClient::new(config);
    let server = GeminiServer::new(client);

    tracing::info!(version = env!("CARGO_PKG_VERSION"), transport = "stdio", "mcp-gemini-server started");

    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
