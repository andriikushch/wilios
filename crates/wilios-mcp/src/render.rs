//! The `render` MCP tool: runs a wilios source (inline or by path), synthesises
//! it offline, and returns the artifacts an agent can actually inspect — the
//! WAV, a waveform PNG, a log-frequency spectrogram PNG, and a compact scalar
//! analysis block.
//!
//! Mirrors `dump_events`: `source` xor `path`, sandbox via `resolve_import_path`,
//! a 1 MB source cap, `spawn_blocking` + a wall-clock timeout, and an `max_ms`
//! composition-time budget clamped to `[1000, 600000]`. A non-terminating piece
//! that still emits audio comes back as a *successful* result with
//! `finished: false` and output truncated at the bound; `isError` is reserved
//! for bad args, an unreadable path, a sandbox escape, or the 30 s timeout.
//!
//! Audio and images are returned as `wilios://render/<id>.…` resource links by
//! default (read them back with `resources/read`); pass `inline: true` to get
//! base64 blocks instead, for payloads under the inline cap.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use rmcp::model::{CallToolResult, ContentBlock, Resource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use wilios_core::interpreter::interpreter::Interpreter;
use wilios_core::lexer::{Lexer, Token};
use wilios_core::parser::parser::{Parser, resolve_import_path};
use wilios_render::analysis::{RenderAnalysis, analyze};
use wilios_render::image::{self, SPECTROGRAM_H, SPECTROGRAM_W, WAVEFORM_H, WAVEFORM_W};
use wilios_render::render::{RenderOpts, encode_wav_bytes, render_to_samples};

const MAX_SOURCE_BYTES: usize = 1_000_000;
const WALL_CLOCK_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MAX_MS: u64 = 60_000;
const MIN_MAX_MS: u64 = 1_000;
const MAX_MAX_MS: u64 = 600_000;
const DEFAULT_SAMPLE_RATE: u32 = 44_100;
const MIN_SAMPLE_RATE: u32 = 8_000;
const MAX_SAMPLE_RATE: u32 = 96_000;
/// Above this, an artifact is always a resource link even with `inline: true`.
const INLINE_MAX_BYTES: usize = 256 * 1024;
/// How many recent renders the server keeps readable via `resources/read`.
const MAX_RENDERS: usize = 8;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn default_max_ms() -> u64 {
    DEFAULT_MAX_MS
}
fn default_sample_rate() -> u32 {
    DEFAULT_SAMPLE_RATE
}

/// One renderable output.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Artifact {
    /// The rendered WAV (16-bit stereo PCM).
    Audio,
    /// A waveform-overview PNG.
    Waveform,
    /// A log-frequency spectrogram PNG (~20 Hz to Nyquist).
    Spectrogram,
    /// The scalar analysis block (level, clipping, silence, per-track notes).
    Analysis,
}

