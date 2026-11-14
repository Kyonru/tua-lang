use super::*;

#[test]
fn localization_editor_requests_follow_catalog_overlay_and_cross_file_wrapper() {
    let project = TempProject::new("localization-editor");
    let config = "source = 'src'\n[i18n]\nenabled = true\n[[i18n.catalogs]]\nlocale = 'en'\npath = 'locales/en.lua'\n";
    let catalog = "return { en = { menu = { play = 'Play %{name}' } } }\n";
    let wrapper = "local function translate(key: I18nKey) return I18n.t(key) end\nreturn { translate = translate }\n";
    let source = "local translate = require('wrapper').translate\nlocal other = translate(\"menu.play\")\nlocal title = I18n.t(\"menu.play\", { name = 'Ada' })\nlocal incomplete = translate(\"menu.pl\")\n";
    project.write("tua.toml", config);
    project.write("src/locales/en.lua", catalog);
    project.write("src/wrapper.tua", wrapper);
    project.write("src/main.tua", source);
    let uri = project.uri_for("src/main.tua");
    let catalog_uri = project.uri_for("src/locales/en.lua");
    let output = run_messages(vec![
        initialize_with_root(1, project.uri()),
        did_open(&uri, source),
        completion_request(2, &uri, source, "translate(\"menu.pl"),
        hover_request(3, &uri, source, "menu.play"),
        definition_request(4, &uri, source, "menu.play", 2),
        references_request(5, &uri, source, "menu.play", 2, true),
        signature_help_request(6, &uri, source, "I18n.t(\"menu.play\", "),
    ]);
    assert_completion_labels(&output, 2, ["menu.play"]);
    assert_hover_contains(&output, 3, "Play %{name}");
    assert_eq!(response_with_id(&output, 4)["result"]["uri"], catalog_uri);
    assert!(response_with_id(&output, 5)["result"]
        .as_array()
        .is_some_and(|items| items.len() >= 3));
    assert!(
        response_with_id(&output, 6)["result"]["signatures"][0]["label"]
            .as_str()
            .is_some_and(|label| label.contains("I18nKey") && label.contains("I18nLocale"))
    );
}

#[test]
fn localization_catalog_request_reports_static_coverage_without_running_lua() {
    let project = TempProject::new("localization-catalog-view");
    project.write("tua.toml", "source = 'src'\n[i18n]\nenabled = true\ndefault_locale = 'en'\nfallback_locale = 'en'\n[[i18n.catalogs]]\nlocale = 'en'\npath = 'locales/en.lua'\n[[i18n.catalogs]]\nlocale = 'fr'\npath = 'locales/fr.lua'\n");
    project.write("src/main.tua", "print(I18n.t('menu.play'))\n");
    project.write(
        "src/locales/en.lua",
        "return { en = { menu = { play = 'Play', quit = 'Quit' } } }\n",
    );
    project.write(
        "src/locales/fr.lua",
        "return { fr = { menu = { play = 'Jouer' } } }\n",
    );
    let output = run_messages(vec![
        initialize_with_root(1, project.uri()),
        json!({"jsonrpc":"2.0","id":2,"method":"tua/localizationCatalog","params":{"search":"menu.","offset":0}}),
    ]);
    let result = &response_with_id(&output, 2)["result"];
    assert_eq!(result["enabled"], true);
    assert_eq!(result["total"], 2);
    let rows = result["rows"].as_array().unwrap();
    assert_eq!(rows[0]["key"], "menu.play");
    assert_eq!(rows[0]["cells"][1]["status"], "translated");
    assert_eq!(rows[1]["key"], "menu.quit");
    assert_eq!(rows[1]["cells"][1]["status"], "fallback");
}

#[test]
fn localization_unknown_key_action_edits_the_fallback_catalog() {
    let project = TempProject::new("localization-key-action");
    project.write("tua.toml", "source = 'src'\n[i18n]\nenabled = true\n[[i18n.catalogs]]\nlocale = 'en'\npath = 'locales/en.lua'\n");
    project.write(
        "src/locales/en.lua",
        "return { en = { menu = { play = 'Play' } } }\n",
    );
    let source = "local title = I18n.t('menu.missing')\n";
    project.write("src/main.tua", source);
    let uri = project.uri_for("src/main.tua");
    let catalog_uri = project.uri_for("src/locales/en.lua");
    let diagnostic = Diagnostic::error(
        project.path.join("src/main.tua"),
        1,
        22,
        "unknown i18n key `menu.missing`",
    )
    .with_code(tua_core::DiagnosticCode::DOMAIN_I18N)
    .with_range(1, 22, 1, 36);
    let output = run_messages(vec![
        initialize_with_root(1, project.uri()),
        did_open(&uri, source),
        code_action_request(
            2,
            &uri,
            0,
            23,
            0,
            24,
            vec![protocol::lsp_diagnostic_from_core(diagnostic, source)],
        ),
    ]);
    let action = response_with_id(&output, 2)["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["title"] == "Add `menu.missing` to fallback catalog")
        .expect("safe missing-key action");
    assert!(action["edit"]["changes"][&catalog_uri][0]["newText"]
        .as_str()
        .unwrap()
        .contains("missing = \"TODO\""));
}
