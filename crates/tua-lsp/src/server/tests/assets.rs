use super::*;

#[test]
fn asset_inventory_returns_grouped_usage_references_and_unused_diagnostics() {
    let temp = TempProject::new("asset-inventory");
    let main = r#"local player = image "assets/player.png"
"#;
    let ui = r#"local portrait = love.graphics.newImage("assets/player.png")
"#;

    temp.write("tua.toml", "source = \".\"\n");
    temp.write("main.tua", main);
    temp.write("ui.lua", ui);
    temp.write("assets/player.png", "");
    temp.write("assets/unused.wav", "");

    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        asset_inventory_request(2),
    ]);
    let result = &response_with_id(&output, 2)["result"];
    let inventory = &result["inventory"];
    let assets = inventory["assets"]
        .as_array()
        .expect("assets should be an array");
    let player = assets
        .iter()
        .find(|asset| asset["path"] == "assets/player.png")
        .expect("used image should be included");
    let unused = assets
        .iter()
        .find(|asset| asset["path"] == "assets/unused.wav")
        .expect("unused sound should be included");
    let diagnostics = inventory["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array");

    assert!(result["analysisRevision"].as_u64().is_some());
    assert_eq!(player["category"], "image");
    assert_eq!(player["usageCount"], 2);
    assert_eq!(
        player["references"]
            .as_array()
            .expect("references should be an array")
            .len(),
        2
    );
    assert_eq!(unused["category"], "audio");
    assert_eq!(unused["usageCount"], 0);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["uri"], temp.uri_for("assets/unused.wav"));
    assert_eq!(diagnostics[0]["diagnostic"]["code"], "TL2009");
    assert_eq!(diagnostics[0]["diagnostic"]["severity"], 4);
}

#[test]
fn asset_inventory_excludes_configured_output_copies() {
    for output_directory in ["build", "dist/game"] {
        let temp = TempProject::new("asset-inventory-output");
        temp.write(
            "tua.toml",
            &format!("source = \".\"\nout = \"{output_directory}\"\n"),
        );
        temp.write("main.tua", "local player = image \"assets/player.png\"\n");
        temp.write("assets/player.png", "");
        temp.write("assets/build/kept.png", "");
        temp.write(&format!("{output_directory}/assets/player.png"), "");
        temp.write(&format!("{output_directory}/assets/generated.png"), "");
        temp.write(&format!("{output_directory}-art/kept.png"), "");

        let output = run_messages(vec![
            initialize_with_root(1, temp.uri()),
            asset_inventory_request(2),
        ]);
        let inventory = &response_with_id(&output, 2)["result"]["inventory"];
        let assets = inventory["assets"].as_array().unwrap();
        assert_eq!(assets.len(), 3, "{output_directory}: {assets:?}");
        let player = assets
            .iter()
            .find(|asset| asset["path"] == "assets/player.png")
            .unwrap();
        assert_eq!(player["usageCount"], 1);
        assert!(assets
            .iter()
            .any(|asset| asset["path"] == "assets/build/kept.png"));
        assert!(assets
            .iter()
            .any(|asset| asset["path"] == format!("{output_directory}-art/kept.png")));
        assert_eq!(inventory["diagnostics"].as_array().unwrap().len(), 2);
    }
}