fn all_artifacts() -> Vec<Artifact> {
    vec![
        Artifact::Audio,
        Artifact::Waveform,
        Artifact::Spectrogram,
        Artifact::Analysis,
    ]
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenderRequest {
    /// Inline wilios source to render. Mutually exclusive with `path`.
    #[serde(default)]
    source: Option<String>,
    /// Path to a `.wilios` file, relative to the sandbox root. Mutually
    /// exclusive with `source`.
    #[serde(default)]
    path: Option<String>,
    /// Composition-time budget in milliseconds (default 60000, clamped to
    /// [1000, 600000]). A piece that has not finished within this many ms of
    /// its own timeline comes back with `finished: false` and audio/images
    /// truncated at the bound — a successful call, not an error.
    #[serde(default = "default_max_ms")]
    max_ms: u64,
    /// Output sample rate in Hz (default 44100, clamped to [8000, 96000]).
    #[serde(default = "default_sample_rate")]
    sample_rate: u32,
    /// Which artifacts to produce. Omit for all four. Ask for a subset (e.g.
    /// ["analysis", "spectrogram"]) to skip the large audio payload.
    #[serde(default)]
    want: Option<Vec<Artifact>>,
    /// Return audio/images as inline base64 blocks instead of
    /// `wilios://render/<id>.…` resource links, when each is under the inline
    /// size cap (256 KiB). Default false.
    #[serde(default)]
    inline: bool,
}

fn tool_error(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

/// Bytes of one stored render, keyed by `<id>` (see [`RenderStore::blob`]).
#[derive(Default)]
struct Stored {
    wav: Option<Vec<u8>>,
    waveform_png: Option<Vec<u8>>,
    spectrogram_png: Option<Vec<u8>>,
}

/// A small bounded in-memory cache of recent renders, so the `wilios://render/…`
/// resource links a `render` call hands back can be read via `resources/read`.
#[derive(Clone, Default)]
pub struct RenderStore {
    inner: Arc<Mutex<VecDeque<(String, Stored)>>>,
}

impl RenderStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn insert(&self, id: String, stored: Stored) {
        let mut q = self.inner.lock().expect("render store poisoned");
        q.push_back((id, stored));
        while q.len() > MAX_RENDERS {
            q.pop_front();
        }
    }

    /// Resolve the tail of a `wilios://render/<name>` URI to `(bytes, mime)`.
    /// `name` is `<id>.wav`, `<id>-waveform.png`, or `<id>-spectrogram.png`.
    pub fn blob(&self, name: &str) -> Option<(Vec<u8>, &'static str)> {
        let q = self.inner.lock().expect("render store poisoned");
        let get = |id: &str| q.iter().find(|(sid, _)| sid == id).map(|(_, s)| s);

        if let Some(id) = name.strip_suffix("-waveform.png") {
            get(id)?.waveform_png.clone().map(|b| (b, "image/png"))
        } else if let Some(id) = name.strip_suffix("-spectrogram.png") {
            get(id)?.spectrogram_png.clone().map(|b| (b, "image/png"))
        } else if let Some(id) = name.strip_suffix(".wav") {
            get(id)?.wav.clone().map(|b| (b, "audio/wav"))
        } else {
            None
        }
    }
}

#[derive(Debug, Default, Serialize)]
struct ResourceUris {
    #[serde(skip_serializing_if = "Option::is_none")]
    audio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    waveform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spectrogram: Option<String>,
}

#[derive(Debug, Serialize)]
struct RenderResult {
    /// `false` if synthesis stopped at `max_ms` rather than the piece ending.
    finished: bool,
    /// `true` if the source calls `rand(...)`, so two renders may differ (there
    /// is no seeded-RNG mode yet).
    used_rng: bool,
    sample_rate: u32,
    duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    analysis: Option<RenderAnalysis>,
    resources: ResourceUris,
}

struct RenderOutcome {
    wav: Option<Vec<u8>>,
    waveform_png: Option<Vec<u8>>,
    spectrogram_png: Option<Vec<u8>>,
    analysis: Option<RenderAnalysis>,
    finished: bool,
    used_rng: bool,
    duration_ms: u64,
    sample_rate: u32,
}

pub async fn handle(
    store: &RenderStore,
    req: RenderRequest,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let (source, path) = (req.source, req.path);
    match (&source, &path) {
        (Some(_), Some(_)) => {
            return Ok(tool_error(
                "exactly one of `source` or `path` must be given, not both",
            ));
        }
        (None, None) => {
            return Ok(tool_error(
                "exactly one of `source` or `path` must be given",
            ));
        }
        _ => {}
    }

    let project_root = match std::env::current_dir().and_then(|d| d.canonicalize()) {
        Ok(p) => p,
        Err(e) => return Ok(tool_error(format!("cannot resolve sandbox root: {e}"))),
    };

    let (text, base_dir, entry_canonical) = if let Some(source) = source {
        (source, project_root.clone(), None)
    } else {
        let path = path.expect("path is Some — checked above");
        let canonical = match resolve_import_path(&path, Some(&project_root), &project_root) {
            Ok(p) => p,
            Err(e) => return Ok(tool_error(format!("cannot resolve path '{path}': {e}"))),
        };
        let text = match std::fs::read_to_string(&canonical) {
            Ok(t) => t,
            Err(e) => return Ok(tool_error(format!("cannot read '{path}': {e}"))),
        };
        let base_dir = canonical
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| project_root.clone());
        (text, base_dir, Some(canonical))
    };

    if text.len() > MAX_SOURCE_BYTES {
        return Ok(tool_error(format!(
            "source exceeds the {MAX_SOURCE_BYTES}-byte limit ({} bytes)",
            text.len()
        )));
    }

    let max_ms = req.max_ms.clamp(MIN_MAX_MS, MAX_MAX_MS);
    let sample_rate = req.sample_rate.clamp(MIN_SAMPLE_RATE, MAX_SAMPLE_RATE);
    let want = req.want.unwrap_or_else(all_artifacts);
    let inline = req.inline;

    let want_task = want.clone();
    let task = tokio::task::spawn_blocking(move || {
        run_render(
            &text,
            base_dir,
            entry_canonical,
            max_ms,
            sample_rate,
            &want_task,
        )
    });

    let outcome = match tokio::time::timeout(WALL_CLOCK_TIMEOUT, task).await {
        Ok(Ok(res)) => res,
        Ok(Err(join_err)) => return Ok(tool_error(format!("render task failed: {join_err}"))),
        Err(_) => return Ok(tool_error("render timed out after 30s (timeout)")),
    };

    let out = match outcome {
        Ok(out) => out,
        Err(msg) => return Ok(tool_error(msg)),
    };

    Ok(build_result(store, out, inline))
}

