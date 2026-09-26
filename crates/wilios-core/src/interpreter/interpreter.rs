use std::collections::HashMap;

use rand::RngExt;

use crate::{
    interpreter::{
        event::{Event, EventKind, FmBlockConfig, FmOpConfig, TrackId},
        frame::Frame,
        tempo::Tempo,
    },
    parser::{
        ast::{
            BinaryOp, Duration, Expr, FmOperator, Ident, Pitch, Stmt, TimeSignature, UnaryOp,
            Waveform,
        },
        parser::{Program, TrackAst},
    },
    time::{self, Beats, TempoHistory},
};

/// Maximum nested depth of synchronous (expression-position) user-function
/// calls. `Expr::Call` evaluates a `func` body via `eval_body_sync`, which
/// recurses on the native stack; past this bound we return a `RuntimeError`
/// rather than overflow the process. Kept well below the point where the
/// (large, in debug builds) `eval` stack frames would exhaust a small thread
/// stack — musical phrase nesting is realistically shallow. The
/// statement-position call path is bounded separately by `MAX_STEPS` in
/// `schedule_until`.
const MAX_CALL_DEPTH: usize = 128;

#[derive(Debug)]
pub struct RuntimeError(pub String);

impl From<time::TimeError> for RuntimeError {
    fn from(e: time::TimeError) -> Self {
        RuntimeError(e.to_string())
    }
}

#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Float(f32),
    Bool(bool),
    Func { params: Vec<Ident>, body: Vec<Stmt> },
    Pitch(Pitch),
    Chord(Vec<Pitch>),
    Builtin(fn(Vec<Value>) -> Result<Value, RuntimeError>),
    Array(Vec<Value>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Pitch(a), Value::Pitch(b)) => a == b,
            (Value::Chord(a), Value::Chord(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            _ => false,
        }
    }
}

#[derive(Clone)]
pub struct TrackContext {
    pub track_id: TrackId,
    /// Authoritative ms position, derived fresh from `nominal_position` each step (not accumulated).
    pub time: u64,
    /// Exact nominal position (whole-note units) since track start.
    pub nominal_position: Beats,
    pub bar_epoch_beats: Beats,
    pub tempo_history: TempoHistory,
    pub pc: usize,         // only for top-level block
    pub stack: Vec<Frame>, // loop or block frames
    pub tempo: Tempo,
    pub volume: usize,
    pub pan: isize,

    pub waveform: Waveform,
    pub attack_ms: f32,
    pub decay_ms: f32,
    pub sustain_level: f32,
    pub release_ms: f32,
    pub fm_ratio: f32,
    pub fm_depth: f32,
    pub fm_block: Option<FmBlockConfig>,
    pub swing: f32,
    /// Sounding time minus written time — see `Stmt::Offset`. Applied to
    /// `Event::at` only; `nominal_position` never moves, so offsetting a track
    /// cannot drift it away from the others.
    pub offset: Beats,
    /// Per-track resonant low-pass filter. `cutoff_hz` defaults to 20000 (open);
    /// `resonance` is 0..1.
    pub cutoff_hz: f32,
    pub resonance: f32,
    /// Per-track vibrato LFO: `vibrato_depth_cents` 0 = off, `vibrato_rate_hz` in Hz.
    pub vibrato_depth_cents: f32,
    pub vibrato_rate_hz: f32,
    pub time_signature: TimeSignature,

    pub env_vars: HashMap<Ident, Value>,
    pub saved_envs: Vec<HashMap<Ident, Value>>,
    /// Current nested depth of synchronous (expression-position) user-function
    /// calls; bounded by `MAX_CALL_DEPTH`.
    pub call_depth: usize,
}

#[derive(Clone)]
pub struct TrackRunner {
    pub ast: TrackAst,
    pub ctx: TrackContext,
}

// =========================================================
// STANDARD LIBRARY
// =========================================================

fn format_value(v: &Value) -> String {
    match v {
        Value::Int(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Func { .. } => "<func>".to_string(),
        Value::Builtin(_) => "<builtin>".to_string(),
        Value::Pitch(p) => {
            let acc = match p.accidental {
                1 => "#",
                -1 => "b",
                _ => "",
            };
            format!("{}{}{}", p.letter, acc, p.octave)
        }
        Value::Chord(ps) => {
            let inner: Vec<String> = ps
                .iter()
                .map(|p| {
                    let acc = match p.accidental {
                        1 => "#",
                        -1 => "b",
                        _ => "",
                    };
                    format!("{}{}{}", p.letter, acc, p.octave)
                })
                .collect();
            format!("<{}>", inner.join(", "))
        }
        Value::Array(elems) => {
            let inner: Vec<String> = elems.iter().map(format_value).collect();
            format!("[{}]", inner.join(", "))
        }
    }
}

fn builtin_print(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let parts: Vec<String> = args.iter().map(format_value).collect();
    // stderr, not stdout: wilios-core can be embedded in a host (e.g. an MCP
    // server) that reserves stdout for its own protocol framing.
    tracing::info!("{}", parts.join(" "));
    Ok(Value::Int(0))
}

fn builtin_rand(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(RuntimeError("rand expects 2 arguments".into()));
    }
    let (min, max) = match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => (*a, *b),
        _ => return Err(RuntimeError("rand: both arguments must be integers".into())),
    };
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
    Ok(Value::Int(rand::rng().random_range(lo..=hi)))
}

fn semitone_for_letter(letter: char) -> i64 {
    match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => panic!("Invalid pitch letter: {}", letter),
    }
}

