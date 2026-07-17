use super::*;

#[test]
fn module_graph_returns_dependencies_cycles_reverse_counts_exports_and_inclusion_paths() {
    let temp = TempProject::new("module-graph-inspection");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/main.tua",
        "local Player = require(\"player\")\nlocal Missing = import(\"missing\")\nreturn { player = Player, missing = Missing }\n",
    );
    temp.write(
        "src/player.tua",
        "local Shared = require(\"shared\")\nreturn { hp = 100, shared = Shared }\n",
    );
    temp.write(
        "src/shared.lua",
        "local Player = require(\"player\")\nreturn { ready = true, player = Player }\n",
    );

    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        module_graph_request(2),
    ]);
    let result = &response_with_id(&output, 2)["result"];
    let graph = &result["graph"];
    let modules = graph["modules"]
        .as_array()
        .expect("modules should be an array");
    let edges = graph["edges"].as_array().expect("edges should be an array");
    let main = modules
        .iter()
        .find(|module| module["uri"] == temp.uri_for("src/main.tua"))
        .expect("main should be present");
    let player = modules
        .iter()
        .find(|module| module["uri"] == temp.uri_for("src/player.tua"))
        .expect("player should be present");
    let shared = modules
        .iter()
        .find(|module| module["uri"] == temp.uri_for("src/shared.lua"))
        .expect("shared should be present");
    let missing = edges
        .iter()
        .find(|edge| edge["specifier"] == "missing")
        .expect("missing import should be present");

    assert!(result["analysisRevision"].as_u64().is_some());
    assert_eq!(modules.len(), 3);
    assert_eq!(edges.len(), 4);
    assert_eq!(main["root"], true);
    assert_eq!(main["unresolvedDependencyCount"], 1);
    assert_eq!(player["reverseDependencyCount"], 2);
    assert_eq!(player["inCycle"], true);
    assert_eq!(shared["inCycle"], true);
    assert_eq!(shared["sourceKind"], "lua");
    assert_eq!(
        graph["cycles"][0]["moduleUris"]
            .as_array()
            .expect("cycle modules should be an array")
            .len(),
        2
    );
    assert_eq!(missing["kind"], "import");
    assert_eq!(missing["resolution"]["status"], "missing");
    assert_eq!(
        player["inclusionPath"]["steps"][0]["sourceUri"],
        temp.uri_for("src/main.tua")
    );
    assert!(player["exports"]
        .as_array()
        .expect("exports should be an array")
        .iter()
        .any(|export| export["name"] == "hp" && export["type"] == "number"));
}
