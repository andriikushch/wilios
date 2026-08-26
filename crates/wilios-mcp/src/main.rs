use anyhow::Result;
use rmcp::{
    ErrorData, RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ContentBlock, Implementation, InitializeResult, ListResourcesResult,
        PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse,
        ReadResourceResult, Resource, ResourceContents, ServerCapabilities,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, JsonSchema)]
struct SymbolDoc {
    name: String,
    kind: String,
    signature: Option<String>,
    category: Option<String>,
    doc: String,
    example: String,
}

impl From<wilios_core::stdlib::Symbol> for SymbolDoc {
    fn from(s: wilios_core::stdlib::Symbol) -> Self {
        SymbolDoc {
            name: s.name.to_string(),
            kind: s.kind.to_string(),
            signature: s.signature.map(str::to_string),
            category: s.category.map(str::to_string),
            doc: s.doc.to_string(),
            example: s.example.to_string(),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DescribeSymbolRequest {
    /// Exact name of a wilios stdlib symbol (built-in function or FM preset), e.g. "transpose" or "epiano".
    name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchStdlibRequest {
    /// Case-insensitive substring to match against stdlib symbol names and descriptions.
    query: String,
}

#[derive(Clone)]
struct WiliosMcp {
    tool_router: ToolRouter<Self>,
}

impl WiliosMcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router(router = tool_router)]
impl WiliosMcp {
    #[tool(
        description = "Look up a wilios stdlib symbol (built-in function or FM preset) by exact name. Returns its signature (if a function), description, and a minimal runnable example."
    )]
    async fn describe_symbol(
        &self,
        Parameters(req): Parameters<DescribeSymbolRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        match wilios_core::stdlib::find(&req.name) {
            Some(symbol) => Ok(CallToolResult::success(vec![ContentBlock::json(
                SymbolDoc::from(symbol),
            )?])),
            None => {
                let suggestions: Vec<String> = wilios_core::stdlib::search(&req.name)
                    .into_iter()
                    .take(3)
                    .map(|s| s.name.to_string())
                    .collect();
                let mut message = format!("No stdlib symbol named '{}'.", req.name);
                if suggestions.is_empty() {
                    message.push_str(" Try search_stdlib to browse available symbols.");
                } else {
                    message.push_str(&format!(
                        " Did you mean: {}? Or try search_stdlib to browse further.",
                        suggestions.join(", ")
                    ));
                }
                Ok(CallToolResult::error(vec![ContentBlock::text(message)]))
            }
        }
    }

    #[tool(
        description = "Search wilios stdlib symbols (built-in functions and FM presets) by a case-insensitive substring match against name and description."
    )]
    async fn search_stdlib(
        &self,
        Parameters(req): Parameters<SearchStdlibRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let matches: Vec<SymbolDoc> = wilios_core::stdlib::search(&req.query)
            .into_iter()
            .map(SymbolDoc::from)
            .collect();
        Ok(CallToolResult::success(vec![ContentBlock::json(matches)?]))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for WiliosMcp {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        InitializeResult::new(
            ServerCapabilities::builder()
                .enable_resources()
                .enable_tools()
                .build(),
        )
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
    let service = WiliosMcp::new().serve(transport).await?;
    service.waiting().await?;
    Ok(())
}
