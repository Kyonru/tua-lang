use super::*;

#[test]
fn document_symbol_returns_file_outline_tree() {
    let uri = "file:///tmp/document-symbol.tua";
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
    for symbol in symbols {
        assert_lsp_document_symbol_ranges_contain_selection(symbol);
    }
}
#[test]
fn document_symbol_unknown_document_returns_empty_array() {
    let uri = "file:///tmp/missing-document-symbol.tua";
    let output = run_messages(vec![document_symbol_request(2, uri)]);
    let response = response_with_id(&output, 2);

    assert_eq!(response["result"], json!([]));
}
#[test]
fn selection_range_grows_from_identifier_to_field_chain_statement_and_block() {
    let uri = "file:///tmp/selection-range.tua";
    let source = r#"
local function update(player: Player)
  player.hp = player.hp - 1
end
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        selection_range_request(2, uri, source, "hp ="),
    ]);
    let response = response_with_id(&output, 2);
    let result = response["result"]
        .as_array()
        .expect("selection ranges should be an array");
    let texts = selection_range_texts(source, &result[0]);

    assert_eq!(texts[0], "hp");
    assert_eq!(texts[1], "player.hp");
    assert!(texts.contains(&"player.hp = player.hp - 1"));
    assert!(texts
        .iter()
        .any(|text| text.contains("local function update")));
}
#[test]
fn folding_range_returns_nested_functions_and_table_constructors() {
    let uri = "file:///tmp/folding-range.tua";
    let source = r#"
local config = {
  player = {
    hp = 10,
  },
}

local function outer()
  local function inner()
    return config.player.hp
  end
end
"#;
    let output = run_messages(vec![did_open(uri, source), folding_range_request(2, uri)]);
    let response = response_with_id(&output, 2);
    let ranges = response["result"]
        .as_array()
        .expect("folding ranges should be an array");

    assert!(ranges.iter().any(|range| {
        range["kind"] == "region" && range["startLine"] == 1 && range["endLine"] == 5
    }));
    assert!(ranges.iter().any(|range| {
        range["kind"] == "region" && range["startLine"] == 2 && range["endLine"] == 4
    }));
    assert!(ranges
        .iter()
        .any(|range| range["startLine"] == 7 && range["endLine"] == 11));
    assert!(ranges
        .iter()
        .any(|range| range["startLine"] == 8 && range["endLine"] == 10));
}
#[test]
fn folding_range_is_stable_for_incomplete_sources() {
    let uri = "file:///tmp/incomplete-folding-range.tua";
    let source = r#"
-- docs
-- still docs
local config = {
  hp = 10,
"#;
    let output = run_messages(vec![did_open(uri, source), folding_range_request(2, uri)]);
    let response = response_with_id(&output, 2);
    let ranges = response["result"]
        .as_array()
        .expect("folding ranges should be an array");

    assert!(ranges.iter().any(|range| {
        range["kind"] == "comment" && range["startLine"] == 1 && range["endLine"] == 2
    }));
}
#[test]
fn code_lens_returns_unresolved_reference_lenses_for_exports_and_aliases() {
    let temp = TempProject::new("code-lens-unresolved");
    let player = r#"type Player = { name: string }

local function makePlayer(name: string): Player
  return { name = name }
end

local function helper()
  return makePlayer("local")
end

return {
  makePlayer = makePlayer,
}
"#;
    let main = r#"local Player = require("player")

local made: Player = Player.makePlayer("Ada")
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", player);
    temp.write("src/main.tua", main);

    let player_uri = temp.uri_for("src/player.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&player_uri, player),
        code_lens_request(2, &player_uri),
    ]);
    let lenses = code_lens_result(&output, 2);

    assert_eq!(code_lens_names(lenses), vec!["Player", "makePlayer"]);
    assert!(lenses.iter().all(|lens| lens.get("command").is_none()));
    assert!(lenses
        .iter()
        .all(|lens| lens["data"]["kind"] == "references"));
    assert_eq!(lenses[0]["data"]["symbolKind"], "typeAlias");
    assert_eq!(range_text(player, &lenses[0]["range"]), "Player");
    assert_eq!(lenses[1]["data"]["symbolKind"], "exportedFunction");
    assert_eq!(range_text(player, &lenses[1]["range"]), "makePlayer");
}
#[test]
fn code_lens_resolve_uses_text_document_references_result_set() {
    let temp = TempProject::new("code-lens-resolve");
    let player = r#"type Player = { name: string }

local function makePlayer(name: string): Player
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#;
    let main = r#"local Player = require("player")

local made = Player.makePlayer("Ada")
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", player);
    temp.write("src/main.tua", main);

    let player_uri = temp.uri_for("src/player.tua");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&player_uri, player));
    server.send(code_lens_request(2, &player_uri));

    let lens = code_lens_with_name(server.output(), 2, "makePlayer");

    server.send(code_lens_resolve_request(3, lens));
    server.send(references_request(
        4,
        &player_uri,
        player,
        "makePlayer(name",
        0,
        false,
    ));

    let resolved = &response_with_id(server.output(), 3)["result"];
    let references = &response_with_id(server.output(), 4)["result"];
    let locations = &resolved["command"]["arguments"][2];
    let reference_count = references
        .as_array()
        .expect("references result should be an array")
        .len();

    assert_eq!(
        resolved["command"]["title"],
        format!("{reference_count} references")
    );
    assert_eq!(
        resolved["command"]["command"],
        "editor.action.showReferences"
    );
    assert_eq!(locations, references);
}
#[test]
fn code_lens_omits_ambiguous_aliases_and_non_exported_functions() {
    let temp = TempProject::new("code-lens-ambiguous");
    let player = r#"type Player = { name: string }

local function helper(): Player
  return { name = "Ada" }
end

local cached = helper()
"#;
    let enemy = r#"type Player = { hp: number }
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", player);
    temp.write("src/enemy.tua", enemy);

    let player_uri = temp.uri_for("src/player.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&player_uri, player),
        code_lens_request(2, &player_uri),
    ]);

    assert_eq!(response_with_id(&output, 2)["result"], json!([]));
}
