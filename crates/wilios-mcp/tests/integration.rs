use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/wilios-mcp; workspace root is two levels up
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Spawns the server, sends each request line, closes stdin, collects all responses.
fn run(requests: &[&str]) -> Vec<serde_json::Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_wilios-mcp"))
        .current_dir(workspace_root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn wilios-mcp");

    {
        let stdin = child.stdin.as_mut().unwrap();
        for req in requests {
            writeln!(stdin, "{req}").unwrap();
        }
    }
    child.stdin.take(); // close → server sees EOF and exits

    let stdout = BufReader::new(child.stdout.take().unwrap());
    let responses = stdout
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(&l).expect("server output was not valid JSON"))
        .collect();

    child.wait().ok();
    responses
}

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;

fn read_req(id: u32, uri: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"resources/read","params":{{"uri":"{uri}"}}}}"#)
}

// ── initialize ────────────────────────────────────────────────────────────────

#[test]
fn initialize_advertises_resources_capability() {
    let responses = run(&[INIT]);
    assert_eq!(responses.len(), 1);
    assert!(
        responses[0]["result"]["capabilities"]["resources"].is_object(),
        "expected resources capability, got: {}",
        responses[0]
    );
}

#[test]
fn initialize_reports_correct_server_name() {
    let responses = run(&[INIT]);
    assert_eq!(
        responses[0]["result"]["serverInfo"]["name"],
        serde_json::json!("wilios-mcp")
    );
}

// ── resources/list ────────────────────────────────────────────────────────────

const LIST: &str = r#"{"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}"#;

#[test]
fn list_resources_returns_five_resources() {
    let responses = run(&[INIT, LIST]);
    let resources = responses[1]["result"]["resources"].as_array().unwrap();
    assert_eq!(
        resources.len(),
        5,
        "expected 5 resources, got: {resources:?}"
    );
}

#[test]
fn list_resources_uris_and_mime_types() {
    let responses = run(&[INIT, LIST]);
    let resources = responses[1]["result"]["resources"].as_array().unwrap();

    let expected = [
        ("wilios://docs/language-reference", "text/markdown"),
        ("wilios://docs/grammar", "text/plain"),
        ("wilios://lib/presets", "text/plain"),
        ("wilios://examples/full-piece", "text/plain"),
        ("wilios://examples/swing", "text/plain"),
    ];

    for (uri, mime) in &expected {
        let entry = resources
            .iter()
            .find(|r| r["uri"] == *uri)
            .unwrap_or_else(|| panic!("resource {uri} not found in list"));
        assert_eq!(
            entry["mimeType"],
            serde_json::json!(mime),
            "wrong mimeType for {uri}"
        );
    }
}

// ── resources/read: embedded docs ────────────────────────────────────────────

#[test]
fn read_language_reference_returns_markdown() {
    let req = read_req(2, "wilios://docs/language-reference");
    let responses = run(&[INIT, &req]);
    let text = responses[1]["result"]["contents"][0]["text"]
        .as_str()
        .expect("expected text content");
    assert!(text.starts_with('#'), "expected markdown heading");
    assert!(
        text.contains("wilios"),
        "expected 'wilios' in language reference"
    );
}

#[test]
fn read_grammar_returns_ebnf() {
    let req = read_req(2, "wilios://docs/grammar");
    let responses = run(&[INIT, &req]);
    let text = responses[1]["result"]["contents"][0]["text"]
        .as_str()
        .expect("expected text content");
    assert!(text.contains("(*"), "expected EBNF comment markers");
    assert!(
        text.contains("program"),
        "expected 'program' rule in grammar"
    );
}

// ── resources/read: disk-based files ─────────────────────────────────────────

#[test]
fn read_lib_presets_contains_fm_presets() {
    let req = read_req(2, "wilios://lib/presets");
    let responses = run(&[INIT, &req]);
    let text = responses[1]["result"]["contents"][0]["text"]
        .as_str()
        .expect("expected text content");
    for preset in &["epiano", "brass", "bass", "kick", "snare"] {
        assert!(
            text.contains(preset),
            "expected preset '{preset}' in lib/lib.wilios"
        );
    }
}

#[test]
fn read_full_piece_example_contains_track_statements() {
    let req = read_req(2, "wilios://examples/full-piece");
    let responses = run(&[INIT, &req]);
    let text = responses[1]["result"]["contents"][0]["text"]
        .as_str()
        .expect("expected text content");
    assert!(
        text.contains("track"),
        "expected 'track' keyword in example"
    );
}

#[test]
fn read_swing_example_contains_swing_parameter() {
    let req = read_req(2, "wilios://examples/swing");
    let responses = run(&[INIT, &req]);
    let text = responses[1]["result"]["contents"][0]["text"]
        .as_str()
        .expect("expected text content");
    assert!(
        text.contains("swing"),
        "expected 'swing' keyword in swing example"
    );
}

// ── resources/read: error handling ───────────────────────────────────────────

#[test]
fn read_unknown_uri_returns_resource_not_found_error() {
    let req = read_req(2, "wilios://does/not/exist");
    let responses = run(&[INIT, &req]);
    let error = &responses[1]["error"];
    assert!(!error.is_null(), "expected error response");
    assert_eq!(
        error["code"],
        serde_json::json!(-32002),
        "expected resource-not-found code -32002"
    );
}
