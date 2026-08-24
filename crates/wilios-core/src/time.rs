//! Exact-rational musical time.
//!
//! `Beats` represents a nominal position or duration in whole-note units
//! (`1/4` in the DSL is exactly `Beats::new(1, 4)`). Durations and track
//! positions are accumulated exactly here; conversion to milliseconds
//! happens once, at the point a value is actually needed, via a per-track
//! [`TempoHistory`] — never by summing already-rounded deltas.

use num_rational::Ratio;

pub type Beats = Ratio<i64>;

/// Early diagnostic threshold. Not the safety net (checked arithmetic is) —
/// just a point past which a denominator is certainly a computed-duration
/// mistake (e.g. `1/n` inside a loop where `n` happened to be a large or
/// prime runtime value) rather than a musically meaningful tuplet.
pub const MAX_DENOM: i64 = 1 << 24;

#[derive(Debug, Clone, PartialEq)]
pub enum TimeError {
    DenominatorTooLarge {
        denom: i64,
        context: String,
    },
    Overflow {
        context: String,
    },
    /// Display text must stay exactly "Duration division cannot be zero" —
    /// `interpreter_tests.rs::interpreter_rest_division_by_zero` string-matches it.
    ZeroDivision,
    NonPositiveBeats {
        context: String,
    },
}

impl std::fmt::Display for TimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeError::DenominatorTooLarge { denom, context } => write!(
                f,
                "{context}: duration denominator {denom} is implausibly large (max {MAX_DENOM}) — likely a computed division"
            ),
            TimeError::Overflow { context } => {
                write!(f, "{context}: duration arithmetic overflowed")
            }
            TimeError::ZeroDivision => write!(f, "Duration division cannot be zero"),
            TimeError::NonPositiveBeats { context } => {
                write!(f, "{context}: duration beats and division must be positive")
            }
        }
    }
}

impl std::error::Error for TimeError {}

