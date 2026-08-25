use anyhow::Result;
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    model::{
        Implementation, InitializeResult, ListResourcesResult, PaginatedRequestParams,
        ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
        ResourceContents, ServerCapabilities,
    },
    service::RequestContext,
};

struct WiliosMcp;

impl ServerHandler for WiliosMcp {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        InitializeResult::new(ServerCapabilities::builder().enable_resources().build())
            .with_server_info(Implementation::new("wilios-mcp", env!("CARGO_PKG_VERSION")))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new("wilios://docs/language-reference", "Wilios Language Reference")
                .with_description("Complete language reference for the wilios music DSL")
                .with_mime_type("text/markdown"),
            Resource::new("wilios://docs/grammar", "Wilios Grammar (EBNF)")
                .with_description("Formal ISO 14977 EBNF grammar for the wilios DSL")
                .with_mime_type("text/plain"),
            Resource::new("wilios://lib/presets", "FM Preset Library")
                .with_description("9 FM synthesis presets: epiano, brass, bass, marimba, strings, kick, snare, hihat_c, hihat_o")
                .with_mime_type("text/plain"),
            Resource::new("wilios://examples/full-piece", "Multi-Track Composition Example")
                .with_description("4-track piece using import, func, loop, and multiple FM presets")
                .with_mime_type("text/plain"),
            Resource::new("wilios://examples/swing", "Swing/Feel Example")
                .with_description("Demonstrates swing parameter: straight (swing 50) vs swing feel (swing 90)")
                .with_mime_type("text/plain"),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, rmcp::ErrorData> {
        let uri = request.uri.as_str();
        let (text, mime) = match uri {
            "wilios://docs/language-reference" => (LANGUAGE_REFERENCE, "text/markdown"),
            "wilios://docs/grammar" => (GRAMMAR, "text/plain"),
            "wilios://lib/presets" => (LIB_PRESETS, "text/plain"),
            "wilios://examples/full-piece" => (EXAMPLE_FULL_PIECE, "text/plain"),
            "wilios://examples/swing" => (EXAMPLE_SWING, "text/plain"),
            _ => {
                return Err(rmcp::ErrorData::resource_not_found(
                    format!("Unknown resource: {uri}"),
                    None,
                ));
            }
        };

        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(text, request.uri).with_mime_type(mime),
        ])
        .into())
    }
}

// All resources embedded at compile time — binary works from any working directory
const LANGUAGE_REFERENCE: &str = include_str!("../../../doc/language-reference.md");
const GRAMMAR: &str = include_str!("../../../doc/grammar.ebnf");
const LIB_PRESETS: &str = include_str!("../../../lib/lib.wilios");
const EXAMPLE_FULL_PIECE: &str = include_str!("../../../examples/example_1.wilios");
const EXAMPLE_SWING: &str = include_str!("../../../examples/example_swing.wilios");

#[tokio::main]
async fn main() -> Result<()> {
    // CRITICAL: log to stderr only — stdout carries JSON-RPC messages
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let transport = rmcp::transport::io::stdio();
    let service = WiliosMcp.serve(transport).await?;
    service.waiting().await?;
    Ok(())
}