/// Map an absolute semitone (C0 = 0) back to a spelled pitch.
///
/// Naturals map straight through. A black-key class is spelled as the flat of
/// the natural above it (`Db Eb Gb Ab Bb`) when `prefer_flats`, otherwise as
/// the sharp of the natural below it (`C# D# F# G# A#`) — so a flat source
/// pitch transposes to flat output and a natural/sharp source stays sharp.
/// C0 is the lowest representable pitch (`octave` is unsigned); a result below
/// it is a runtime error rather than a silent clamp.
fn semitone_to_pitch(semitone: i64, prefer_flats: bool) -> Result<Pitch, RuntimeError> {
    if semitone < 0 {
        return Err(RuntimeError(
            "transpose: result is below C0, the lowest representable pitch".into(),
        ));
    }
    let octave = (semitone / 12) as usize;
    let class = semitone.rem_euclid(12);
    // Natural semitone positions.
    const NATURALS: [(char, i64); 7] = [
        ('C', 0),
        ('D', 2),
        ('E', 4),
        ('F', 5),
        ('G', 7),
        ('A', 9),
        ('B', 11),
    ];
    if let Some(&(letter, _)) = NATURALS.iter().find(|&&(_, nat)| nat == class) {
        return Ok(Pitch {
            letter,
            accidental: 0,
            octave,
        });
    }
    let flat_of_natural_above = prefer_flats
        .then(|| NATURALS.iter().find(|&&(_, nat)| nat == class + 1))
        .flatten();
    if let Some(&(letter, _)) = flat_of_natural_above {
        return Ok(Pitch {
            letter,
            accidental: -1,
            octave,
        });
    }
    if let Some(&(letter, _)) = NATURALS.iter().find(|&&(_, nat)| nat + 1 == class) {
        return Ok(Pitch {
            letter,
            accidental: 1,
            octave,
        });
    }
    unreachable!("semitone_to_pitch: class {class} is not a natural or its neighbour");
}

fn transpose_one(p: &Pitch, n: i64) -> Result<Pitch, RuntimeError> {
    let abs = semitone_for_letter(p.letter) + p.accidental as i64 + p.octave as i64 * 12;
    semitone_to_pitch(abs + n, p.accidental < 0)
}

fn builtin_transpose(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(RuntimeError("transpose expects 2 arguments".into()));
    }
    let n = match &args[1] {
        Value::Int(n) => *n,
        _ => {
            return Err(RuntimeError(
                "transpose: second argument must be an integer".into(),
            ));
        }
    };
    match &args[0] {
        Value::Pitch(p) => Ok(Value::Pitch(transpose_one(p, n)?)),
        Value::Chord(ps) => Ok(Value::Chord(
            ps.iter()
                .map(|p| transpose_one(p, n))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        _ => Err(RuntimeError(
            "transpose: first argument must be a pitch or chord".into(),
        )),
    }
}

fn builtin_len(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError("len expects 1 argument".into()));
    }
    match &args[0] {
        Value::Array(v) => Ok(Value::Int(v.len() as i64)),
        _ => Err(RuntimeError("len: argument must be an array".into())),
    }
}

/// A built-in function's registration, doubling as its documentation entry
/// for `wilios-mcp`'s `describe_symbol`/`search_stdlib` tools. `name`/`func`
/// drive `Interpreter::new()`'s `initial_env`; `signature`/`doc`/`example`
/// must stay in sync with `doc/stdlib.md` (checked by
/// `crates/wilios-core/tests/stdlib_doc_consistency.rs`) and `example` must
/// actually run (checked by `crates/wilios-core/tests/stdlib_examples.rs`).
pub struct BuiltinSpec {
    pub name: &'static str,
    pub signature: &'static str,
    pub doc: &'static str,
    pub example: &'static str,
    /// Minimum argument count actually enforced by `func` at runtime.
    pub min_args: usize,
    /// Maximum argument count actually enforced by `func` at runtime;
    /// `None` means variadic (no upper bound).
    pub max_args: Option<usize>,
    pub func: fn(Vec<Value>) -> Result<Value, RuntimeError>,
}

pub static BUILTINS: &[BuiltinSpec] = &[
    BuiltinSpec {
        name: "print",
        signature: "print(value, value, ...) -> Int",
        doc: "Print one or more values to standard output, separated by spaces. Returns 0.",
        example: "print(42)",
        // builtin_print never checks args.len(); print() with zero args is
        // valid (prints an empty line), so there is no enforced minimum.
        min_args: 0,
        max_args: None,
        func: builtin_print,
    },
    BuiltinSpec {
        name: "rand",
        signature: "rand(min: Int, max: Int) -> Int",
        doc: "Return a random integer in the range [min, max] inclusive.",
        example: "let n = rand(1, 6)",
        min_args: 2,
        max_args: Some(2),
        func: builtin_rand,
    },
    BuiltinSpec {
        name: "transpose",
        signature: "transpose(value: Pitch | Chord, semitones: Int) -> Pitch | Chord",
        doc: "Transpose a pitch or chord by a given number of semitones. Positive semitones transpose up; negative transpose down. A flat input spells black keys as flats (Eb, not D#); a natural or sharp input spells them as sharps. Transposing below C0 is a runtime error.",
        example: "let fifth = transpose(C4, 7)",
        min_args: 2,
        max_args: Some(2),
        func: builtin_transpose,
    },
    BuiltinSpec {
        name: "len",
        signature: "len(array: Array) -> Int",
        doc: "Return the number of elements in an array.",
        example: "let n = len([C4, E4, G4])",
        min_args: 1,
        max_args: Some(1),
        func: builtin_len,
    },
];

#[derive(Clone)]
pub struct Interpreter {
    pub tracks: Vec<TrackRunner>,
}

