//! Machine-readable index of the wilios "standard library" — the DSL's 4
//! built-in functions (see [`crate::interpreter::BUILTINS`]) plus the 9 FM
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

/// One of the 9 FM synthesis instrument presets in `lib/lib.wilios`. Unlike
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
        name: "bass",
        category: "tonal",
        doc: "Deep FM bass. A sub-octave modulator at high depth produces a thick, punchy transient over a clean fundamental.",
        example: "track 1\nbass()\n<A2> 1/4",
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
}

fn all_symbols() -> impl Iterator<Item = Symbol> {
    let builtins = BUILTINS.iter().map(|b| Symbol {
        name: b.name,
        kind: "builtin",
        signature: Some(b.signature),
        category: None,
        doc: b.doc,
        example: b.example,
    });
    let presets = PRESETS.iter().map(|p| Symbol {
        name: p.name,
        kind: "preset",
        signature: None,
        category: Some(p.category),
        doc: p.doc,
        example: p.example,
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
