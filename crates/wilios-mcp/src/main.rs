use anyhow::Result;
use base64::Engine as _;
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

mod dump;
mod render;
mod validate;

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
    /// Recent `render` outputs, readable via `wilios://render/<id>.…`.
    renders: render::RenderStore,
}

impl WiliosMcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            renders: render::RenderStore::new(),
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

    #[tool(
        description = "Check a wilios source (inline or by path) without rendering or executing it. Returns structured diagnostics: spans, stable error codes, and \"did you mean\" suggestions for unknown identifiers. Invalid source is a successful call with `ok: false` in the result, not a tool error."
    )]
    async fn validate(
        &self,
        Parameters(req): Parameters<validate::ValidateRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        validate::handle(req).await
    }

    #[tool(
        description = "Run a wilios source (inline or by path) and return its per-track note-event timeline: onset (ms and exact beats), pitch spelling, MIDI note number and frequency, duration, velocity, pan, waveform, ADSR, and full FM operator config. Set `format` to `roll` for a compact ASCII piano roll (text) instead of the full JSON. Bounded by `max_ms` of composition time (default 60000) and a 5s wall-clock timeout; a piece that does not finish in that budget comes back with `finished: false` and truncated events — a successful call, not an error. Run `validate` first to confirm the source compiles."
    )]
    async fn dump_events(
        &self,
        Parameters(req): Parameters<dump::DumpRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        dump::handle(req).await
    }

    #[tool(
        description = "Render a wilios source (inline or by path) to audio and return what it sounds like: the WAV, a waveform PNG, a log-frequency spectrogram PNG, and a compact analysis block (peak/RMS dBFS, clipped-sample count, limiter ratio, leading/trailing silence, per-track note count and pitch range). Use `want` to pick a subset, e.g. [\"analysis\",\"spectrogram\"], and skip the large audio payload. Audio and images come back as `wilios://render/<id>.…` resource links (read them with resources/read); pass `inline: true` for base64 blocks under 256 KiB. Executes the interpreter, bounded by `max_ms` of composition time (default 60000, clamped [1000,600000]) and a 30s wall clock. A piece that keeps emitting notes past `max_ms` returns `finished: false` with truncated output — a successful call, not an error; `used_rng: true` flags a piece whose `rand(...)` calls make renders non-reproducible. Run `validate` first to confirm the source compiles."
    )]
    async fn render(
        &self,
        Parameters(req): Parameters<render::RenderRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        render::handle(&self.renders, req).await
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
                .with_description("14 FM synthesis presets: epiano, brass, trumpet, bass, upright, marimba, strings, comp_piano, kick, snare, hihat_c, hihat_o, ride, brushes")
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

        // Ephemeral render artifacts: `wilios://render/<id>.wav` etc., served
        // from the in-memory store a `render` call populated.
        if let Some(name) = uri.strip_prefix("wilios://render/") {
            return match self.renders.blob(name) {
                Some((bytes, mime)) => Ok(ReadResourceResult::new(vec![
                    ResourceContents::blob(
                        base64::engine::general_purpose::STANDARD.encode(bytes),
                        request.uri,
                    )
                    .with_mime_type(mime),
                ])
                .into()),
                None => Err(rmcp::ErrorData::resource_not_found(
                    format!("Unknown or expired render artifact: {uri}"),
                    None,
                )),
            };
        }

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
