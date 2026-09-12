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
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(&l).expect("server output was not valid JSON"))
        .collect();

    child.wait().ok();
    responses
}

/// Like [`run`], but waits for the response to each request (matched by `id`)
/// before sending the next — the server answers requests concurrently, so a
/// fast `resources/read` can otherwise overtake the `render` that populates it.
fn run_seq(requests: &[&str]) -> Vec<serde_json::Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_wilios-mcp"))
        .current_dir(workspace_root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn wilios-mcp");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut responses = Vec::new();

    for req in requests {
        writeln!(stdin, "{req}").unwrap();
        stdin.flush().unwrap();
        let want_id = serde_json::from_str::<serde_json::Value>(req).unwrap()["id"].clone();
        loop {
            let mut line = String::new();
            if stdout.read_line(&mut line).unwrap() == 0 {
                panic!("server closed stdout before answering {req}");
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let v: serde_json::Value =
                serde_json::from_str(line).expect("server output was not valid JSON");
            if v.get("id") == Some(&want_id) {
                responses.push(v);
                break;
            }
        }
    }

    drop(stdin);
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
    for preset in &["epiano", "brass", "trumpet", "bass", "kick", "snare"] {
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
    assert_eq!(names.len(), 5, "expected exactly 5 tools, got: {names:?}");
    assert!(names.contains(&"describe_symbol"));
    assert!(names.contains(&"search_stdlib"));
    assert!(names.contains(&"validate"));
    assert!(names.contains(&"dump_events"));
    assert!(names.contains(&"render"));
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

// ── tools/call: validate ─────────────────────────────────────────────────────

#[test]
fn validate_invalid_source_is_a_successful_call_with_ok_false() {
    // Spec §4: isError is reserved for tool-level failure; invalid *source*
    // must come back as isError:false with ok:false in the payload — unlike
    // describe_symbol's "unknown name -> CallToolResult::error" precedent,
    // which does not apply here.
    let req = call_tool_req(
        2,
        "validate",
        serde_json::json!({"source": "track 1\nlet x = transpse(C4, 7)\n<C4> 1/4\n"}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));
    let result = tool_result_json(&responses[1]);
    assert_eq!(result["ok"], serde_json::json!(false));
    let diagnostics = result["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], serde_json::json!("WIL-E3001"));
    assert!(
        diagnostics[0]["message"]
            .as_str()
            .unwrap()
            .contains("transpse")
    );
    let suggestions = diagnostics[0]["suggestions"].as_array().unwrap();
    assert_eq!(
        suggestions[0]["replacement"],
        serde_json::json!("transpose")
    );
    assert_eq!(suggestions[0]["confidence"], serde_json::json!("high"));
}

#[test]
fn validate_wrong_arity_builtin_call_is_flagged() {
    let req = call_tool_req(
        2,
        "validate",
        serde_json::json!({"source": "let n = len(1, 2)\n"}),
    );
    let responses = run(&[INIT, &req]);
    let result = tool_result_json(&responses[1]);
    assert_eq!(result["ok"], serde_json::json!(false));
    let diagnostics = result["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"] == serde_json::json!("WIL-E4001"))
    );
}

#[test]
fn validate_clean_source_reports_ok_true() {
    let req = call_tool_req(
        2,
        "validate",
        serde_json::json!({"source": "track 0\n<C4> 1/4\n"}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));
    let result = tool_result_json(&responses[1]);
    assert_eq!(result["ok"], serde_json::json!(true));
    assert_eq!(result["diagnostics"], serde_json::json!([]));
    assert_eq!(result["files_validated"], serde_json::json!(["<source>"]));
}

#[test]
fn validate_both_source_and_path_is_a_tool_error() {
    let req = call_tool_req(
        2,
        "validate",
        serde_json::json!({"source": "track 0\n<C4> 1/4\n", "path": "examples/example_1.wilios"}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(true));
}

#[test]
fn validate_neither_source_nor_path_is_a_tool_error() {
    let req = call_tool_req(2, "validate", serde_json::json!({}));
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(true));
}

#[test]
fn validate_path_escaping_the_sandbox_is_a_tool_error() {
    let req = call_tool_req(
        2,
        "validate",
        serde_json::json!({"path": "../outside.wilios"}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(true));
}

#[test]
fn validate_is_deterministic_across_repeated_calls() {
    let req = call_tool_req(
        2,
        "validate",
        serde_json::json!({"source": "track 1\nlet x = transpse(C4, 7)\n<C4> 1/4\n"}),
    );
    let a = run(&[INIT, &req]);
    let b = run(&[INIT, &req]);
    assert_eq!(tool_result_json(&a[1]), tool_result_json(&b[1]));
}

#[test]
fn validate_real_example_file_via_path_is_clean() {
    let req = call_tool_req(
        2,
        "validate",
        serde_json::json!({"path": "examples/example_1.wilios"}),
    );
    let responses = run(&[INIT, &req]);
    let result = tool_result_json(&responses[1]);
    assert_eq!(result["ok"], serde_json::json!(true));
    assert_eq!(result["diagnostics"], serde_json::json!([]));
    let files = result["files_validated"].as_array().unwrap();
    assert!(files.contains(&serde_json::json!("examples/example_1.wilios")));
    assert!(files.contains(&serde_json::json!("lib/lib.wilios")));
}

#[test]
fn validate_func_calling_another_func_is_clean() {
    // A phrase built from a sub-phrase: the call to `hit` inside `bar` must
    // resolve, and mutually-recursive top-level funcs must not be flagged.
    let req = call_tool_req(
        2,
        "validate",
        serde_json::json!({"source": concat!(
            "let hit = func() { <C4> 1/8 }\n",
            "let bar = func() { hit() hit() }\n",
            "let ping = func(k) { if (k > 0) { pong(k - 1) } }\n",
            "let pong = func(k) { if (k > 0) { ping(k - 1) } }\n",
            "track 1\nbar()\n",
        )}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));
    let result = tool_result_json(&responses[1]);
    assert_eq!(result["ok"], serde_json::json!(true));
    assert_eq!(result["diagnostics"], serde_json::json!([]));
}

// ── tools/call: dump_events ──────────────────────────────────────────────────

#[test]
fn dump_events_func_composed_of_subphrases_expands_in_timeline() {
    // `bar` calls `hit` twice; `hit` plays one note — so two notes land.
    let req = call_tool_req(
        2,
        "dump_events",
        serde_json::json!({"source": concat!(
            "let hit = func() { <C4> 1/8 }\n",
            "let bar = func() { hit() hit() }\n",
            "tempo 120\ntrack 1\nbar()\n",
        )}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));
    let result = tool_result_json(&responses[1]);
    assert_eq!(result["finished"], serde_json::json!(true));
    assert_eq!(result["note_count"], serde_json::json!(2));
    let notes = result["tracks"][0]["notes"].as_array().unwrap();
    assert_eq!(notes[0]["pitch"], serde_json::json!("C4"));
    assert_eq!(notes[1]["at_ms"], serde_json::json!(250)); // 1/8 @ 120 BPM
}

#[test]
fn dump_events_finite_source_returns_timeline() {
    let req = call_tool_req(
        2,
        "dump_events",
        serde_json::json!({"source": "tempo 120\ntrack 1\n<C4> 1/4\n<E4> 1/4\n"}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));
    let result = tool_result_json(&responses[1]);
    assert_eq!(result["finished"], serde_json::json!(true));
    assert_eq!(result["note_count"], serde_json::json!(2));
    let notes = result["tracks"][0]["notes"].as_array().unwrap();
    assert_eq!(notes[0]["pitch"], serde_json::json!("C4"));
    assert_eq!(notes[0]["dur_ms"], serde_json::json!(500)); // 1/4 @ 120 BPM
    assert_eq!(notes[1]["at_ms"], serde_json::json!(500));
}

#[test]
fn dump_events_roll_format_returns_an_ascii_piano_roll() {
    let req = call_tool_req(
        2,
        "dump_events",
        serde_json::json!({
            "source": "tempo 120\ntrack 1\n<C4> 1/4\n<E4> 1/4\n",
            "format": "roll",
        }),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));
    let text = responses[1]["result"]["content"][0]["text"]
        .as_str()
        .expect("expected text content");
    // Plain text, not the JSON EventDump.
    assert!(serde_json::from_str::<serde_json::Value>(text).is_err());
    assert!(text.contains("piano roll"));
    assert!(text.contains("track 1  (4/4, 2 notes"));
    assert!(text.contains("C4 "));
}

#[test]
fn dump_events_endless_loop_is_truncated_not_an_error() {
    let req = call_tool_req(
        2,
        "dump_events",
        serde_json::json!({"source": "track 1\nloop (true) {\n<C4> 1/8\n}\n", "max_ms": 2000}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));
    let result = tool_result_json(&responses[1]);
    assert_eq!(result["finished"], serde_json::json!(false));
    assert!(!result["tracks"][0]["notes"].as_array().unwrap().is_empty());
}

#[test]
fn dump_events_via_path_runs_the_example() {
    let req = call_tool_req(
        2,
        "dump_events",
        serde_json::json!({"path": "examples/example_1.wilios", "max_ms": 30000}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));
    let result = tool_result_json(&responses[1]);
    assert!(!result["tracks"].as_array().unwrap().is_empty());
    assert!(result["note_count"].as_u64().unwrap() > 0);
}

#[test]
fn dump_events_uncompilable_source_is_a_tool_error() {
    // Lex failure (`@` is not a valid character) — no diagnostic channel here,
    // so it surfaces as a tool-level error pointing at `validate`.
    let req = call_tool_req(
        2,
        "dump_events",
        serde_json::json!({"source": "track 1\n<C4> 1/4\n@@@\n"}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(true));
}

#[test]
fn dump_events_both_source_and_path_is_a_tool_error() {
    let req = call_tool_req(
        2,
        "dump_events",
        serde_json::json!({"source": "track 0\n<C4> 1/4\n", "path": "examples/example_1.wilios"}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(true));
}

#[test]
fn dump_events_path_escaping_the_sandbox_is_a_tool_error() {
    let req = call_tool_req(
        2,
        "dump_events",
        serde_json::json!({"path": "../outside.wilios"}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(true));
}

// ── tools/call: render ──────────────────────────────────────────────────────

fn b64_decode(s: &str) -> Vec<u8> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .expect("valid base64")
}

#[test]
fn render_finite_source_reports_analysis_and_a_spectrogram_link() {
    let req = call_tool_req(
        2,
        "render",
        serde_json::json!({
            "source": "tempo 120\ntrack 1\n<A4> 1/1\n",
            "want": ["analysis", "spectrogram"],
        }),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));

    let result = tool_result_json(&responses[1]);
    assert_eq!(result["finished"], serde_json::json!(true));
    assert_eq!(result["used_rng"], serde_json::json!(false));
    assert_eq!(
        result["analysis"]["tracks"][0]["note_count"],
        serde_json::json!(1)
    );

    let content = responses[1]["result"]["content"].as_array().unwrap();
    let has_spectrogram = content.iter().any(|c| {
        c["type"] == serde_json::json!("resource_link")
            && c["uri"]
                .as_str()
                .is_some_and(|u| u.ends_with("-spectrogram.png"))
    });
    assert!(
        has_spectrogram,
        "expected a spectrogram resource_link: {content:?}"
    );
    // No audio/waveform were requested.
    assert!(content.iter().all(|c| {
        c["uri"]
            .as_str()
            .is_none_or(|u| !u.ends_with(".wav") && !u.ends_with("-waveform.png"))
    }));
}

#[test]
fn render_want_analysis_only_returns_just_the_json_block() {
    let req = call_tool_req(
        2,
        "render",
        serde_json::json!({"source": "track 1\n<C4> 1/4\n", "want": ["analysis"]}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));
    let content = responses[1]["result"]["content"].as_array().unwrap();
    assert_eq!(
        content.len(),
        1,
        "expected only the JSON summary: {content:?}"
    );
    assert_eq!(content[0]["type"], serde_json::json!("text"));
}

#[test]
fn render_audio_resource_link_can_be_read_back_as_a_wav() {
    // First render of a fresh server process gets id 0.
    let call = call_tool_req(
        2,
        "render",
        serde_json::json!({"source": "tempo 120\ntrack 1\n<A4> 1/2\n", "want": ["audio"]}),
    );
    let read = read_req(3, "wilios://render/0000000000000000.wav");
    let responses = run_seq(&[INIT, &call, &read]);

    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));
    let blob = responses[2]["result"]["contents"][0]["blob"]
        .as_str()
        .expect("blob resource contents");
    let bytes = b64_decode(blob);
    assert_eq!(&bytes[0..4], b"RIFF", "not a RIFF container");
    assert_eq!(&bytes[8..12], b"WAVE", "not a WAVE file");
}

#[test]
fn render_endless_loop_is_truncated_not_an_error() {
    let req = call_tool_req(
        2,
        "render",
        serde_json::json!({
            "source": "track 1\nloop (true) {\n<C4> 1/8\n}\n",
            "max_ms": 2000,
            "want": ["analysis"],
        }),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(false));
    let result = tool_result_json(&responses[1]);
    assert_eq!(result["finished"], serde_json::json!(false));
}

#[test]
fn render_both_source_and_path_is_a_tool_error() {
    let req = call_tool_req(
        2,
        "render",
        serde_json::json!({"source": "track 1\n<C4> 1/4\n", "path": "examples/example_1.wilios"}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(true));
}

#[test]
fn render_path_escaping_the_sandbox_is_a_tool_error() {
    let req = call_tool_req(
        2,
        "render",
        serde_json::json!({"path": "../outside.wilios"}),
    );
    let responses = run(&[INIT, &req]);
    assert_eq!(responses[1]["result"]["isError"], serde_json::json!(true));
}

// ── naming-collision guard ──────────────────────────────────────────────────

#[test]
fn tool_names_do_not_shadow_stdlib_symbols() {
    for tool in [
        "describe_symbol",
        "search_stdlib",
        "validate",
        "dump_events",
        "render",
    ] {
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