fn build_result(store: &RenderStore, out: RenderOutcome, inline: bool) -> CallToolResult {
    let id = format!("{:016x}", NEXT_ID.fetch_add(1, Ordering::Relaxed));
    store.insert(
        id.clone(),
        Stored {
            wav: out.wav.clone(),
            waveform_png: out.waveform_png.clone(),
            spectrogram_png: out.spectrogram_png.clone(),
        },
    );

    let mut blocks: Vec<ContentBlock> = Vec::new();
    let mut uris = ResourceUris::default();

    let emit = |blocks: &mut Vec<ContentBlock>,
                bytes: &Option<Vec<u8>>,
                suffix: &str,
                mime: &'static str,
                name: &str|
     -> Option<String> {
        let bytes = bytes.as_ref()?;
        let uri = format!("wilios://render/{id}{suffix}");
        if inline && bytes.len() <= INLINE_MAX_BYTES {
            let b64 = BASE64.encode(bytes);
            blocks.push(if mime.starts_with("audio/") {
                ContentBlock::audio(b64, mime)
            } else {
                ContentBlock::image(b64, mime)
            });
        } else {
            blocks.push(ContentBlock::resource_link(
                Resource::new(uri.clone(), name.to_string())
                    .with_mime_type(mime)
                    .with_size(bytes.len() as u64),
            ));
        }
        Some(uri)
    };

    uris.audio = emit(&mut blocks, &out.wav, ".wav", "audio/wav", "render.wav");
    uris.waveform = emit(
        &mut blocks,
        &out.waveform_png,
        "-waveform.png",
        "image/png",
        "waveform.png",
    );
    uris.spectrogram = emit(
        &mut blocks,
        &out.spectrogram_png,
        "-spectrogram.png",
        "image/png",
        "spectrogram.png",
    );

    let summary = RenderResult {
        finished: out.finished,
        used_rng: out.used_rng,
        sample_rate: out.sample_rate,
        duration_ms: out.duration_ms,
        analysis: out.analysis,
        resources: uris,
    };
    match ContentBlock::json(summary) {
        Ok(json) => blocks.insert(0, json),
        Err(e) => return tool_error(format!("failed to serialize render summary: {e}")),
    }

    CallToolResult::success(blocks)
}

/// Lex → parse → interpret → synthesise, all synchronous. Compile failures are
/// tool-level errors (use `validate` for diagnostics); a non-terminating piece
/// that still produces audio is a normal result with `finished: false`.
fn run_render(
    text: &str,
    base_dir: PathBuf,
    entry_canonical: Option<PathBuf>,
    max_ms: u64,
    sample_rate: u32,
    want: &[Artifact],
) -> Result<RenderOutcome, String> {
    let (interp, used_rng) = compile(text, &base_dir, &entry_canonical)?;

    let opts = RenderOpts {
        out: PathBuf::from("render.wav"), // unused: render_to_samples writes nothing
        sample_rate,
        duration: None,
        max_render_secs: max_ms as f32 / 1000.0,
    };
    let samples = render_to_samples(interp, &opts)?;

    let analysis = if want.contains(&Artifact::Analysis) {
        // A second pass for the event timeline (mirrors `dump_events`); the
        // audio path consumes its interpreter and does not surface events.
        let (mut interp2, _) = compile(text, &base_dir, &entry_canonical)?;
        let (events, _) = interp2
            .schedule_to_end(max_ms)
            .map_err(|e| format!("runtime error while scheduling: {}", e.0))?;
        Some(analyze(&samples, &events, samples.finished))
    } else {
        None
    };

    let wav = want
        .contains(&Artifact::Audio)
        .then(|| encode_wav_bytes(&samples))
        .transpose()?;
    let waveform_png = want
        .contains(&Artifact::Waveform)
        .then(|| image::waveform_png(&samples, WAVEFORM_W, WAVEFORM_H))
        .transpose()?;
    let spectrogram_png = want
        .contains(&Artifact::Spectrogram)
        .then(|| image::spectrogram_png(&samples, SPECTROGRAM_W, SPECTROGRAM_H))
        .transpose()?;

    Ok(RenderOutcome {
        wav,
        waveform_png,
        spectrogram_png,
        analysis,
        finished: samples.finished,
        used_rng,
        duration_ms: samples.duration_ms(),
        sample_rate: samples.sample_rate,
    })
}

fn compile(
    text: &str,
    base_dir: &Path,
    entry_canonical: &Option<PathBuf>,
) -> Result<(Interpreter, bool), String> {
    let tokens = Lexer::new(text)
        .lex()
        .map_err(|e| format!("source did not compile: {e} — use `validate` for diagnostics"))?;

    let used_rng = tokens
        .iter()
        .any(|s| matches!(&s.token, Token::Ident(name) if name == "rand"));

    let loaded: HashSet<PathBuf> = entry_canonical.clone().into_iter().collect();
    let program = Parser::new_with_context(tokens, Some(base_dir.to_path_buf()), loaded)
        .parse()
        .map_err(|e| format!("source did not compile: {e} — use `validate` for diagnostics"))?;

    let interp = Interpreter::new(program).map_err(|e| {
        format!(
            "source did not compile: {} — use `validate` for diagnostics",
            e.0
        )
    })?;

    Ok((interp, used_rng))
}