impl Interpreter {
    pub fn new(program: Program) -> Result<Self, RuntimeError> {
        // Evaluate global statements once against a single "defaults" context.
        // Let/Assign/Call stmts insert into env_vars without yielding or emitting events;
        // Tempo/Pan/Volume/TimeSignature/etc. mutate the synth-param fields, which are then
        // cloned into every track below as that track's starting defaults (overridable
        // per-track by the same statements inside `track N { ... }`).
        let mut tmp_ctx = {
            let mut initial_env: HashMap<Ident, Value> = HashMap::new();
            for b in BUILTINS {
                initial_env.insert(Ident(b.name.into()), Value::Builtin(b.func));
            }

            TrackContext {
                track_id: usize::MAX,
                stack: vec![],
                time: 0,
                nominal_position: Beats::from_integer(0),
                bar_epoch_beats: Beats::from_integer(0),
                tempo_history: TempoHistory::new(120),
                tempo: Tempo { bpm: 120 },
                volume: 100,
                pan: 0,
                waveform: Waveform::Sine,
                attack_ms: 10.0,
                decay_ms: 0.0,
                sustain_level: 1.0,
                release_ms: 100.0,
                fm_ratio: 1.0,
                fm_depth: 0.0,
                fm_block: None,
                swing: 50.0,
                offset: Beats::from_integer(0),
                cutoff_hz: 20_000.0,
                resonance: 0.0,
                vibrato_depth_cents: 0.0,
                vibrato_rate_hz: 0.0,
                time_signature: TimeSignature {
                    numerator: 4,
                    denominator: 4,
                },
                pc: 0,
                env_vars: initial_env,
                saved_envs: Vec::new(),
                call_depth: 0,
            }
        };
        let mut dummy: Vec<Event> = Vec::new();
        for stmt in &program.global_stmts {
            Self::exec_stmt(stmt, &mut tmp_ctx, &mut dummy, u64::MAX)?;
        }

        let tracks = program
            .tracks
            .into_iter()
            .map(|ast| {
                let mut ctx = tmp_ctx.clone();
                ctx.track_id = ast.id;
                ctx.stack = vec![Frame::Block {
                    statements: ast.statements.clone(),
                    pc: 0,
                }];
                ctx.time = 0;
                ctx.nominal_position = Beats::from_integer(0);
                ctx.bar_epoch_beats = Beats::from_integer(0);
                // Don't inherit tempo history from global-scope execution.
                ctx.tempo_history.reset(ctx.tempo.bpm);
                ctx.pc = 0;
                ctx.saved_envs = Vec::new();
                TrackRunner { ctx, ast }
            })
            .collect();

        Ok(Self { tracks })
    }

    /// True once every track has exhausted its statements and has no
    /// pending loop/block/function-call frames left to resume.
    pub fn all_tracks_finished(&self) -> bool {
        self.tracks
            .iter()
            .all(|t| t.ctx.stack.is_empty() && t.ctx.pc >= t.ast.statements.len())
    }

    /// Schedule events in a **frame window**: [from_ms, until_ms)
    pub fn schedule_until(
        &mut self,
        _from_ms: u64,
        until_ms: u64,
    ) -> Result<Vec<Event>, RuntimeError> {
        let mut out = Vec::new();

        const MAX_STEPS: usize = 1_000_000;
        for track in &mut self.tracks {
            let mut steps = 0usize;
            loop {
                if steps >= MAX_STEPS {
                    break;
                }
                steps += 1;
                let stmt_opt = if let Some(frame) = track.ctx.stack.last().cloned() {
                    match frame {
                        Frame::Block { statements, pc }
                        | Frame::Loop {
                            body: statements,
                            pc,
                            ..
                        }
                        | Frame::FunctionCall {
                            body: statements,
                            pc,
                        } => {
                            if pc < statements.len() {
                                Some(statements[pc].clone())
                            } else {
                                None
                            }
                        }
                    }
                } else if track.ctx.pc < track.ast.statements.len() {
                    Some(track.ast.statements[track.ctx.pc].clone())
                } else {
                    None
                };

                let stmt = match stmt_opt {
                    Some(s) => s,
                    None => {
                        if let Some(frame) = track.ctx.stack.pop() {
                            match frame {
                                Frame::Loop {
                                    condition, body, ..
                                } => {
                                    if let Value::Bool(true) =
                                        Self::eval(&condition, &mut track.ctx)?
                                    {
                                        track.ctx.stack.push(Frame::Loop {
                                            condition,
                                            body,
                                            pc: 0,
                                        });
                                    }
                                }
                                Frame::Block { .. } => {
                                    track.ctx.pc = track.ast.statements.len();
                                }
                                Frame::FunctionCall { .. } => {
                                    track.ctx.env_vars = track
                                        .ctx
                                        .saved_envs
                                        .pop()
                                        .expect("FunctionCall frame popped with no saved env");
                                    track.ctx.call_depth = track.ctx.call_depth.saturating_sub(1);
                                }
                            }
                        } else if track.ctx.pc < track.ast.statements.len() {
                            track.ctx.pc += 1;
                        } else {
                            break; // nothing left in track
                        }
                        continue;
                    }
                };

                let stmt_start = track.ctx.time;
                match &stmt {
                    Stmt::Chord { duration, .. } => {
                        Self::eval_duration_beats(duration, &mut track.ctx)?;
                    }
                    Stmt::Rest { duration } => {
                        Self::eval_duration_beats(duration, &mut track.ctx)?;
                    }
                    _ => {}
                };

                if stmt_start >= until_ms {
                    break;
                }

                if !Self::exec_stmt(&stmt, &mut track.ctx, &mut out, until_ms)? {
                    if let Some(frame) = track.ctx.stack.last_mut() {
                        match frame {
                            Frame::Block { pc, .. }
                            | Frame::Loop { pc, .. }
                            | Frame::FunctionCall { pc, .. } => *pc += 1,
                        }
                    } else {
                        track.ctx.pc += 1;
                    }
                }
            }
        }

        Ok(out)
    }