fn gcd_i128(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Reduce an arbitrary-precision numer/denom pair to lowest terms and pack
/// it into a `Beats` (`i64`-backed), or report why it doesn't fit.
fn reduce_and_pack(numer: i128, denom: i128, context: &str) -> Result<Beats, TimeError> {
    debug_assert!(denom != 0);
    let (mut numer, mut denom) = (numer, denom);
    if denom < 0 {
        numer = -numer;
        denom = -denom;
    }
    let g = gcd_i128(numer, denom).max(1);
    let numer = numer / g;
    let denom = denom / g;
    let overflow = || TimeError::Overflow {
        context: context.to_string(),
    };
    if numer < i64::MIN as i128 || numer > i64::MAX as i128 || denom > i64::MAX as i128 {
        return Err(overflow());
    }
    if denom > MAX_DENOM as i128 {
        return Err(TimeError::DenominatorTooLarge {
            denom: denom as i64,
            context: context.to_string(),
        });
    }
    Ok(Ratio::new(numer as i64, denom as i64))
}

pub fn checked_add(a: Beats, b: Beats, context: &str) -> Result<Beats, TimeError> {
    let (n1, d1) = (*a.numer() as i128, *a.denom() as i128);
    let (n2, d2) = (*b.numer() as i128, *b.denom() as i128);
    let overflow = || TimeError::Overflow {
        context: context.to_string(),
    };
    let t1 = n1.checked_mul(d2).ok_or_else(overflow)?;
    let t2 = n2.checked_mul(d1).ok_or_else(overflow)?;
    let numer = t1.checked_add(t2).ok_or_else(overflow)?;
    let denom = d1.checked_mul(d2).ok_or_else(overflow)?;
    reduce_and_pack(numer, denom, context)
}

pub fn checked_sub(a: Beats, b: Beats, context: &str) -> Result<Beats, TimeError> {
    let (n2, d2) = (*b.numer() as i128, *b.denom() as i128);
    checked_add_raw(a, -n2, d2, context)
}

fn checked_add_raw(a: Beats, n2: i128, d2: i128, context: &str) -> Result<Beats, TimeError> {
    let (n1, d1) = (*a.numer() as i128, *a.denom() as i128);
    let overflow = || TimeError::Overflow {
        context: context.to_string(),
    };
    let t1 = n1.checked_mul(d2).ok_or_else(overflow)?;
    let t2 = n2.checked_mul(d1).ok_or_else(overflow)?;
    let numer = t1.checked_add(t2).ok_or_else(overflow)?;
    let denom = d1.checked_mul(d2).ok_or_else(overflow)?;
    reduce_and_pack(numer, denom, context)
}

pub fn checked_mul(a: Beats, b: Beats, context: &str) -> Result<Beats, TimeError> {
    let (n1, d1) = (*a.numer() as i128, *a.denom() as i128);
    let (n2, d2) = (*b.numer() as i128, *b.denom() as i128);
    let overflow = || TimeError::Overflow {
        context: context.to_string(),
    };
    let numer = n1.checked_mul(n2).ok_or_else(overflow)?;
    let denom = d1.checked_mul(d2).ok_or_else(overflow)?;
    reduce_and_pack(numer, denom, context)
}

/// Exact rational modulo (`Ratio<i64>` doesn't implement `Rem`): `a - m * floor(a / m)`.
pub fn rem_euclid(a: Beats, m: Beats, context: &str) -> Result<Beats, TimeError> {
    debug_assert!(m > Ratio::new(0, 1));
    let q = (a / m).floor();
    checked_sub(a, checked_mul(q, m, context)?, context)
}

/// A DSL duration (`beats`/`division`, as written — e.g. `1/3`) as an exact
/// `Beats` value in whole-note units, `* 3/2` if dotted. `1/4` => `1/4`
/// (a quarter note); three `1/3`s sum to exactly `1` (a whole note).
pub fn beats_from_duration(
    beats: i64,
    division: i64,
    dotted: bool,
    context: &str,
) -> Result<Beats, TimeError> {
    if division == 0 {
        return Err(TimeError::ZeroDivision);
    }
    if beats <= 0 || division < 0 {
        return Err(TimeError::NonPositiveBeats {
            context: context.to_string(),
        });
    }
    let base = reduce_and_pack(beats as i128, division as i128, context)?;
    if dotted {
        checked_mul(base, Ratio::new(3, 2), context)
    } else {
        Ok(base)
    }
}

/// A `Beats` delta (whole-note units) at a single constant tempo, rounded
/// once (round-half-up) to whole milliseconds. `240_000 = 4 * 60_000`: a
/// whole note is 4 quarter notes, each `60_000 / bpm` ms long.
pub fn beats_delta_to_ms(delta: Beats, bpm: u32) -> Result<u64, TimeError> {
    if bpm == 0 {
        return Err(TimeError::Overflow {
            context: "tempo (bpm) cannot be zero".to_string(),
        });
    }
    let ms_per_whole_note = Ratio::new(240_000i64, bpm as i64);
    let ms = checked_mul(delta, ms_per_whole_note, "beats_delta_to_ms")?;
    Ok(round_half_up(ms))
}

/// Round a nonnegative exact `Beats`-typed ms value to the nearest whole ms
/// (ties round up), via an `i128` intermediate to avoid overflow.
pub fn round_half_up(r: Beats) -> u64 {
    let numer = *r.numer() as i128;
    let denom = *r.denom() as i128;
    debug_assert!(denom > 0);
    debug_assert!(numer >= 0);
    ((2 * numer + denom) / (2 * denom)) as u64
}

#[derive(Clone, Debug)]
struct Breakpoint {
    position: Beats,
    bpm: u32,
    /// Exact (unrounded) ms elapsed to reach `position`, at whatever tempo
    /// history preceded it. Precomputed once when the breakpoint is added,
    /// so later lookups never re-walk earlier segments.
    cumulative_ms: Beats,
}

/// A track's tempo timeline: an ordered list of (position, bpm) breakpoints.
/// `Stmt::Tempo` only affects notes going forward from where it's executed
/// (no retroactive recompute of already-emitted events) — this mirrors that
/// by construction: earlier breakpoints' `cumulative_ms` never changes once
/// a later one is appended.
#[derive(Clone, Debug)]
pub struct TempoHistory {
    breakpoints: Vec<Breakpoint>,
}

impl TempoHistory {
    pub fn new(initial_bpm: u32) -> Self {
        TempoHistory {
            breakpoints: vec![Breakpoint {
                position: Ratio::from_integer(0),
                bpm: initial_bpm,
                cumulative_ms: Ratio::from_integer(0),
            }],
        }
    }

    /// Reset to a fresh single breakpoint at position 0 — used when a
    /// `TrackContext` (re)starts, so a track never inherits tempo-change
    /// breakpoints accumulated while executing global-scope statements.
    pub fn reset(&mut self, bpm: u32) {
        *self = TempoHistory::new(bpm);
    }

    pub fn record_tempo_change(&mut self, at: Beats, bpm: u32) -> Result<(), TimeError> {
        let cumulative_ms = self.exact_ms_at(at)?;
        if let Some(last) = self.breakpoints.last_mut()
            && last.position == at
        {
            last.bpm = bpm;
            last.cumulative_ms = cumulative_ms;
            return Ok(());
        }
        self.breakpoints.push(Breakpoint {
            position: at,
            bpm,
            cumulative_ms,
        });
        Ok(())
    }

    /// Exact (unrounded) cumulative ms elapsed to reach `position`, expressed
    /// as a `Beats`-typed ms value. Binary-searches the active segment
    /// (`O(log n)`) instead of re-walking from position 0 every call, so a
    /// pathological `Stmt::Tempo` inside a `Stmt::Loop` (up to `MAX_STEPS`
    /// breakpoints) stays `O(n log n)` overall rather than `O(n^2)`.
    fn exact_ms_at(&self, position: Beats) -> Result<Beats, TimeError> {
        let idx = self
            .breakpoints
            .partition_point(|bp| bp.position <= position);
        let active = &self.breakpoints[idx.saturating_sub(1)];
        let delta = checked_sub(position, active.position, "tempo segment elapsed")?;
        let ms_per_whole_note = Ratio::new(240_000i64, active.bpm as i64);
        let segment_ms = checked_mul(delta, ms_per_whole_note, "tempo segment ms")?;
        checked_add(active.cumulative_ms, segment_ms, "cumulative ms")
    }

    /// The authoritative ms position for `position` — a pure function of the
    /// exact nominal position and this track's tempo history, not an
    /// accumulator. Two tracks with the same tempo history that reach the
    /// same nominal `position` (however differently subdivided the path
    /// there was) get bit-identical results.
    pub fn ms_at(&self, position: Beats) -> Result<u64, TimeError> {
        Ok(round_half_up(self.exact_ms_at(position)?))
    }
}

#[cfg(test)]
#[path = "time_tests.rs"]
mod time_tests;
