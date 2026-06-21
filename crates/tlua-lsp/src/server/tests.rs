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
    assert_eq!(
        output[0]["result"]["capabilities"]["completionProvider"]["triggerCharacters"],
        json!([":", "."])
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["definitionProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["documentSymbolProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["referencesProvider"],
        true
    );
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

#[test]
fn completion_returns_type_aliases_and_table_fields() {
    let uri = "file:///tmp/completion.tlua";
    let source = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

local player: Player = {
  pos = { x = 0, y = 0 },
  hp = 10,
}

local typed: Pl = nil
local field = player.hp
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "typed: Pl"),
        completion_request(3, uri, source, "player."),
    ]);

    assert_completion_labels(&output, 2, ["Player"]);
    assert_completion_labels(&output, 3, ["alive", "hp", "pos"]);
}

#[test]
fn completion_is_quiet_for_any_and_unknown_field_receivers() {
    let uri = "file:///tmp/quiet-completion.tlua";
    let source = r#"local payload: any = {}
local any_field = payload.value
local unknown_field = missing.value
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "payload."),
        completion_request(3, uri, source, "missing."),
    ]);

    assert_completion_labels(&output, 2, []);
    assert_completion_labels(&output, 3, []);
}

#[test]
fn definition_returns_known_alias_function_local_and_table_field_locations() {
    let uri = "file:///tmp/definition.tlua";
    let source = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

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
        definition_request(2, uri, source, "player: Player", "player: ".len()),
        definition_request(3, uri, source, "damage(player", 0),
        definition_request(4, uri, source, "damage(player", "damage(".len()),
        definition_request(5, uri, source, "player.hp =", "player.".len()),
    ]);

    assert_definition_text(&output, 2, uri, source, "Player");
    assert_definition_text(&output, 3, uri, source, "damage");
    assert_definition_text(&output, 4, uri, source, "player");
    assert_definition_text(&output, 5, uri, source, "hp");
}

#[test]
fn definition_unknown_symbol_returns_null_result() {
    let uri = "file:///tmp/unknown-definition.tlua";
    let source = "local result = missing\n";
    let output = run_messages(vec![
        did_open(uri, source),
        definition_request(2, uri, source, "missing", 0),
    ]);

    let response = response_with_id(&output, 2);

    assert!(response["result"].is_null());
    assert!(response.get("error").is_none());
}

#[test]
fn document_symbol_returns_file_outline_tree() {
    let uri = "file:///tmp/document-symbol.tlua";
    let source = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

local player: Player = nil

local function damage(target: Player, amount: number): Player
  return target
end

return {
  damage = damage,
  player = player,
  ["ignored"] = player,
  [player] = player,
  player,
}
"#;
    let output = run_messages(vec![did_open(uri, source), document_symbol_request(2, uri)]);
    let response = response_with_id(&output, 2);
    let symbols = response["result"]
        .as_array()
        .expect("document symbols should be an array");

    assert_eq!(
        symbol_names(symbols),
        vec!["Vec2", "Player", "player", "damage", "exports"]
    );
    assert_eq!(symbols[1]["kind"], 23);
    assert_eq!(
        symbols[1]["detail"],
        "type { pos: Vec2, hp: number, alive?: boolean }"
    );
    assert_eq!(
        symbol_names(
            symbols[1]["children"]
                .as_array()
                .expect("fields should exist")
        ),
        vec!["pos", "hp", "alive"]
    );
    assert_eq!(symbols[1]["children"][0]["kind"], 8);

    assert_eq!(symbols[2]["kind"], 13);
    assert_eq!(symbols[2]["detail"], "Player");

    assert_eq!(symbols[3]["kind"], 12);
    assert_eq!(symbols[3]["detail"], "returns Player");
    assert_eq!(
        symbol_names(
            symbols[3]["children"]
                .as_array()
                .expect("parameters should exist")
        ),
        vec!["target", "amount"]
    );
    assert_eq!(symbols[3]["children"][0]["kind"], 13);

    assert_eq!(symbols[4]["kind"], 19);
    assert_eq!(
        symbol_names(
            symbols[4]["children"]
                .as_array()
                .expect("exports should exist")
        ),
        vec!["damage", "player"]
    );
    assert!(symbols[0]["range"]["start"]["line"].is_number());
    assert!(symbols[0]["selectionRange"]["start"]["character"].is_number());
}

#[test]
fn document_symbol_unknown_document_returns_empty_array() {
    let uri = "file:///tmp/missing-document-symbol.tlua";
    let output = run_messages(vec![document_symbol_request(2, uri)]);
    let response = response_with_id(&output, 2);

    assert_eq!(response["result"], json!([]));
}

