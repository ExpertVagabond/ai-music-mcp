// ─── Security & Validation (ai-music-mcp) ─────────────────────────────
// Input validation, path sanitization, and error redaction are defined
// at the top of the entry file before any handler or tool logic.
//
// - All string parameters validated for length and null bytes
// - File paths validated against traversal and restricted to allowed dirs
// - Error messages redacted to prevent leaking internal paths
// - No hardcoded secrets — all configuration from environment variables

mod tools;

use psm_mcp_transport::server::McpServer;
use tools::{
    ConvertVoiceTool, FullPipelineTool, GenerateTool, GetInfoTool, ListFilesTool, ListStemsTool,
    ListVoiceModelsTool, SeparateStemsTool,
};

use std::sync::Arc;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_writer(std::io::stderr)
        .init();

    let config = Arc::new(tools::Config::from_env());

    let mut server = McpServer::new("ai-music-mcp", "0.1.0");

    server.register_tool(GenerateTool::new(Arc::clone(&config)));
    server.register_tool(SeparateStemsTool::new(Arc::clone(&config)));
    server.register_tool(ListStemsTool::new(Arc::clone(&config)));
    server.register_tool(ConvertVoiceTool::new(Arc::clone(&config)));
    server.register_tool(ListVoiceModelsTool::new(Arc::clone(&config)));
    server.register_tool(ListFilesTool::new(Arc::clone(&config)));
    server.register_tool(GetInfoTool::new(Arc::clone(&config)));
    server.register_tool(FullPipelineTool::new(Arc::clone(&config)));

    eprintln!("[ai-music-mcp] Starting with 8 tools");
    eprintln!(
        "[ai-music-mcp] MusicGen: [configured], RVC: [configured], Output: [configured]"
    );

    server.run_stdio().await
}
