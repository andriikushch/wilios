//! `doc/stdlib.md` is hand-copied into `wilios_core::interpreter::BUILTINS`
//! and `wilios_core::stdlib::PRESETS`. This test line-scans the doc's
//! consistent per-symbol structure (builtin `### \`name(...)\`` headings +
//! `**Signature:**` lines; the "Preset Overview" table) and asserts names,
//! builtin signatures, and preset categories can't silently drift apart from
//! the Rust tables. It deliberately does not diff full prose — `doc`/`example`
//! in the Rust tables are short derivatives of the doc's paragraphs, not
//! copies of them.

use std::collections::BTreeMap;

use wilios_core::interpreter::BUILTINS;
use wilios_core::stdlib::PRESETS;

const STDLIB_DOC: &str = include_str!("../../../doc/stdlib.md");

/// Returns the content between the first pair of backticks on `line`.
fn extract_backtick(line: &str) -> Option<&str> {
    let start = line.find('`')? + 1;
    let end = line[start..].find('`')?;
    Some(&line[start..start + end])
}

fn doc_section<'a>(doc: &'a str, start_heading: &str, end_heading: &str) -> &'a str {
    let start = doc
        .find(start_heading)
        .unwrap_or_else(|| panic!("doc/stdlib.md missing heading {start_heading:?}"));
    let end = doc[start..]
        .find(end_heading)
        .map(|i| start + i)
        .unwrap_or(doc.len());
    &doc[start..end]
}

/// name -> `**Signature:**` value, from the "## 1. Built-in Functions" section.
fn doc_builtin_signatures() -> BTreeMap<String, String> {
    let section = doc_section(
        STDLIB_DOC,
        "## 1. Built-in Functions",
        "## 2. Importing the Standard Library",
    );
    let lines: Vec<&str> = section.lines().collect();
    let mut result = BTreeMap::new();
    for (i, line) in lines.iter().enumerate() {
        // Builtin headings look like "### `print(...)`" — no numbering
        // prefix, unlike preset headings ("### 4.1 `epiano()`").
        if !line.starts_with("### `") {
            continue;
        }
        let Some(inner) = extract_backtick(line) else {
            continue;
        };
        let name = inner.split('(').next().unwrap().to_string();

        let signature = lines[i + 1..]
            .iter()
            .take_while(|l| !l.starts_with("### ") && !l.starts_with("## "))
            .find_map(|l| l.strip_prefix("**Signature:**").and_then(extract_backtick))
            .unwrap_or_else(|| {
                panic!("no **Signature:** line found for `{name}` in doc/stdlib.md")
            });

        result.insert(name, signature.to_string());
    }
    result
}

/// name -> category (lowercased), from the "## 3. Preset Overview" table.
fn doc_preset_categories() -> BTreeMap<String, String> {
    let section = doc_section(STDLIB_DOC, "## 3. Preset Overview", "## 4. Tonal Presets");
    let mut result = BTreeMap::new();
    for line in section.lines() {
        let line = line.trim();
        if !line.starts_with("| `") || !line.contains("()`") {
            continue;
        }
        let cells: Vec<&str> = line
            .split('|')
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
            .collect();
        let Some(name_cell) = cells.first() else {
            continue;
        };
        let name = name_cell
            .trim_matches('`')
            .trim_end_matches("()")
            .to_string();
        let category = cells
            .get(1)
            .unwrap_or_else(|| panic!("preset table row for `{name}` has no Category cell"))
            .to_lowercase();
        result.insert(name, category);
    }
    result
}

#[test]
fn builtin_names_match_doc() {
    // BTreeMap keys come out alphabetically; sort BUILTINS's declaration
    // order to match before comparing sets.
    let doc_names: Vec<String> = doc_builtin_signatures().into_keys().collect();
    let mut table_names: Vec<String> = BUILTINS.iter().map(|b| b.name.to_string()).collect();
    table_names.sort();
    assert_eq!(
        doc_names, table_names,
        "BUILTINS names must match doc/stdlib.md §1 headings exactly — a mismatch means a builtin \
         was added/removed on only one side"
    );
}

#[test]
fn builtin_signatures_match_doc() {
    let doc_sigs = doc_builtin_signatures();
    for b in BUILTINS {
        let doc_sig = doc_sigs.get(b.name).unwrap_or_else(|| {
            panic!(
                "`{}` is in BUILTINS but not documented in doc/stdlib.md §1",
                b.name
            )
        });
        assert_eq!(
            doc_sig, b.signature,
            "BuiltinSpec::signature for `{}` has drifted from doc/stdlib.md's **Signature:** line",
            b.name
        );
    }
}

#[test]
fn preset_names_match_doc() {
    let mut doc_names: Vec<String> = doc_preset_categories().into_keys().collect();
    doc_names.sort();
    let mut table_names: Vec<String> = PRESETS.iter().map(|p| p.name.to_string()).collect();
    table_names.sort();
    assert_eq!(
        doc_names, table_names,
        "PRESETS names must match doc/stdlib.md §3 Preset Overview table exactly"
    );
}

#[test]
fn preset_categories_match_doc() {
    let doc_categories = doc_preset_categories();
    for p in PRESETS {
        let doc_category = doc_categories.get(p.name).unwrap_or_else(|| {
            panic!(
                "`{}` is in PRESETS but not in doc/stdlib.md §3 table",
                p.name
            )
        });
        assert_eq!(
            doc_category, p.category,
            "PresetSpec::category for `{}` has drifted from doc/stdlib.md's Preset Overview table",
            p.name
        );
    }
}