#[test]
fn references_returns_alias_function_local_and_table_field_locations() {
    let uri = "file:///tmp/references.tlua";
    let source = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

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

local payload: any = {}
local dynamic = payload.value
local unresolved = missing
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        references_request(2, uri, source, "player: Player", "player: ".len(), true),
        references_request(3, uri, source, "damage(player", 0, true),
        references_request(4, uri, source, "damage(player", "damage(".len(), true),
        references_request(5, uri, source, "target.hp", "target.".len(), true),
    ]);

    assert_reference_texts(&output, 2, source, ["Player", "Player", "Player", "Player"]);
    assert_reference_texts(&output, 3, source, ["damage", "damage"]);
    assert_reference_texts(&output, 4, source, ["player", "player", "player", "player"]);
    assert_reference_texts(&output, 5, source, ["hp", "hp", "hp", "hp"]);
}

#[test]
fn references_can_exclude_declarations_and_returns_empty_for_unknowns() {
    let uri = "file:///tmp/quiet-references.tlua";
    let source = r#"type Player = { hp: number }
local player: Player = { hp = 10 }
player.hp = player.hp - 1

local payload: any = {}
local dynamic = payload.value
local unresolved = missing
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        references_request(2, uri, source, "player: Player", "player: ".len(), false),
        references_request(3, uri, source, "payload.value", "payload.".len(), true),
        references_request(4, uri, source, "missing", 0, true),
    ]);

    assert_reference_texts(&output, 2, source, ["Player"]);
    assert_reference_texts(&output, 3, source, []);
    assert_reference_texts(&output, 4, source, []);
}

#[test]
fn references_unknown_document_returns_empty_array() {
    let uri = "file:///tmp/missing-references.tlua";
    let output = run_messages(vec![references_request(
        2,
        uri,
        "local value = missing\n",
        "missing",
        0,
        true,
    )]);
    let response = response_with_id(&output, 2);

    assert_eq!(response["result"], json!([]));
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

fn completion_request(id: u64, uri: &str, source: &str, needle: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/completion",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_after(source, needle)
        }
    })
}

fn definition_request(id: u64, uri: &str, source: &str, needle: &str, offset: usize) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/definition",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset)
        }
    })
}

fn document_symbol_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/documentSymbol",
        "params": {
            "textDocument": {
                "uri": uri
            }
        }
    })
}

fn references_request(
    id: u64,
    uri: &str,
    source: &str,
    needle: &str,
    offset: usize,
    include_declaration: bool,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/references",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset),
            "context": {
                "includeDeclaration": include_declaration
            }
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

fn lsp_position_with_offset(source: &str, needle: &str, offset: usize) -> Value {
    let mut position = lsp_position_of(source, needle);
    position["character"] = json!(
        position["character"]
            .as_u64()
            .expect("character should be numeric")
            + offset as u64
    );

    position
}

fn lsp_position_after(source: &str, needle: &str) -> Value {
    let mut position = lsp_position_of(source, needle);
    position["character"] = json!(
        position["character"]
            .as_u64()
            .expect("character should be numeric")
            + needle.chars().count() as u64
    );

    position
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

fn assert_completion_labels<const N: usize>(output: &[Value], id: u64, expected: [&str; N]) {
    let response = response_with_id(output, id);
    let labels = response["result"]
        .as_array()
        .expect("completion result should be an array")
        .iter()
        .map(|item| {
            item["label"]
                .as_str()
                .expect("completion label should be a string")
        })
        .collect::<Vec<_>>();

    assert_eq!(labels, expected);
}

fn assert_definition_text(output: &[Value], id: u64, uri: &str, source: &str, expected: &str) {
    let response = response_with_id(output, id);

    assert_eq!(response["result"]["uri"], uri);

    let range = &response["result"]["range"];
    let line = range["start"]["line"]
        .as_u64()
        .expect("definition start line should be numeric") as usize;
    let start = range["start"]["character"]
        .as_u64()
        .expect("definition start character should be numeric") as usize;
    let end = range["end"]["character"]
        .as_u64()
        .expect("definition end character should be numeric") as usize;
    let text = source
        .split('\n')
        .nth(line)
        .expect("definition line should exist");

    assert_eq!(&text[start..end], expected);
}

fn assert_reference_texts<const N: usize>(
    output: &[Value],
    id: u64,
    source: &str,
    expected: [&str; N],
) {
    let response = response_with_id(output, id);
    let texts = response["result"]
        .as_array()
        .expect("references result should be an array")
        .iter()
        .map(|location| range_text(source, &location["range"]))
        .collect::<Vec<_>>();

    assert_eq!(texts, expected);
}

fn range_text<'a>(source: &'a str, range: &Value) -> &'a str {
    let line = range["start"]["line"]
        .as_u64()
        .expect("range start line should be numeric") as usize;
    let start = range["start"]["character"]
        .as_u64()
        .expect("range start character should be numeric") as usize;
    let end = range["end"]["character"]
        .as_u64()
        .expect("range end character should be numeric") as usize;
    let text = source
        .split('\n')
        .nth(line)
        .expect("reference line should exist");

    &text[start..end]
}

fn symbol_names(symbols: &[Value]) -> Vec<&str> {
    symbols
        .iter()
        .map(|symbol| {
            symbol["name"]
                .as_str()
                .expect("symbol name should be a string")
        })
        .collect()
}

fn response_with_id(output: &[Value], id: u64) -> &Value {
    output
        .iter()
        .find(|message| message["id"] == id)
        .expect("response should exist")
}
