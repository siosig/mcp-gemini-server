//! MCP server: registers all 8 Gemini tools via the rmcp `#[tool_router]` macro and
//! implements [`ServerHandler`] (tool listing + dispatch) via `#[tool_handler]`.

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler};

use crate::services::gemini_client::GeminiClient;
use crate::tools::analyze_media::{handle_analyze_media, AnalyzeMediaParams};
use crate::tools::custom_agent::{handle_custom_agent, CustomAgentParams};
use crate::tools::execute_code::{handle_execute_code, ExecuteCodeParams};
use crate::tools::gemini_chat::{handle_gemini_chat, GeminiChatParams};
use crate::tools::generate_image::{handle_generate_image, GenerateImageParams};
use crate::tools::google_search::{handle_google_search, GoogleSearchParams};
use crate::tools::manage_files::{handle_manage_files, ManageFilesParams};
use crate::tools::team::{handle_team, TeamParams};
use crate::tools::into_call_result;

#[derive(Clone)]
pub struct GeminiServer {
    client: GeminiClient,
    tool_router: ToolRouter<GeminiServer>,
}

#[tool_router]
impl GeminiServer {
    pub fn new(client: GeminiClient) -> Self {
        GeminiServer {
            client,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Chat with Gemini. Supports thinking levels, grounding, and JSON mode.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn gemini_chat(
        &self,
        params: Parameters<GeminiChatParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(into_call_result(handle_gemini_chat(&self.client, params.0).await))
    }

    #[tool(
        description = "Search the web via Google using Gemini Grounding.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn gemini_search(
        &self,
        params: Parameters<GoogleSearchParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(into_call_result(handle_google_search(&self.client, params.0).await))
    }

    #[tool(
        description = "Run a task with a specialized agent role. REQUIRED: task (string), role (string — e.g. \"architect\" | \"reviewer\" | \"developer\" | \"analyst\" | \"critic\" | \"summarizer\" | \"researcher\"). Any free-form role string is also accepted.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn gemini_custom_agent(
        &self,
        params: Parameters<CustomAgentParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(into_call_result(handle_custom_agent(&self.client, params.0).await))
    }

    #[tool(
        description = "Analyze images, PDF, video, or audio using Gemini vision.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn gemini_analyze_media(
        &self,
        params: Parameters<AnalyzeMediaParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(into_call_result(handle_analyze_media(&self.client, params.0).await))
    }

    #[tool(
        description = "Generate a single image via Gemini Flash Image (Nano Banana 2, model fixed to gemini-3.1-flash-image-preview) and save as PNG. All generated images carry SynthID watermarking by Google.",
        annotations(read_only_hint = false, destructive_hint = false, open_world_hint = true)
    )]
    async fn gemini_generate_image(
        &self,
        params: Parameters<GenerateImageParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(into_call_result(handle_generate_image(&self.client, params.0).await))
    }

    #[tool(
        description = "Execute Python code in Gemini's sandbox (numpy, pandas, matplotlib available).",
        annotations(read_only_hint = false, destructive_hint = false, open_world_hint = true)
    )]
    async fn gemini_execute_code(
        &self,
        params: Parameters<ExecuteCodeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(into_call_result(handle_execute_code(&self.client, params.0).await))
    }

    #[tool(
        description = "Manage files in Gemini (upload, list, status, delete). Files stored 48h, up to 2GB.",
        annotations(read_only_hint = false, destructive_hint = true, open_world_hint = true)
    )]
    async fn gemini_manage_files(
        &self,
        params: Parameters<ManageFilesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(into_call_result(handle_manage_files(&self.client, params.0).await))
    }

    #[tool(
        description = "Run a multi-agent team task entirely server-side. Reads local files, runs specialist agents, and returns only the final result. REQUIRED: task (string), mode (\"mul\" | \"it\" | \"mulit\").",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn gemini_team(
        &self,
        params: Parameters<TeamParams>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(into_call_result(handle_team(&self.client, params.0).await))
    }
}

#[tool_handler]
impl ServerHandler for GeminiServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "mcp-gemini-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            instructions: None,
        }
    }
}
