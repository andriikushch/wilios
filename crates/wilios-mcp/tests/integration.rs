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

fn call_tool_req(id: u32, name: &str, arguments: serde_json::Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments}
    })
    .to_string()
}

/// The `content[0].text` of a `tools/call` response, parsed as JSON — tool
/// results are returned as a JSON string inside a text content block.
fn tool_result_json(response: &serde_json::Value) -> serde_json::Value {
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("expected text content in tool result");
    serde_json::from_str(text).expect("tool result text was not valid JSON")
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

// ── tools/list ────────────────────────────────────────────────────────────────

const LIST_TOOLS: &str = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;

#[test]
fn list_tools_returns_describe_symbol_and_search_stdlib() {
    let responses = run(&[INIT, LIST_TOOLS]);
    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names.len(), 2, "expected exactly 2 tools, got: {names:?}");
    assert!(names.contains(&"describe_symbol"));
    assert!(names.contains(&"search_stdlib"));
}

// ── tools/call: describe_symbol ─────────────────────────────────────────────

#[test]
fn describe_symbol_returns_doc_for_known_builtin() {
    let req = call_tool_req(2, "describe_symbol", serde_json::json!({"name": "print"}));
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));
    let symbol = tool_result_json(&responses[1]);
    assert_eq!(symbol["name"], serde_json::json!("print"));
    assert_eq!(symbol["kind"], serde_json::json!("builtin"));
    assert!(symbol["signature"].as_str().unwrap().contains("print"));
}

#[test]
fn describe_symbol_returns_doc_for_known_preset() {
    let req = call_tool_req(2, "describe_symbol", serde_json::json!({"name": "epiano"}));
    let responses = run(&[INIT, &req]);
    let symbol = tool_result_json(&responses[1]);
    assert_eq!(symbol["name"], serde_json::json!("epiano"));
    assert_eq!(symbol["kind"], serde_json::json!("preset"));
    assert_eq!(symbol["category"], serde_json::json!("tonal"));
}

#[test]
fn describe_symbol_unknown_name_is_an_error_result() {
    let req = call_tool_req(
        2,
        "describe_symbol",
        serde_json::json!({"name": "does_not_exist"}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(
        responses[1]["result"]["isError"],
        serde_json::json!(true),
        "expected isError:true, got: {}",
        responses[1]
    );
}

// ── tools/call: search_stdlib ───────────────────────────────────────────────

#[test]
fn search_stdlib_finds_matches_by_name_substring() {
    let req = call_tool_req(2, "search_stdlib", serde_json::json!({"query": "trans"}));
    let responses = run(&[INIT, &req]);
    let matches = tool_result_json(&responses[1]);
    let names: Vec<&str> = matches
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"transpose"),
        "expected 'transpose' in {names:?}"
    );
}

#[test]
fn search_stdlib_no_matches_returns_empty_success() {
    let req = call_tool_req(
        2,
        "search_stdlib",
        serde_json::json!({"query": "zzznotfound"}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));
    let matches = tool_result_json(&responses[1]);
    assert_eq!(matches, serde_json::json!([]));
}

// ── naming-collision guard ──────────────────────────────────────────────────

#[test]
fn tool_names_do_not_shadow_stdlib_symbols() {
    for tool in ["describe_symbol", "search_stdlib"] {
        assert!(
            wilios_core::stdlib::find(tool).is_none(),
            "MCP tool '{tool}' shadows a DSL stdlib symbol — rename it (e.g. append _source)"
        );
    }
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
