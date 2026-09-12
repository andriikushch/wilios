//! Machine-readable index of the wilios "standard library" — the DSL's 4
//! built-in functions (see [`crate::interpreter::BUILTINS`]) plus the 14 FM
//! presets defined in `lib/lib.wilios`. Backs `wilios-mcp`'s `describe_symbol`
//! and `search_stdlib` tools.
//!
//! `doc/stdlib.md` is the human-facing source of truth for prose; the
//! `doc`/`example` strings here are short derivatives of it and should be
//! kept in sync by hand. `crates/wilios-core/tests/stdlib_doc_consistency.rs`
//! checks that names/signatures/categories can't silently drift apart from
//! `doc/stdlib.md`, and `crates/wilios-core/tests/stdlib_examples.rs` checks
//! that every `example` below actually lexes/parses/runs.

use crate::interpreter::BUILTINS;

/// One of the 14 FM synthesis instrument presets in `lib/lib.wilios`. Unlike
/// [`crate::interpreter::BuiltinSpec`], these can't be generated from a Rust
/// registration site — the presets are wilios source, not Rust functions —
/// so this table is hand-maintained.
pub struct PresetSpec {
    pub name: &'static str,
    pub category: &'static str, // "tonal" | "drum"
    pub doc: &'static str,
    pub example: &'static str,
}

pub static PRESETS: &[PresetSpec] = &[
    PresetSpec {
        name: "epiano",
        category: "tonal",
        doc: "Electric piano. Two independent 2-op pairs produce a bell-like \"ting\" attack; fully percussive, no sustain.",
        example: "track 1\nepiano()\n<C4, E4, G4> 1/4",
    },
    PresetSpec {
        name: "brass",
        category: "tonal",
        doc: "Brass stab. A same-ratio modulator at high depth creates a bright \"blat\" attack that settles into a warm, sustained tone.",
        example: "track 1\nbrass()\n<C3> 1/4",
    },
    PresetSpec {
        name: "trumpet",
        category: "tonal",
        doc: "Sustained FM brass with vibrato. A same-ratio modulator held through the sustain gives the buzzy body; a low-pass tames the fizz and a gentle vibrato adds air on held notes. Slower, tongued onset than brass; play <A3>-<A5>.",
        example: "track 1\ntrumpet()\n<A3> 1/2",
    },
    PresetSpec {
        name: "bass",
        category: "tonal",
        doc: "Deep FM bass. A sub-octave modulator at high depth produces a thick, punchy transient over a clean fundamental.",
        example: "track 1\nbass()\n<A2> 1/4",
    },
    PresetSpec {
        name: "upright",
        category: "tonal",
        doc: "Fingered acoustic bass. A same-ratio modulator at modest depth gives a soft, woody thump over a warm fundamental — rounder and less punchy than bass; play low like <E1>.",
        example: "track 1\nupright()\n<E1> 1/4",
    },
    PresetSpec {
        name: "marimba",
        category: "tonal",
        doc: "Bell/mallet. Two inharmonic modulators feed a single carrier for a metallic, resonant, fast-decaying tone.",
        example: "track 1\nmarimba()\n<C5> 1/8",
    },
    PresetSpec {
        name: "strings",
        category: "tonal",
        doc: "Lush slow pad. Two 2-op pairs with slow attacks produce a smooth swell across a fundamental and octave layer.",
        example: "track 1\nstrings()\n<A3, C4> 1/2",
    },
    PresetSpec {
        name: "comp_piano",
        category: "tonal",
        doc: "Sustaining comping keyboard. Two carrier layers hold a real sustain so voiced chords ring under a melody, unlike epiano's purely percussive decay; mild inharmonic modulators add tine colour.",
        example: "track 1\ncomp_piano()\n<C3, E3, G3> 1/2",
    },
    PresetSpec {
        name: "kick",
        category: "drum",
        doc: "Bass drum. A sub-octave modulator at very high depth creates the characteristic pitch-drop thud; play at low notes like <B1>.",
        example: "track 1\nkick()\n<B1> 1/4",
    },
    PresetSpec {
        name: "snare",
        category: "drum",
        doc: "Snare drum. Two high-ratio inharmonic modulators on a square-wave carrier produce a bright, noise-dense crack; play around <A3>.",
        example: "track 1\nsnare()\n<A3> 1/4",
    },
    PresetSpec {
        name: "hihat_c",
        category: "drum",
        doc: "Closed hi-hat. Two very high-ratio inharmonic modulators produce a short, metallic click; play in the high register like <F5>.",
        example: "track 1\nhihat_c()\n<F5> 1/8",
    },
    PresetSpec {
        name: "hihat_o",
        category: "drum",
        doc: "Open hi-hat. Same topology as hihat_c but with longer decay/release for a sustained \"tsss\" ring; play in the high register like <F5>.",
        example: "track 1\nhihat_o()\n<F5> 1/8",
    },
    PresetSpec {
        name: "ride",
        category: "drum",
        doc: "Ride cymbal. Lower, more musical inharmonic partials than the hi-hats plus long decay/release, so the strike blooms into a sustained shimmer wash rather than a short click; play high like <F5>.",
        example: "track 1\nride()\n<F5> 1/4",
    },
    PresetSpec {
        name: "brushes",
        category: "drum",
        doc: "Snare brush swish. A soft attack and mid-band inharmonic modulators give a breathy, sustained swish rather than snare's hard crack; play around <A3>.",
        example: "track 1\nbrushes()\n<A3> 1/4",
    },
];

