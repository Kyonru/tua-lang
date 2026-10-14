use super::*;

fn project_layout_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tua/projectLayout",
        "params": { "uri": uri }
    })
}

#[test]
fn project_layout_uses_canonical_toml_parsing_and_nested_project() {
    let outer = TempProject::new("project-layout-outer");
    outer.write("tua.toml", "source = \"outer\"\n");
    let nested = outer.path.join("examples/game");
    fs::create_dir_all(nested.join("src-code/love")).expect("nested source should exist");
    fs::write(
        nested.join("tua.toml"),
        "source = \"src\\u002dcode\"\nout = 'dist'\nmain = \"love/boot.tua\"\n",
    )
    .expect("nested config should be written");
    let source = nested.join("src-code/love/boot.tua");
    fs::write(&source, "return true\n").expect("nested source should be written");

    let output = run_messages(vec![
        initialize_with_root(1, outer.uri()),
        project_layout_request(2, &file_uri_from_path(&source)),
    ]);
    let layout = &response_with_id(&output, 2)["result"];

    assert_eq!(layout["workspaceUri"], outer.uri());
    assert_eq!(layout["projectUri"], file_uri_from_path(&nested));
    assert_eq!(
        layout["configUri"],
        file_uri_from_path(nested.join("tua.toml"))
    );
    assert_eq!(
        layout["sourceUri"],
        file_uri_from_path(nested.join("src-code"))
    );
    assert_eq!(layout["outputUri"], file_uri_from_path(nested.join("dist")));
    assert_eq!(layout["mainSource"], "love/boot.tua");
    assert_eq!(
        layout["generatedUri"],
        file_uri_from_path(nested.join("dist/main.lua"))
    );
    assert_eq!(layout["moduleName"], "love.boot");
}

#[test]
fn project_layout_selects_the_containing_multi_root_workspace() {
    let game = TempProject::new("project-layout-game");
    let tools = TempProject::new("project-layout-tools");
    game.write("tua.toml", "source = \"game-src\"\nout = \"game-out\"\n");
    tools.write("tua.toml", "source = \"tools-src\"\nout = \"tools-out\"\n");
    game.write("game-src/main.tua", "return true\n");
    tools.write("tools-src/task.tua", "return true\n");
    let game_uri = game.uri_for("game-src/main.tua");
    let tools_uri = tools.uri_for("tools-src/task.tua");

    let output = run_messages(vec![
        initialize_with_workspace_folders(1, [game.uri(), tools.uri()]),
        project_layout_request(2, &game_uri),
        project_layout_request(3, &tools_uri),
    ]);

    assert_eq!(
        response_with_id(&output, 2)["result"]["workspaceUri"],
        game.uri()
    );
    assert_eq!(
        response_with_id(&output, 3)["result"]["workspaceUri"],
        tools.uri()
    );
    assert_eq!(
        response_with_id(&output, 3)["result"]["sourceUri"],
        file_uri_from_path(tools.path.join("tools-src"))
    );
    assert_eq!(response_with_id(&output, 3)["result"]["moduleName"], "task");
}

#[test]
fn project_layout_does_not_read_outside_initialized_workspaces() {
    let workspace = TempProject::new("project-layout-workspace");
    let external = TempProject::new("project-layout-external");
    workspace.write("tua.toml", "source = \"src\"\n");
    external.write("tua.toml", "source = \"secret\"\n");
    external.write("secret/main.tua", "return true\n");

    let output = run_messages(vec![
        initialize_with_root(1, workspace.uri()),
        project_layout_request(2, &external.uri_for("secret/main.tua")),
    ]);

    assert_eq!(response_with_id(&output, 2)["result"], Value::Null);
}