    /// Drive every track to completion — or until `max_ms` of composition time
    /// is reached, whichever comes first — and return every event emitted,
    /// paired with whether the piece actually ended.
    ///
    /// `finished == false` means scheduling stopped at the bound (or hit the
    /// internal per-call step cap): the source contains an endless loop, or is
    /// simply longer than `max_ms`. Callers given an explicit time bound can
    /// treat that as normal; callers relying on natural termination should
    /// report it as an error. This is the entry point for offline consumers
    /// (`wilios dump`, the `dump_events` MCP tool) that want the whole timeline
    /// in one shot rather than a rolling window like the audio path.
    pub fn schedule_to_end(&mut self, max_ms: u64) -> Result<(Vec<Event>, bool), RuntimeError> {
        let events = self.schedule_until(0, max_ms)?;
        Ok((events, self.all_tracks_finished()))
    }

    /// Execute a single statement.
    /// Returns `Ok(true)` if the outer pc was already advanced (loop/if/call),
    /// `Ok(false)` to let the scheduler advance pc normally.
    fn exec_stmt(
        stmt: &Stmt,
        ctx: &mut TrackContext,
        out: &mut Vec<Event>,
        until_ms: u64,
    ) -> Result<bool, RuntimeError> {
        match stmt {
            Stmt::Chord { duration, pitches } => {
                let dur_beats = Self::eval_duration_beats(duration, ctx)?;
                // Written position and length are authoritative; `swing` and
                // `offset` only decide when it *sounds*. The sounding length is
                // the gap between this onset and the next, so a swung pair still
                // comes out long-short without either value leaving the grid.
                let at_ms = Self::sounding_ms_at(ctx, ctx.nominal_position)?;
                let end_beats = time::checked_add(ctx.nominal_position, dur_beats, "note end")?;
                let dur_ms = Self::sounding_ms_at(ctx, end_beats)?
                    .saturating_sub(at_ms)
                    .max(1);

                if ctx.time < until_ms {
                    let mut resolved: Vec<Pitch> = Vec::new();
                    for pitch_expr in pitches {
                        match Self::eval(pitch_expr, ctx)? {
                            Value::Pitch(p) => resolved.push(p),
                            Value::Chord(ps) => resolved.extend(ps),
                            _ => {
                                return Err(RuntimeError(
                                    "Chord: pitch expression must evaluate to a pitch or chord"
                                        .into(),
                                ));
                            }
                        }
                    }
                    for pitch in resolved {
                        out.push(Event {
                            at: at_ms,
                            at_beats: ctx.nominal_position,
                            track: ctx.track_id,
                            kind: EventKind::Note {
                                pitch,
                                duration: dur_ms,
                                duration_beats: dur_beats,
                                volume: ctx.volume,
                                pan: ctx.pan,
                                waveform: ctx.waveform.clone(),
                                attack_ms: ctx.attack_ms,
                                decay_ms: ctx.decay_ms,
                                sustain_level: ctx.sustain_level,
                                release_ms: ctx.release_ms,
                                fm_ratio: ctx.fm_ratio,
                                fm_depth: ctx.fm_depth,
                                fm_block: ctx.fm_block.clone(),
                                cutoff_hz: ctx.cutoff_hz,
                                resonance: ctx.resonance,
                                vibrato_depth_cents: ctx.vibrato_depth_cents,
                                vibrato_rate_hz: ctx.vibrato_rate_hz,
                                time_signature: ctx.time_signature,
                            },
                        });
                    }
                }
                ctx.nominal_position =
                    time::checked_add(ctx.nominal_position, dur_beats, "advance position")?;
                ctx.time = ctx.tempo_history.ms_at(ctx.nominal_position)?;
                tracing::debug!(
                    track = ctx.track_id,
                    nominal_position = %ctx.nominal_position,
                    time_ms = ctx.time,
                    "track position advanced (note)"
                );
                Ok(false)
            }
            Stmt::Rest { duration } => {
                let dur_beats = Self::eval_duration_beats(duration, ctx)?;
                ctx.nominal_position =
                    time::checked_add(ctx.nominal_position, dur_beats, "advance position")?;
                ctx.time = ctx.tempo_history.ms_at(ctx.nominal_position)?;
                tracing::debug!(
                    track = ctx.track_id,
                    nominal_position = %ctx.nominal_position,
                    time_ms = ctx.time,
                    "track position advanced (rest)"
                );
                Ok(false)
            }
            Stmt::Loop { condition, body } => {
                // Advance the outer frame's pc BEFORE pushing the inner frame,
                // same pattern as Stmt::Call, so the scheduler doesn't advance
                // the new frame's pc and the outer frame resumes correctly after
                // the loop exits.
                if let Some(frame) = ctx.stack.last_mut() {
                    match frame {
                        Frame::Block { pc, .. }
                        | Frame::Loop { pc, .. }
                        | Frame::FunctionCall { pc, .. } => *pc += 1,
                    }
                } else {
                    ctx.pc += 1;
                }
                if let Value::Bool(true) = Self::eval(condition, ctx)? {
                    ctx.stack.push(Frame::Loop {
                        condition: condition.clone(),
                        body: body.clone(),
                        pc: 0,
                    });
                }
                Ok(true) // outer pc already advanced
            }
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                // Same pre-advance pattern as Stmt::Loop / Stmt::Call.
                if let Some(frame) = ctx.stack.last_mut() {
                    match frame {
                        Frame::Block { pc, .. }
                        | Frame::Loop { pc, .. }
                        | Frame::FunctionCall { pc, .. } => *pc += 1,
                    }
                } else {
                    ctx.pc += 1;
                }
                if let Value::Bool(true) = Self::eval(condition, ctx)? {
                    ctx.stack.push(Frame::Block {
                        statements: then_body.clone(),
                        pc: 0,
                    });
                } else if let Some(else_body) = else_body {
                    ctx.stack.push(Frame::Block {
                        statements: else_body.clone(),
                        pc: 0,
                    });
                }
                Ok(true) // outer pc already advanced
            }
            Stmt::Pan(n) => {
                ctx.pan = *n;
                Ok(false)
            }
            Stmt::Volume(v) => {
                ctx.volume = *v;
                Ok(false)
            }
            Stmt::TimeSignature(ts) => {
                if ts.numerator == 0 || ts.denominator == 0 {
                    return Err(RuntimeError(format!(
                        "time_signature: numerator and denominator must both be > 0, got {}/{}",
                        ts.numerator, ts.denominator
                    )));
                }
                ctx.time_signature = *ts;
                ctx.bar_epoch_beats = ctx.nominal_position;
                Ok(false)
            }
            Stmt::Wave(w) => {
                ctx.waveform = w.clone();
                Ok(false)
            }
            Stmt::Attack(expr) => {
                if let Value::Int(n) = Self::eval(expr, ctx)? {
                    ctx.attack_ms = n as f32;
                }
                Ok(false)
            }
            Stmt::Decay(expr) => {
                if let Value::Int(n) = Self::eval(expr, ctx)? {
                    ctx.decay_ms = n as f32;
                }
                Ok(false)
            }
            Stmt::Sustain(expr) => {
                if let Value::Int(n) = Self::eval(expr, ctx)? {
                    ctx.sustain_level = (n as f32 / 100.0).clamp(0.0, 1.0);
                }
                Ok(false)
            }
            Stmt::Release(expr) => {
                if let Value::Int(n) = Self::eval(expr, ctx)? {
                    ctx.release_ms = n as f32;
                }
                Ok(false)
            }
            Stmt::FmRatio(expr) => {
                if let Value::Float(f) = Self::eval(expr, ctx)? {
                    ctx.fm_ratio = f;
                }
                Ok(false)
            }
            Stmt::FmDepth(expr) => {
                if let Value::Float(f) = Self::eval(expr, ctx)? {
                    ctx.fm_depth = f;
                }
                Ok(false)
            }
            Stmt::Swing(expr) => {
                let val = match Self::eval(expr, ctx)? {
                    Value::Int(n) => n as f32,
                    Value::Float(f) => f,
                    _ => return Err(RuntimeError("swing: expected a numeric value".into())),
                };
                if !(50.0..=100.0).contains(&val) {
                    return Err(RuntimeError(format!(
                        "swing: value {:.1} is out of range [50, 100]",
                        val
                    )));
                }
                ctx.swing = val;
                Ok(false)
            }
            Stmt::Offset { duration, negative } => {
                let beats = match Self::eval(&duration.beats, ctx)? {
                    Value::Int(v) => v,
                    _ => return Err(RuntimeError("offset beats must be int".into())),
                };
                let division = match Self::eval(&duration.division, ctx)? {
                    Value::Int(v) => v,
                    _ => return Err(RuntimeError("offset division must be int".into())),
                };
                let context = format!(
                    "track {} offset {}{}/{}{}",
                    ctx.track_id,
                    if *negative { "-" } else { "" },
                    beats,
                    division,
                    if duration.dotted { "." } else { "" }
                );
                let magnitude =
                    time::beats_from_offset(beats, division, duration.dotted, &context)?;
                let value = if *negative { -magnitude } else { magnitude };
                // A whole note either way is far past any playable feel; beyond
                // that it is a mistake, not an intention.
                let limit = Beats::from_integer(1);
                if value > limit || value < -limit {
                    return Err(RuntimeError(format!(
                        "offset: {value} is beyond one whole note either side of the beat"
                    )));
                }
                ctx.offset = value;
                Ok(false)
            }
            Stmt::Cutoff(expr) => {
                let val = Self::eval_number(expr, ctx, "cutoff")?;
                if val < 0.0 {
                    return Err(RuntimeError(format!(
                        "cutoff: value {val:.1} must not be negative"
                    )));
                }
                // Clamping to the audible / Nyquist range happens in the synth
                // so a `dump` still shows the authored figure.
                ctx.cutoff_hz = val;
                Ok(false)
            }
            Stmt::Resonance(expr) => {
                let val = Self::eval_number(expr, ctx, "resonance")?;
                if val < 0.0 {
                    return Err(RuntimeError(format!(
                        "resonance: value {val:.2} must not be negative"
                    )));
                }
                ctx.resonance = val;
                Ok(false)
            }
            Stmt::Vibrato { depth, rate } => {
                let d = Self::eval_number(depth, ctx, "vibrato depth")?;
                let r = Self::eval_number(rate, ctx, "vibrato rate")?;
                if d < 0.0 || r < 0.0 {
                    return Err(RuntimeError(
                        "vibrato: depth and rate must not be negative".into(),
                    ));
                }
                ctx.vibrato_depth_cents = d;
                ctx.vibrato_rate_hz = r;
                Ok(false)
            }
            Stmt::FmBlock { ops, algorithm } => {
                let evaluated_ops: Vec<FmOpConfig> = ops
                    .iter()
                    .map(|op: &FmOperator| -> Result<FmOpConfig, RuntimeError> {
                        let ratio = match Self::eval(&op.ratio, ctx)? {
                            Value::Float(f) => f,
                            Value::Int(n) => n as f32,
                            _ => 1.0,
                        };
                        let level = match Self::eval(&op.level, ctx)? {
                            Value::Float(f) => f,
                            Value::Int(n) => n as f32,
                            _ => 1.0,
                        };
                        let wave = op.wave.clone().unwrap_or_else(|| ctx.waveform.clone());
                        let attack_ms = op
                            .attack_ms
                            .as_ref()
                            .map(|e| -> Result<f32, RuntimeError> {
                                Ok(match Self::eval(e, ctx)? {
                                    Value::Int(n) => n as f32,
                                    Value::Float(f) => f,
                                    _ => ctx.attack_ms,
                                })
                            })
                            .transpose()?
                            .unwrap_or(ctx.attack_ms);
                        let decay_ms = op
                            .decay_ms
                            .as_ref()
                            .map(|e| -> Result<f32, RuntimeError> {
                                Ok(match Self::eval(e, ctx)? {
                                    Value::Int(n) => n as f32,
                                    Value::Float(f) => f,
                                    _ => ctx.decay_ms,
                                })
                            })
                            .transpose()?
                            .unwrap_or(ctx.decay_ms);
                        let sustain_level = op
                            .sustain_level
                            .as_ref()
                            .map(|e| -> Result<f32, RuntimeError> {
                                Ok(match Self::eval(e, ctx)? {
                                    Value::Int(n) => (n as f32 / 100.0).clamp(0.0, 1.0),
                                    Value::Float(f) => f.clamp(0.0, 1.0),
                                    _ => ctx.sustain_level,
                                })
                            })
                            .transpose()?
                            .unwrap_or(ctx.sustain_level);
                        let release_ms = op
                            .release_ms
                            .as_ref()
                            .map(|e| -> Result<f32, RuntimeError> {
                                Ok(match Self::eval(e, ctx)? {
                                    Value::Int(n) => n as f32,
                                    Value::Float(f) => f,
                                    _ => ctx.release_ms,
                                })
                            })
                            .transpose()?
                            .unwrap_or(ctx.release_ms);
                        Ok(FmOpConfig {
                            id: op.id,
                            ratio,
                            level,
                            wave,
                            attack_ms,
                            decay_ms,
                            sustain_level,
                            release_ms,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                ctx.fm_block = Some(FmBlockConfig {
                    ops: evaluated_ops,
                    algorithm: algorithm.clone(),
                });
                Ok(false)
            }
            Stmt::Tempo(t) => {
                tracing::debug!(new_tempo = *t, track_id = ctx.track_id, "tempo changed");
                if *t == 0 {
                    return Err(RuntimeError("Tempo must be greater than 0".into()));
                }
                ctx.tempo.bpm = *t as u32;
                ctx.bar_epoch_beats = ctx.nominal_position;
                ctx.tempo_history
                    .record_tempo_change(ctx.nominal_position, ctx.tempo.bpm)?;
                Ok(false)
            }
            Stmt::Let { name, value } => {
                let value = Self::eval(value, ctx)?;
                ctx.env_vars.insert(name.clone(), value);
                Ok(false)
            }
            Stmt::Assign { name, value } => {
                let value = Self::eval(value, ctx)?;
                ctx.env_vars.insert(name.clone(), value);
                Ok(false)
            }
            Stmt::Call { callee, args } => {
                let func_val = Self::eval(callee, ctx)?;
                match func_val {
                    Value::Func { params, body } => {
                        let arg_vals: Vec<Value> = args
                            .iter()
                            .map(|a| Self::eval(a, ctx))
                            .collect::<Result<Vec<_>, _>>()?;
                        // Bound recursion: each pushed FunctionCall frame also
                        // clones the caller env into `saved_envs`, so runaway
                        // recursion would grow the heap until MAX_STEPS. Cap it
                        // and report cleanly instead. Decremented on frame
                        // teardown (see `schedule_until`).
                        if ctx.call_depth >= MAX_CALL_DEPTH {
                            return Err(RuntimeError(
                                "call stack too deep (possible infinite recursion)".into(),
                            ));
                        }
                        ctx.call_depth += 1;
                        // Snapshot the caller's env so it can be restored on
                        // return, but keep it live as the body's base scope
                        // (clone, not take) — so a func body can see other
                        // top-level funcs, globals, and the builtins. Params
                        // are layered on top and shadow.
                        let caller_env = ctx.env_vars.clone();
                        ctx.saved_envs.push(caller_env);
                        for (param, val) in params.into_iter().zip(arg_vals) {
                            ctx.env_vars.insert(param, val);
                        }
                        // Advance the outer frame's pc past the call statement BEFORE
                        // pushing the FunctionCall frame, so the scheduler doesn't
                        // mistakenly advance the new frame's pc on return.
                        if let Some(frame) = ctx.stack.last_mut() {
                            match frame {
                                Frame::Block { pc, .. }
                                | Frame::Loop { pc, .. }
                                | Frame::FunctionCall { pc, .. } => *pc += 1,
                            }
                        } else {
                            ctx.pc += 1;
                        }
                        ctx.stack.push(Frame::FunctionCall { body, pc: 0 });
                        Ok(true) // outer pc already advanced; don't advance again
                    }
                    Value::Builtin(f) => {
                        let arg_vals: Vec<Value> = args
                            .iter()
                            .map(|a| Self::eval(a, ctx))
                            .collect::<Result<Vec<_>, _>>()?;
                        f(arg_vals)?; // discard return value for statement-level call
                        Ok(false)
                    }
                    _ => {
                        tracing::warn!("Call: callee is not a function");
                        Ok(false)
                    }
                }
            }
            Stmt::Return { .. } => {
                // Pop nested Loop/Block frames until we reach FunctionCall
                loop {
                    match ctx.stack.last() {
                        Some(Frame::FunctionCall { .. }) => break,
                        Some(_) => {
                            ctx.stack.pop();
                        }
                        None => break,
                    }
                }
                // Set FunctionCall's pc to body.len() to trigger normal frame pop
                if let Some(Frame::FunctionCall { body, pc }) = ctx.stack.last_mut() {
                    *pc = body.len();
                }
                Ok(true) // don't advance pc
            }
            Stmt::IndexAssign { name, index, value } => {
                let idx = match Self::eval(index, ctx)? {
                    Value::Int(i) => i as usize,
                    _ => return Err(RuntimeError("Array index must be an integer".into())),
                };
                let val = Self::eval(value, ctx)?;
                match ctx.env_vars.get_mut(name) {
                    Some(Value::Array(arr)) => {
                        if idx < arr.len() {
                            arr[idx] = val;
                            Ok(false)
                        } else {
                            Err(RuntimeError(format!(
                                "Array index {} out of bounds (len {})",
                                idx,
                                arr.len()
                            )))
                        }
                    }
                    Some(_) => Err(RuntimeError(format!("{:?} is not an array", name))),
                    None => Err(RuntimeError(format!("Undefined variable: {:?}", name))),
                }
            }
            _ => Ok(false),
        }
    }

    /// How far a note at `pos` is pushed off its written time by `swing`.
    ///
    /// Only a position that lands *exactly* on an odd 8th-note slot of the bar
    /// is displaced, by `(swing/100 - 1/2)` of a quarter; everything else —
    /// downbeats, triplets, dotted values, 16ths — is left alone. Swing never
    /// touches `nominal_position`, so no duration is re-quantized and no bar
    /// can drift.
    fn swing_displacement(ctx: &TrackContext, pos: Beats) -> Result<Beats, RuntimeError> {
        let zero = Beats::from_integer(0);
        if (ctx.swing - 50.0).abs() < f32::EPSILON {
            return Ok(zero);
        }
        let eighth = Beats::new(1, 8);
        let bar_len = Beats::new(
            ctx.time_signature.numerator as i64,
            ctx.time_signature.denominator as i64,
        );
        let since_epoch = time::checked_sub(pos, ctx.bar_epoch_beats, "swing bar position")?;
        let in_bar = time::rem_euclid(since_epoch, bar_len, "swing bar position")?;
        let slots = in_bar / eighth;
        if !slots.is_integer() || slots.to_integer() % 2 == 0 {
            return Ok(zero);
        }
        // 3-decimal precision on the ratio, as the duration-based version used.
        let ratio = Beats::new((ctx.swing as f64 * 1000.0).round() as i64, 100_000);
        let long = time::checked_mul(Beats::new(1, 4), ratio, "swing long slot")?;
        Ok(time::checked_sub(long, eighth, "swing displacement")?)
    }

    /// Where a note at `pos` should *sound*: its written ms, displaced by
    /// `swing` and by the track's `offset`. `nominal_position` is untouched, so
    /// this changes audible placement without moving the piece's timeline.
    fn sounding_ms_at(ctx: &TrackContext, pos: Beats) -> Result<u64, RuntimeError> {
        let zero = Beats::from_integer(0);
        let swing = Self::swing_displacement(ctx, pos)?;
        if swing == zero && ctx.offset == zero {
            return Ok(ctx.tempo_history.ms_at(pos)?);
        }
        let shifted = time::checked_add(pos, swing, "swing position")?;
        let shifted = time::checked_add(shifted, ctx.offset, "offset position")?;
        if shifted <= zero {
            return Ok(0); // an early offset at the top of the piece
        }
        Ok(ctx.tempo_history.ms_at(shifted)?)
    }

    fn eval_duration_beats(
        duration: &Duration,
        ctx: &mut TrackContext,
    ) -> Result<Beats, RuntimeError> {
        let beats = match Self::eval(&duration.beats, ctx)? {
            Value::Int(v) => v,
            _ => return Err(RuntimeError("Duration beats must be int".into())),
        };
        let division = match Self::eval(&duration.division, ctx)? {
            Value::Int(v) => v,
            _ => return Err(RuntimeError("Duration division must be int".into())),
        };
        let context = format!(
            "track {} duration {}/{}{} (line {})",
            ctx.track_id,
            beats,
            division,
            if duration.dotted { "." } else { "" },
            duration.line
        );
        time::beats_from_duration(beats, division, duration.dotted, &context)
            .map_err(RuntimeError::from)
    }

    /// Evaluate `expr` and require a numeric result (`Int` or `Float`), naming
    /// `what` in the error. Used by the tone-shaping statements.
    fn eval_number(expr: &Expr, ctx: &mut TrackContext, what: &str) -> Result<f32, RuntimeError> {
        match Self::eval(expr, ctx)? {
            Value::Int(n) => Ok(n as f32),
            Value::Float(f) => Ok(f),
            _ => Err(RuntimeError(format!("{what}: expected a numeric value"))),
        }
    }

    fn eval(expr: &Expr, ctx: &mut TrackContext) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Int(i) => Ok(Value::Int(*i as i64)),
            Expr::Float(f) => Ok(Value::Float(*f)),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Pitch(p) => Ok(Value::Pitch(p.clone())),
            Expr::Chord(exprs) => {
                let mut pitches: Vec<Pitch> = Vec::new();
                for e in exprs {
                    match Self::eval(e, ctx)? {
                        Value::Pitch(p) => pitches.push(p),
                        Value::Chord(ps) => pitches.extend(ps),
                        _ => {
                            return Err(RuntimeError(
                                "Chord expression: each element must evaluate to a pitch or chord"
                                    .into(),
                            ));
                        }
                    }
                }
                Ok(Value::Chord(pitches))
            }

            Expr::Unary { op, expr } => match op {
                UnaryOp::Neg => {
                    if let Value::Int(v) = Self::eval(expr, ctx)? {
                        Ok(Value::Int(-v))
                    } else {
                        Err(RuntimeError("Unary - on non-int".into()))
                    }
                }
                UnaryOp::Not => {
                    if let Value::Bool(v) = Self::eval(expr, ctx)? {
                        Ok(Value::Bool(!v))
                    } else {
                        Err(RuntimeError("Unary ! on non-boolean".into()))
                    }
                }
            },

            Expr::Var { name, .. } => ctx
                .env_vars
                .get(name)
                .cloned()
                .ok_or_else(|| RuntimeError(format!("Undefined variable: {:?}", name))),

            Expr::Func { params, body } => Ok(Value::Func {
                params: params.clone(),
                body: body.clone(),
            }),

            Expr::Call { callee, args } => {
                let func_val = Self::eval(callee, ctx)?;
                match func_val {
                    Value::Func { params, body } => {
                        let arg_vals: Vec<Value> = args
                            .iter()
                            .map(|a| Self::eval(a, ctx))
                            .collect::<Result<Vec<_>, _>>()?;
                        if ctx.call_depth >= MAX_CALL_DEPTH {
                            return Err(RuntimeError(
                                "call stack too deep (possible infinite recursion)".into(),
                            ));
                        }
                        // Clone (not take) so the body can still see other
                        // top-level funcs, globals, and the builtins; params
                        // are layered on top and shadow. Restored on return.
                        let saved_env = ctx.env_vars.clone();
                        for (param, val) in params.iter().zip(arg_vals) {
                            ctx.env_vars.insert(param.clone(), val);
                        }
                        ctx.call_depth += 1;
                        let result = Self::eval_body_sync(&body, ctx);
                        ctx.call_depth -= 1;
                        ctx.env_vars = saved_env;
                        result
                    }
                    Value::Builtin(f) => {
                        let arg_vals: Vec<Value> = args
                            .iter()
                            .map(|a| Self::eval(a, ctx))
                            .collect::<Result<Vec<_>, _>>()?;
                        f(arg_vals)
                    }
                    _ => Err(RuntimeError("Call: callee is not a function".into())),
                }
            }

            Expr::Array(exprs) => {
                let elems = exprs
                    .iter()
                    .map(|e| Self::eval(e, ctx))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Value::Array(elems))
            }

            Expr::Index { array, index } => {
                let arr = Self::eval(array, ctx)?;
                let idx = match Self::eval(index, ctx)? {
                    Value::Int(i) => i as usize,
                    _ => return Err(RuntimeError("Array index must be an integer".into())),
                };
                match arr {
                    Value::Array(elems) => elems
                        .into_iter()
                        .nth(idx)
                        .ok_or_else(|| RuntimeError(format!("Array index {} out of bounds", idx))),
                    _ => Err(RuntimeError(
                        "Index operator applied to non-array value".into(),
                    )),
                }
            }

            Expr::Binary { left, op, right } => {
                let l = Self::eval(left, ctx)?;
                let r = Self::eval(right, ctx)?;

                match op {
                    BinaryOp::Add => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                        _ => Err(RuntimeError("+ expects ints".into())),
                    },

                    BinaryOp::Sub => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
                        _ => Err(RuntimeError("- expects ints".into())),
                    },

                    BinaryOp::Mul => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
                        _ => Err(RuntimeError("* expects ints".into())),
                    },

                    BinaryOp::Div => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => {
                            if b == 0 {
                                return Err(RuntimeError("Division by zero".into()));
                            }
                            Ok(Value::Int(a / b))
                        }
                        _ => Err(RuntimeError("/ expects ints".into())),
                    },