/// A stdlib symbol (builtin or preset), flattened into one shape for
/// `describe_symbol`/`search_stdlib` to serialize without matching on kind
/// at every call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: &'static str,
    pub kind: &'static str,              // "builtin" | "preset"
    pub signature: Option<&'static str>, // builtins only
    pub category: Option<&'static str>,  // presets only
    pub doc: &'static str,
    pub example: &'static str,
    /// Minimum argument count. Builtins: from `BuiltinSpec`. Presets: every
    /// FM preset in `lib/lib.wilios` is declared `func() { ... }` — zero
    /// parameters — so this is uniformly `0`.
    pub min_args: usize,
    /// Maximum argument count; `None` means variadic. Presets: uniformly
    /// `Some(0)`, matching `min_args`.
    pub max_args: Option<usize>,
}

/// All stdlib symbols (builtins + presets), flattened for lookup/search/did-you-mean.
///
/// `pub` so `wilios_core::diagnostics::suggest` (did-you-mean candidates)
/// and `wilios_core::resolve::checks` (arity checking) can consume it
/// directly rather than duplicating this table — the same instruction the
/// original `describe_symbol`/`search_stdlib` tools were built under.
pub fn all_symbols() -> impl Iterator<Item = Symbol> {
    let builtins = BUILTINS.iter().map(|b| Symbol {
        name: b.name,
        kind: "builtin",
        signature: Some(b.signature),
        category: None,
        doc: b.doc,
        example: b.example,
        min_args: b.min_args,
        max_args: b.max_args,
    });
    let presets = PRESETS.iter().map(|p| Symbol {
        name: p.name,
        kind: "preset",
        signature: None,
        category: Some(p.category),
        doc: p.doc,
        example: p.example,
        min_args: 0,
        max_args: Some(0),
    });
    builtins.chain(presets)
}

/// Exact (case-sensitive) name lookup across builtins and presets.
pub fn find(name: &str) -> Option<Symbol> {
    all_symbols().find(|s| s.name == name)
}

/// Case-insensitive substring search over name and doc text.
pub fn search(query: &str) -> Vec<Symbol> {
    let query = query.to_lowercase();
    all_symbols()
        .filter(|s| s.name.to_lowercase().contains(&query) || s.doc.to_lowercase().contains(&query))
        .collect()
}
