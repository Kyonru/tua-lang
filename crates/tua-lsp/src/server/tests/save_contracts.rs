use super::*;

#[test]
fn imported_save_contract_keeps_snapshot_completion_and_hover() {
    let project = TempProject::new("save-contract-editor");
    project.write("tua.toml", "source = 'src'\n");
    let profile = "return Save.contract({slot = 'profile', version = 1, schema = Save.record({score = Save.number(0), name = Save.string('Ada')})})\n";
    project.write("src/profile.tua", profile);
    let source = "local Profile = require('profile')\nlocal initial = Profile:defaults()\nprint(initial.s)\n";
    project.write("src/main.tua", source);
    let uri = project.uri_for("src/main.tua");
    let profile_uri = project.uri_for("src/profile.tua");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, project.uri()));
    server.send(did_open(&uri, source));
    server.send(did_open(&profile_uri, profile));
    server.send(completion_request(2, &uri, source, "initial.s"));
    server.send(hover_request(3, &uri, source, "initial"));
    server.send(signature_help_request(4, &uri, source, "Profile:defaults("));
    server.send(hover_request(5, &uri, source, "defaults()"));
    server.send(hover_request(6, &profile_uri, profile, "contract({"));
    let output = server.output();
    assert_completion_labels(output, 2, ["score"]);
    assert_hover_contains(output, 3, "score");
    assert!(
        response_with_id(output, 4)["result"]["signatures"][0]["label"]
            .as_str()
            .is_some_and(|label| label.contains("defaults"))
    );
    assert!(
        response_with_id(output, 4)["result"]["signatures"][0]["documentation"]["value"]
            .as_str()
            .is_some_and(|doc| doc.contains("deep-copied"))
    );
    assert_hover_contains(output, 5, "deep-copied");
    assert_hover_contains(output, 6, "versioned save slot");
    server.send(did_change(
        &profile_uri,
        &profile.replace("score", "stamina"),
        2,
    ));
    server.send(completion_request(7, &uri, source, "initial.s"));
    assert_completion_labels(server.output(), 7, ["stamina"]);
}
