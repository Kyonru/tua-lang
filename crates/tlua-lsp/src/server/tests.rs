use super::*;
use serde_json::Value;
use std::io::Cursor;

#[test]
fn exposes_server_name() {
    assert_eq!(server_name(), "tlua-lsp");
}

#[test]
fn describes_tlua_language_server() {
    assert!(server_description().contains("language server"));
}

#[test]
fn initialize_returns_server_capabilities() {
    let output = run_messages(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    })]);

    assert_eq!(output.len(), 1);
    assert_eq!(output[0]["id"], 1);
    assert_eq!(output[0]["result"]["serverInfo"]["name"], "tlua-lsp");
    assert_eq!(output[0]["result"]["capabilities"]["textDocumentSync"], 1);
    assert_eq!(output[0]["result"]["capabilities"]["hoverProvider"], true);
}

#[test]
fn did_open_publishes_matching_core_diagnostics() {
    let uri = "file:///tmp/player.tlua";
    let output = run_messages(vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "tlua",
                    "version": 1,
                    "text": "local hp: number = \"full\"\n"
                }
            }
        }),
    ]);

    let diagnostics = published_diagnostics(&output, uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL1003");
    assert_eq!(diagnostics[0]["source"], "tlua");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 0);
    assert_eq!(diagnostics[0]["range"]["start"]["character"], 19);
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("type mismatch: expected number, found string"));
}

#[test]
fn did_change_republishes_empty_diagnostics_for_clean_document() {
    let uri = "file:///tmp/player.tlua";
    let output = run_messages(vec![
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "tlua",
                    "version": 1,
                    "text": "local hp: number = \"full\"\n"
                }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "version": 2
                },
                "contentChanges": [
                    {
                        "text": "local hp: number = 10\n"
                    }
                ]
            }
        }),
    ]);

    let publications = output
        .iter()
        .filter(|message| message["method"] == "textDocument/publishDiagnostics")
        .collect::<Vec<_>>();

    assert_eq!(publications.len(), 2);
    assert_eq!(
        publications[1]["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics should be an array")
            .len(),
        0
    );
}

#[test]
fn hover_returns_known_alias_local_function_and_table_field_types() {
    let uri = "file:///tmp/hover.tlua";
    let source = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number }

local player: Player = {
  pos = { x = 0, y = 0 },
  hp = 10,
}

local function damage(target: Player, amount: number): Player
  target.hp -= amount
  return target
end

damage(player, 2)
player.hp = player.hp - 1
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "Vec2"),
        hover_request(3, uri, source, "player:"),
        hover_request(4, uri, source, "damage(player"),
        hover_request(5, uri, source, "hp = player"),
    ]);

    assert_hover_contains(&output, 2, "type Vec2 = { x: number, y: number }");
    assert_hover_contains(&output, 3, "local player: Player");
    assert_hover_contains(
        &output,
        4,
        "function damage(target: Player, amount: number): Player",
    );
    assert_hover_contains(&output, 5, "(field) hp: number");
}

#[test]
fn hover_unknown_symbol_returns_null_result() {
    let uri = "file:///tmp/unknown-hover.tlua";
    let source = "local result = missing\n";
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "missing"),
    ]);

    let response = response_with_id(&output, 2);

    assert!(response["result"].is_null());
    assert!(response.get("error").is_none());
}

fn run_messages(messages: Vec<Value>) -> Vec<Value> {
    let mut input = Vec::new();

    for message in messages {
        let body = serde_json::to_vec(&message).expect("message should serialize");

        input.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
        input.extend_from_slice(&body);
    }

    let mut output = Vec::new();

    run(Cursor::new(input), &mut output).expect("server should run");

    read_output_messages(&output)
}

fn did_open(uri: &str, source: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": uri,
                "languageId": "tlua",
                "version": 1,
                "text": source
            }
        }
    })
}

fn hover_request(id: u64, uri: &str, source: &str, needle: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/hover",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_of(source, needle)
        }
    })
}

fn lsp_position_of(source: &str, needle: &str) -> Value {
    let index = source.find(needle).expect("needle should exist in source");
    let before = &source[..index];
    let line = before.bytes().filter(|byte| *byte == b'\n').count();
    let character = before
        .rsplit_once('\n')
        .map(|(_, line)| line.len())
        .unwrap_or(before.len());

    json!({
        "line": line,
        "character": character
    })
}

fn read_output_messages(mut output: &[u8]) -> Vec<Value> {
    let mut messages = Vec::new();

    while !output.is_empty() {
        let header_end = output
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("output should contain a header terminator");
        let header = std::str::from_utf8(&output[..header_end]).expect("header should be UTF-8");
        let content_length = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;

                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("valid content length"))
            })
            .expect("content length should exist");
        let body_start = header_end + 4;
        let body_end = body_start + content_length;
        let message =
            serde_json::from_slice(&output[body_start..body_end]).expect("valid JSON response");

        messages.push(message);
        output = &output[body_end..];
    }

    messages
}

fn published_diagnostics(output: &[Value], uri: &str) -> Vec<Value> {
    output
        .iter()
        .find(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == uri
        })
        .and_then(|message| message["params"]["diagnostics"].as_array())
        .cloned()
        .expect("diagnostics publication should exist")
}

fn assert_hover_contains(output: &[Value], id: u64, expected: &str) {
    let response = response_with_id(output, id);
    let contents = response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be a string");

    assert!(
        contents.contains(expected),
        "expected hover contents to contain {expected:?}, got {contents:?}"
    );
}

fn response_with_id(output: &[Value], id: u64) -> &Value {
    output
        .iter()
        .find(|message| message["id"] == id)
        .expect("response should exist")
}
