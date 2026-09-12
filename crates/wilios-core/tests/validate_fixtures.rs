//! Acceptance-criteria fixtures for `validate` (spec §11):
//! - A fixture of computed-argument cases that must validate clean (nothing
//!   requiring evaluation should ever be flagged).
//! - The real corpus (`examples/*.wilios`, `lib/lib.wilios`) must validate
//!   clean — if it doesn't, either the corpus or the validator is wrong,
//!   and both are worth knowing about.

use std::path::{Path, PathBuf};

use wilios_core::resolve::{ValidateOptions, validate_source};

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/wilios-core; workspace root is two levels up.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn opts(project_root: PathBuf) -> ValidateOptions {
    ValidateOptions {
        follow_imports: true,
        suggestions: true,
        lints: true,
        max_diagnostics: 50,
        excerpt: true,
        project_root,
    }
}

#[test]
fn computed_argument_fixtures_validate_clean() {
    let root = workspace_root();
    let fixtures = [
        // Duration denominator computed from a builtin call result.
        "let a = [1, 2, 3]\nlet n = len(a)\ntrack 0\nrest n/4\n",
        // Duration numerator computed from a variable.
        "let n = 4\ntrack 0\nrest n/4\n",
        // A random value used as a duration component.
        "let n = rand(2, 8)\ntrack 0\nrest n/4\n",
        // A builtin call with a variadic/valid arg count.
        "print(1, 2, 3)\nprint()\n",
        // transpose/len used exactly as documented.
        "let x = transpose(C4, 7)\nlet y = len([C4, E4, G4])\n",
    ];
    for src in fixtures {
        let outcome = validate_source(src, "<source>", None, None, &opts(root.clone())).unwrap();
        assert!(
            outcome.ok,
            "expected {src:?} to validate clean, got {:?}",
            outcome.diagnostics
        );
    }
}

#[test]
fn cookbook_examples_validate_clean() {
    let root = workspace_root();
    for rel in ["examples/example_1.wilios", "examples/example_swing.wilios"] {
        let path = root.join(rel);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {rel}: {e}"));
        let canonical = path.canonicalize().unwrap();
        let base_dir = canonical.parent().map(|p| p.to_path_buf());
        let outcome =
            validate_source(&text, rel, Some(canonical), base_dir, &opts(root.clone())).unwrap();
        // Zero diagnostics of any severity, not just zero errors — a
        // warning on the project's own flagship example is still a false
        // positive worth catching here.
        assert!(
            outcome.diagnostics.is_empty(),
            "expected {rel} to validate with no diagnostics at all, got {:?}",
            outcome.diagnostics
        );
    }
}

#[test]
fn lib_presets_validate_clean() {
    let root = workspace_root();
    let path = root.join("lib/lib.wilios");
    let text = std::fs::read_to_string(&path).unwrap();
    let canonical = path.canonicalize().unwrap();
    let base_dir = canonical.parent().map(|p| p.to_path_buf());
    let outcome = validate_source(
        &text,
        "lib/lib.wilios",
        Some(canonical),
        base_dir,
        &opts(root),
    )
    .unwrap();
    assert!(
        outcome.diagnostics.is_empty(),
        "expected lib/lib.wilios to validate with no diagnostics at all, got {:?}",
        outcome.diagnostics
    );
}
