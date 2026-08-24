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
    /// Authoritative ms position — derived fresh from `nominal_position` via
    /// `tempo_history.ms_at(..)` after every note/rest, not accumulated.
    pub time: u64,
    /// Exact nominal position (whole-note units) since track start. Two
    /// tracks with the same tempo history that reach the same
    /// `nominal_position` are guaranteed the same `time`, regardless of how
    /// differently subdivided the path there was.
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
    pub time_signature: TimeSignature,

    pub env_vars: HashMap<Ident, Value>,
    pub saved_envs: Vec<HashMap<Ident, Value>>,
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
    println!("{}", parts.join(" "));
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

fn semitone_to_pitch(semitone: i64) -> Pitch {
    let octave = (semitone / 12).max(0) as usize;
    let class = semitone.rem_euclid(12);
    // Natural semitone positions; class+1 = sharp of that note
    const NATURALS: [(char, i64); 7] = [
        ('C', 0),
        ('D', 2),
        ('E', 4),
        ('F', 5),
        ('G', 7),
        ('A', 9),
        ('B', 11),
    ];
    for &(letter, nat) in &NATURALS {
        if class == nat {
            return Pitch {
                letter,
                accidental: 0,
                octave,
            };
        }
        if class == nat + 1 {
            return Pitch {
                letter,
                accidental: 1,
                octave,
            };
        }
    }
    // B# edge case (semitone 12 within octave, treated as C of next octave by rem_euclid — unreachable)
    panic!("semitone_to_pitch: unreachable class {}", class);
}

fn transpose_one(p: &Pitch, n: i64) -> Pitch {
    let abs = semitone_for_letter(p.letter) + p.accidental as i64 + p.octave as i64 * 12;
    semitone_to_pitch(abs + n)
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
        Value::Pitch(p) => Ok(Value::Pitch(transpose_one(p, n))),
        Value::Chord(ps) => Ok(Value::Chord(
            ps.iter().map(|p| transpose_one(p, n)).collect(),
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
            initial_env.insert(Ident("print".into()), Value::Builtin(builtin_print));
            initial_env.insert(Ident("rand".into()), Value::Builtin(builtin_rand));
            initial_env.insert(Ident("transpose".into()), Value::Builtin(builtin_transpose));
            initial_env.insert(Ident("len".into()), Value::Builtin(builtin_len));

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
                time_signature: TimeSignature {
                    numerator: 4,
                    denominator: 4,
                },
                pc: 0,
                env_vars: initial_env,
                saved_envs: Vec::new(),
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
                // Fresh single breakpoint — must not inherit tempo-change
                // history accumulated while executing global-scope statements
                // against `tmp_ctx`.
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
                // 1️⃣ Determine next statement
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
                        // End of frame / no statement
                        if let Some(frame) = track.ctx.stack.pop() {
                            match frame {
                                Frame::Loop {
                                    condition, body, ..
                                } => {
                                    // Re-evaluate loop condition
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

                // 2️⃣ Compute duration if note/rest
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
                    // Reached end of this frame
                    break;
                }

                // 3️⃣ Execute statement
                if !Self::exec_stmt(&stmt, &mut track.ctx, &mut out, until_ms)? {
                    // Advance pc
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
                let (swung_beats, dur_ms) = Self::apply_swing_beats(dur_beats, ctx)?;

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
                            at: ctx.time,
                            at_beats: ctx.nominal_position,
                            track: ctx.track_id,
                            kind: EventKind::Note {
                                pitch,
                                duration: dur_ms,
                                duration_beats: swung_beats,
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
                                time_signature: ctx.time_signature,
                            },
                        });
                    }
                }
                ctx.nominal_position =
                    time::checked_add(ctx.nominal_position, swung_beats, "advance position")?;
                ctx.time = ctx.tempo_history.ms_at(ctx.nominal_position)?;
                Ok(false)
            }
            Stmt::Rest { duration } => {
                let dur_beats = Self::eval_duration_beats(duration, ctx)?;
                let (swung_beats, _dur_ms) = Self::apply_swing_beats(dur_beats, ctx)?;
                ctx.nominal_position =
                    time::checked_add(ctx.nominal_position, swung_beats, "advance position")?;
                ctx.time = ctx.tempo_history.ms_at(ctx.nominal_position)?;
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
                println!("new tempo {}, track id: {}", *t, ctx.track_id);
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
                        let caller_env = std::mem::take(&mut ctx.env_vars);
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
                        eprintln!("Call: callee is not a function");
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

    /// Same rules as `doc/synthesis.md`'s Swing section, re-derived from exact
    /// beats instead of post-hoc ms: on-beat/even 8th-slot lengthened,
    /// off-beat/odd shortened, quarter+ unaffected, sub-eighth passthrough.
    /// Slot parity comes from the exact bar-relative nominal position, so it
    /// can never be misclassified by ms-side rounding drift near a boundary.
    /// Returns the swung duration in both beats (for `nominal_position`
    /// advancement / `Event.duration_beats`) and ms (for `Event.duration`).
    fn apply_swing_beats(
        duration_beats: Beats,
        ctx: &TrackContext,
    ) -> Result<(Beats, u64), RuntimeError> {
        let bpm = ctx.tempo.bpm;
        let eighth_beats = Beats::new(1, 8);
        if (ctx.swing - 50.0).abs() < f32::EPSILON || bpm == 0 || duration_beats < eighth_beats {
            let ms = time::beats_delta_to_ms(duration_beats, bpm)?;
            return Ok((duration_beats, ms));
        }

        let quarter_beats = Beats::new(1, 4);
        // 3-decimal-place precision on the swing ratio itself (not just the
        // percent) — reproduces every existing swing test exactly.
        let swing_ratio = Beats::new((ctx.swing as f64 * 1000.0).round() as i64, 100_000);
        let long_beats = time::checked_mul(quarter_beats, swing_ratio, "swing long slot")?;
        // Deriving `short` as the exact complement of `long` (not independently
        // from `1 - ratio`) is what guarantees long+short == quarter_beats
        // exactly, making "quarter+ unaffected" a real invariant.
        let short_beats = time::checked_sub(quarter_beats, long_beats, "swing short slot")?;

        let bar_len_beats = Beats::new(
            ctx.time_signature.numerator as i64,
            ctx.time_signature.denominator as i64,
        );
        let position_since_epoch = time::checked_sub(
            ctx.nominal_position,
            ctx.bar_epoch_beats,
            "swing bar position",
        )?;
        let position_in_bar =
            time::rem_euclid(position_since_epoch, bar_len_beats, "swing bar position")?;
        let start_slot = time::round_half_up(position_in_bar / eighth_beats);
        let num_slots = time::round_half_up(duration_beats / eighth_beats);

        let mut swung_beats = Beats::from_integer(0);
        for i in 0..num_slots {
            let slot_beats = if (start_slot + i).is_multiple_of(2) {
                long_beats
            } else {
                short_beats
            };
            swung_beats = time::checked_add(swung_beats, slot_beats, "swing sum")?;
        }
        let ms = time::beats_delta_to_ms(swung_beats, bpm)?;
        Ok((swung_beats, ms))
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

            Expr::Var(v) => ctx
                .env_vars
                .get(v)
                .cloned()
                .ok_or_else(|| RuntimeError(format!("Undefined variable: {:?}", v))),

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
                        let saved_env = std::mem::take(&mut ctx.env_vars);
                        for (param, val) in params.iter().zip(arg_vals) {
                            ctx.env_vars.insert(param.clone(), val);
                        }
                        let result = Self::eval_body_sync(&body, ctx)?;
                        ctx.env_vars = saved_env;
                        Ok(result)
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