                    BinaryOp::Mod => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => {
                            if b == 0 {
                                return Err(RuntimeError("Modulo by zero".into()));
                            }
                            Ok(Value::Int(a % b))
                        }
                        _ => Err(RuntimeError("% expects ints".into())),
                    },

                    BinaryOp::Eq => Ok(Value::Bool(l == r)),
                    BinaryOp::NotEq => Ok(Value::Bool(l != r)),

                    BinaryOp::Lt => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
                        _ => Err(RuntimeError("< expects ints".into())),
                    },

                    BinaryOp::LtEq => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
                        _ => Err(RuntimeError("<= expects ints".into())),
                    },

                    BinaryOp::Gt => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
                        _ => Err(RuntimeError("> expects ints".into())),
                    },

                    BinaryOp::GtEq => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
                        _ => Err(RuntimeError(">= expects ints".into())),
                    },

                    BinaryOp::And => match (l, r) {
                        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
                        _ => Err(RuntimeError("&& expects bools".into())),
                    },

                    BinaryOp::Or => match (l, r) {
                        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),
                        _ => Err(RuntimeError("|| expects bools".into())),
                    },
                }
            }
        }
    }

    fn eval_body_sync(body: &[Stmt], ctx: &mut TrackContext) -> Result<Value, RuntimeError> {
        for stmt in body {
            match stmt {
                Stmt::Return { value } => return Self::eval(value, ctx),
                Stmt::Let { name, value } => {
                    let v = Self::eval(value, ctx)?;
                    ctx.env_vars.insert(name.clone(), v);
                }
                Stmt::Assign { name, value } => {
                    let v = Self::eval(value, ctx)?;
                    ctx.env_vars.insert(name.clone(), v);
                }
                _ => {}
            }
        }
        Ok(Value::Int(0))
    }
}

#[cfg(test)]
#[path = "interpreter_tests.rs"]
mod interpreter_tests;
