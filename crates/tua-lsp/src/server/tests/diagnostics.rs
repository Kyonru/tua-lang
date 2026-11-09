use super::*;

#[test]
fn diagnostic_severities_round_trip_through_lsp() {
    for (severity, expected) in [
        (DiagnosticSeverity::Error, 1),
        (DiagnosticSeverity::Warning, 2),
        (DiagnosticSeverity::Info, 3),
        (DiagnosticSeverity::Hint, 4),
    ] {
        let diagnostic = Diagnostic::new("src/main.tua", 3, 5, severity, "sample");
        let lsp = protocol::lsp_diagnostic_from_core(diagnostic, "");

        assert_eq!(lsp["severity"], expected);

        let round_tripped = protocol::core_diagnostic_from_lsp(Path::new("src/main.tua"), &lsp, "")
            .expect("diagnostic should round-trip");

        assert_eq!(round_tripped.severity, severity);
    }
}

#[test]
fn diagnostic_ranges_round_trip_through_lsp() {
    let diagnostic =
        Diagnostic::error("tua.toml", 2, 3, "unknown config key").with_range(2, 3, 2, 18);
    let lsp = protocol::lsp_diagnostic_from_core(diagnostic, "");

    assert_eq!(lsp["range"]["start"]["line"], 1);
    assert_eq!(lsp["range"]["start"]["character"], 2);
    assert_eq!(lsp["range"]["end"]["line"], 1);
    assert_eq!(lsp["range"]["end"]["character"], 17);

    let round_tripped = protocol::core_diagnostic_from_lsp(Path::new("tua.toml"), &lsp, "")
        .expect("diagnostic should round-trip");

    assert_eq!(round_tripped.line, 2);
    assert_eq!(round_tripped.column, 3);
    assert_eq!(round_tripped.end_line, 2);
    assert_eq!(round_tripped.end_column, 18);
}

#[test]
fn related_diagnostic_ranges_use_utf16_columns() {
    let source = "-- 😀 update";
    let diagnostic = Diagnostic::error("src/main.tua", 1, 1, "method does not satisfy protocol")
        .with_related_information(tua_core::DiagnosticRelatedInformation {
            file: PathBuf::from("src/main.tua"),
            span: tua_core::SourceSpan {
                start: SourcePosition { line: 1, column: 6 },
                end: SourcePosition {
                    line: 1,
                    column: 12,
                },
            },
            message: "protocol requirement `Updatable.update` is declared here".to_owned(),
        });
    let lsp = protocol::lsp_diagnostic_from_core(diagnostic, source);

    assert_eq!(
        lsp["relatedInformation"][0]["location"]["range"]["start"]["character"],
        6
    );
    assert_eq!(
        lsp["relatedInformation"][0]["location"]["range"]["end"]["character"],
        12
    );

    let round_tripped = protocol::core_diagnostic_from_lsp(Path::new("src/main.tua"), &lsp, source)
        .expect("related diagnostic should round-trip");
    assert_eq!(round_tripped.related_information.len(), 1);
    assert_eq!(
        round_tripped.related_information[0].span,
        tua_core::SourceSpan {
            start: SourcePosition { line: 1, column: 6 },
            end: SourcePosition {
                line: 1,
                column: 12,
            },
        }
    );
}

#[test]
fn code_diagnostics_underline_complete_identifier_tokens() {
    let uri = "file:///tmp/full-diagnostic-range.tua";
    let source = "local unusedPlayer = 1\nprint(missingPlayer)\n";
    let mut server = TestServer::new();

    server.send(did_open(uri, source));

    let diagnostics = last_published_diagnostics(server.output(), uri);
    let unused = diagnostics
        .iter()
        .find(|diagnostic| diagnostic["code"] == "TL4005")
        .expect("unused local warning should be published");
    let undefined = diagnostics
        .iter()
        .find(|diagnostic| diagnostic["code"] == "TL4004")
        .expect("undefined variable error should be published");

    assert_eq!(unused["range"]["start"]["character"], 6);
    assert_eq!(unused["range"]["end"]["character"], 18);
    assert_eq!(undefined["range"]["start"]["character"], 6);
    assert_eq!(undefined["range"]["end"]["character"], 19);
}

#[test]
fn did_open_publishes_matching_core_diagnostics() {
    let uri = "file:///tmp/player.tua";
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
                    "languageId": "tua",
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
    assert_eq!(diagnostics[0]["source"], "tua");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 0);
    assert_eq!(diagnostics[0]["range"]["start"]["character"], 19);
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("type mismatch at hp: expected number, found string"));
}

#[test]
fn did_open_publishes_actionable_module_scope_diagnostic_for_nested_protocol() {
    let uri = "file:///tmp/nested-protocol.tua";
    let output = run_messages(vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }),
        did_open(
            uri,
            "local function build()\n  protocol Hidden {\n    run(): nil\n  }\nend\n",
        ),
    ]);

    let diagnostics = published_diagnostics(&output, uri);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(diagnostics[0]["code"], "TL0001");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 1);
    assert_eq!(diagnostics[0]["range"]["start"]["character"], 2);
    assert_eq!(
        diagnostics[0]["message"],
        "protocol `Hidden` must be declared at module scope"
    );
}

#[test]
fn imported_protocol_conformance_diagnostics_are_published_in_implementing_file() {
    let temp = TempProject::new("cross-file-protocol-conformance");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/contracts.tua",
        "export protocol Updatable {\n  update(dt: number): nil\n}\n",
    );
    let actors = r#"import type { Updatable } from "./contracts"
final class Valid implements Updatable {
  update(dt: number): nil {
  }
}
final class Missing implements Updatable {
}
final class Bad implements Updatable {
  update(dt: string): nil {
  }
}
"#;
    let contracts_uri = temp.uri_for("src/contracts.tua");
    let actors_uri = temp.uri_for("src/actors.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(
            &contracts_uri,
            "export protocol Updatable {\n  update(dt: number): nil\n}\n",
        ),
        did_open(&actors_uri, actors),
    ]);
    let diagnostics = last_published_diagnostics(&output, &actors_uri);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic["message"].as_str().is_some_and(|message| {
                message.contains("Missing") && message.contains("missing method `update`")
            })
        }),
        "missing protocol method should be published on the implementing file: {diagnostics:#?}"
    );
    let bad = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic["message"].as_str().is_some_and(|message| {
                message.contains("Bad.update") && message.contains("does not satisfy protocol")
            })
        })
        .expect("incompatible protocol signature should be published on the implementing file");
    let related = bad["relatedInformation"]
        .as_array()
        .expect("protocol diagnostics should link to the contract requirement");
    assert_eq!(related.len(), 1, "{related:#?}");
    assert_eq!(related[0]["location"]["uri"], contracts_uri);
    assert_eq!(related[0]["location"]["range"]["start"]["line"], 1);
    assert_eq!(related[0]["location"]["range"]["start"]["character"], 2);
    assert_eq!(related[0]["location"]["range"]["end"]["line"], 1);
    assert_eq!(related[0]["location"]["range"]["end"]["character"], 8);
    assert!(related[0]["message"]
        .as_str()
        .is_some_and(|message| message.contains("Updatable.update")));
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic["message"].as_str().is_some_and(|message| {
                message.contains("Valid") && message.contains("does not satisfy protocol")
            })
        }),
        "valid protocol conformance should remain accepted: {diagnostics:#?}"
    );
}

#[test]
fn imported_generic_protocol_bound_diagnostics_use_project_conformance() {
    let temp = TempProject::new("imported-generic-protocol-bound");
    temp.write("tua.toml", "source = \"src\"\n");
    let contracts = "export protocol Updatable {\n  update(dt: number): nil\n}\n";
    let relay = "export type { Updatable } from \"./contracts\"\n";
    let api = "import type { Updatable } from \"./relay\"\nlocal function tick<T: Updatable>(value: T): T\n  value:update(1)\n  return value\nend\nreturn { tick = tick }\n";
    let actors = "import type { Updatable } from \"./contracts\"\nfinal class Actor implements Updatable {\n  update(dt: number): nil { return nil }\n}\nreturn { Actor = Actor }\n";
    let lookalikes = "final class Lookalike {\n  update(dt: number): nil { return nil }\n}\nreturn { Lookalike = Lookalike }\n";
    temp.write("src/contracts.d.tua", contracts);
    temp.write("src/relay.tua", relay);
    temp.write("src/api.tua", api);
    temp.write("src/actors.tua", actors);
    temp.write("src/lookalike.tua", lookalikes);
    let main = "local api = require(\"api\")\nlocal actors = require(\"actors\")\nlocal lookalikes = require(\"lookalike\")\nlocal actor = actors.Actor.new()\nlocal _valid = api.tick(actor)\nlocal _invalid = api.tick(lookalikes.Lookalike.new())\n";
    temp.write("src/main.tua", main);

    let contracts_uri = temp.uri_for("src/contracts.d.tua");
    let relay_uri = temp.uri_for("src/relay.tua");
    let api_uri = temp.uri_for("src/api.tua");
    let actors_uri = temp.uri_for("src/actors.tua");
    let lookalikes_uri = temp.uri_for("src/lookalike.tua");
    let main_uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&contracts_uri, contracts),
        did_open(&relay_uri, relay),
        did_open(&api_uri, api),
        did_open(&actors_uri, actors),
        did_open(&lookalikes_uri, lookalikes),
        did_open(&main_uri, main),
        text_document_diagnostic_request(2, &main_uri),
    ]);
    let diagnostics = response_with_id(&output, 2)["result"]["items"]
        .as_array()
        .expect("pull diagnostics should return items");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic["code"] == "TL1015"
                && diagnostic["message"].as_str().is_some_and(|message| {
                    message.contains("Lookalike") && message.contains("Updatable")
                })
        }),
        "the project-aware pass should retain the nonconforming class diagnostic: {diagnostics:#?}"
    );
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic["code"] == "TL1015"
                && diagnostic["message"].as_str().is_some_and(|message| {
                    message.contains("Actor") && message.contains("Updatable")
                })
        }),
        "the fast file-only diagnostic should not reject the imported explicit conformer: {diagnostics:#?}"
    );
}

#[test]
fn inherited_protocol_override_diagnostics_link_to_the_requirement() {
    let temp = TempProject::new("inherited-protocol-override-related-info");
    temp.write("tua.toml", "source = \"src\"\n");
    let source = r#"protocol Updatable {
  update(dt: number): nil
}
open class Actor implements Updatable {
  update(dt: number): nil {}
}
final class Panel extends Actor {
  override update(dt: string): nil {}
}
"#;
    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
    ]);
    let diagnostics = last_published_diagnostics(&output, &uri);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic["message"].as_str().is_some_and(|message| {
                message.contains("inherited protocol requirement `Updatable.update`")
            })
        })
        .expect("the broken inherited conformance should be diagnosed");

    assert_eq!(diagnostic["range"]["start"]["line"], 7);
    assert_eq!(diagnostic["range"]["start"]["character"], 11);
    let related = diagnostic["relatedInformation"]
        .as_array()
        .expect("the override error should link to the inherited requirement");
    assert_eq!(related.len(), 1);
    assert_eq!(related[0]["location"]["uri"], uri);
    assert_eq!(related[0]["location"]["range"]["start"]["line"], 1);
    assert_eq!(related[0]["location"]["range"]["start"]["character"], 2);
    assert_eq!(related[0]["location"]["range"]["end"]["character"], 8);
}

#[test]
fn inherited_override_diagnostics_link_to_imported_protocol_requirements() {
    let temp = TempProject::new("inherited-imported-protocol-override-related-info");
    temp.write("tua.toml", "source = \"src\"\n");
    let contracts = "export protocol Updatable {\n  update(dt: string): nil\n}\nexport protocol Steppable {\n  update(dt: string): nil\n}\n";
    let source = r#"import type { Updatable, Steppable } from "./contracts"
open class Parent implements Updatable, Steppable {
  update(dt: string): nil {}
}
final class Child extends Parent {
  override update(dt: number): nil {}
}
"#;
    let contracts_uri = temp.uri_for("src/contracts.tua");
    let actors_uri = temp.uri_for("src/actors.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&contracts_uri, contracts),
        did_open(&actors_uri, source),
    ]);
    let diagnostics = last_published_diagnostics(&output, &actors_uri);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic["message"].as_str().is_some_and(|message| {
                message.contains("inherited protocol requirements `Updatable.update`")
            })
        })
        .unwrap_or_else(|| {
            panic!("an imported inherited protocol violation should be diagnosed: {diagnostics:#?}")
        });

    assert_eq!(diagnostic["range"]["start"]["line"], 5);
    assert_eq!(diagnostic["range"]["start"]["character"], 11);
    let related = diagnostic["relatedInformation"]
        .as_array()
        .expect("the override error should link to the imported requirement");
    assert_eq!(related.len(), 2);
    assert_eq!(related[0]["location"]["uri"], contracts_uri);
    assert_eq!(related[0]["location"]["range"]["start"]["line"], 1);
    assert_eq!(related[0]["location"]["range"]["start"]["character"], 2);
    assert_eq!(related[0]["location"]["range"]["end"]["character"], 8);
    assert_eq!(related[1]["location"]["uri"], contracts_uri);
    assert_eq!(related[1]["location"]["range"]["start"]["line"], 4);
    assert_eq!(related[1]["location"]["range"]["start"]["character"], 2);
    assert_eq!(related[1]["location"]["range"]["end"]["character"], 8);
}

#[test]
fn inherited_override_diagnostics_link_to_requirements_in_each_imported_file() {
    let temp = TempProject::new("inherited-override-multiple-imported-requirements");
    temp.write("tua.toml", "source = \"src\"\n");
    let updatable =
        "-- 😀 first contract\nexport protocol Updatable {\n  update(dt: string): nil\n}\n";
    let steppable =
        "-- 🧪 second contract\nexport protocol Steppable {\n  update(dt: string): nil\n}\n";
    let source = r#"import type { Updatable } from "./updatable"
import type { Steppable } from "./steppable"
-- 🧙‍♂️ consumer
open class Parent implements Updatable, Steppable {
  update(dt: string): nil {}
}
final class Child extends Parent {
  override update(dt: number): nil {}
}
"#;
    let updatable_uri = temp.uri_for("src/updatable.tua");
    let steppable_uri = temp.uri_for("src/steppable.tua");
    let actors_uri = temp.uri_for("src/actors.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&updatable_uri, updatable),
        did_open(&steppable_uri, steppable),
        did_open(&actors_uri, source),
    ]);
    let diagnostics = last_published_diagnostics(&output, &actors_uri);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic["message"].as_str().is_some_and(|message| {
                message.contains("inherited protocol requirements `Updatable.update`")
                    && message.contains("`Steppable.update`")
            })
        })
        .unwrap_or_else(|| {
            panic!("the override should identify every inherited protocol: {diagnostics:#?}")
        });

    assert_eq!(diagnostic["range"]["start"]["line"], 7);
    assert_eq!(diagnostic["range"]["start"]["character"], 11);
    assert_eq!(diagnostic["range"]["end"]["line"], 7);
    assert_eq!(diagnostic["range"]["end"]["character"], 17);
    let related = diagnostic["relatedInformation"]
        .as_array()
        .expect("each imported contract should be navigable");
    assert_eq!(related.len(), 2, "{diagnostic:#?}");
    assert_eq!(related[0]["location"]["uri"], updatable_uri);
    assert_eq!(related[0]["location"]["range"]["start"]["line"], 2);
    assert_eq!(related[0]["location"]["range"]["start"]["character"], 2);
    assert_eq!(related[0]["location"]["range"]["end"]["line"], 2);
    assert_eq!(related[0]["location"]["range"]["end"]["character"], 8);
    assert_eq!(related[1]["location"]["uri"], steppable_uri);
    assert_eq!(related[1]["location"]["range"]["start"]["line"], 2);
    assert_eq!(related[1]["location"]["range"]["start"]["character"], 2);
    assert_eq!(related[1]["location"]["range"]["end"]["line"], 2);
    assert_eq!(related[1]["location"]["range"]["end"]["character"], 8);
}

#[test]
fn unsaved_protocol_edit_refreshes_importing_class_diagnostics() {
    let temp = TempProject::new("unsaved-protocol-conformance-refresh");
    temp.write("tua.toml", "source = \"src\"\n");
    let contracts = "export protocol Updatable {\n  update(dt: number): nil\n}\n";
    let changed_contracts = "export protocol Updatable {\n  update(dt: string): nil\n}\n";
    let incomplete_contracts = "export protocol Updatable {\n  update(dt: string):\n";
    temp.write("src/contracts.tua", contracts);
    let actors = r#"import type { Updatable } from "./contracts"
final class Particle implements Updatable {
  update(dt: number): nil {}
}
"#;
    let contracts_uri = temp.uri_for("src/contracts.tua");
    let actors_uri = temp.uri_for("src/actors.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(1, temp.uri(), true));
    server.send(did_open(&contracts_uri, contracts));
    server.send(did_open(&actors_uri, actors));
    server.send(text_document_diagnostic_request(2, &actors_uri));
    assert!(document_diagnostic_items(server.output(), 2).is_empty());

    server.send(did_change(&contracts_uri, incomplete_contracts, 2));
    server.send(text_document_diagnostic_request(3, &actors_uri));
    let diagnostics = document_diagnostic_items(server.output(), 3);
    assert!(
        diagnostics.is_empty(),
        "an incomplete contract edit should keep last-good requirements for dependent diagnostics: {diagnostics:#?}"
    );

    server.send(did_change(&contracts_uri, changed_contracts, 3));
    server.send(text_document_diagnostic_request(4, &actors_uri));
    let diagnostics = document_diagnostic_items(server.output(), 4);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["message"].as_str().is_some_and(|message| {
            message.contains("Particle.update") && message.contains("does not satisfy protocol")
        })
    }), "an unsaved requirement edit should update dependent conformance diagnostics: {diagnostics:#?}");
}

#[test]
fn deleting_a_protocol_dependency_refreshes_importing_class_diagnostics() {
    let temp = TempProject::new("deleted-protocol-dependency-refresh");
    temp.write("tua.toml", "source = \"src\"\n");
    let contracts = "export protocol Updatable {\n  update(dt: number): nil\n}\n";
    temp.write("src/contracts.tua", contracts);
    let actors = r#"import type { Updatable } from "./contracts"
final class Particle implements Updatable {
  update(dt: number): nil {}
}
"#;
    temp.write("src/actors.tua", actors);
    let contracts_uri = temp.uri_for("src/contracts.tua");
    let actors_uri = temp.uri_for("src/actors.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(1, temp.uri(), true));
    server.send(did_open(&contracts_uri, contracts));
    server.send(did_open(&actors_uri, actors));
    server.send(text_document_diagnostic_request(2, &actors_uri));
    assert!(document_diagnostic_items(server.output(), 2).is_empty());

    server.send(did_close(&contracts_uri));
    temp.remove("src/contracts.tua");
    server.send(watched_files_changed([(&contracts_uri, 3)]));
    server.send(text_document_diagnostic_request(3, &actors_uri));
    let diagnostics = document_diagnostic_items(server.output(), 3);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic["code"] == "TL4002"
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("import file not found: `./contracts`"))
        }),
        "deleting a protocol file should report the missing import: {diagnostics:#?}"
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic["code"] != "TL1008"),
        "consumers must not keep checking against the deleted last-good protocol contract: {diagnostics:#?}"
    );
}

#[test]
fn creating_and_editing_a_protocol_dependency_refreshes_import_diagnostics() {
    let temp = TempProject::new("created-protocol-dependency-refresh");
    temp.write("tua.toml", "source = \"src\"\n");
    let compatible = "export protocol Updatable {\n  update(dt: number): nil\n}\n";
    let incompatible = "export protocol Updatable {\n  update(dt: string): nil\n}\n";
    let actors = r#"import type { Updatable } from "./contracts"
final class Particle implements Updatable {
  update(dt: number): nil {}
}
"#;
    let contracts_uri = temp.uri_for("src/contracts.tua");
    let actors_uri = temp.uri_for("src/actors.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(1, temp.uri(), true));
    server.send(did_open(&actors_uri, actors));
    server.send(text_document_diagnostic_request(2, &actors_uri));
    let diagnostics = document_diagnostic_items(server.output(), 2);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic["code"] == "TL4002"
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("import file not found: `./contracts`"))
        }),
        "the unresolved type-only protocol import should be diagnosed: {diagnostics:#?}"
    );

    temp.write("src/contracts.tua", compatible);
    server.send(watched_files_changed([(&contracts_uri, 1)]));
    server.send(text_document_diagnostic_request(3, &actors_uri));
    let diagnostics = document_diagnostic_items(server.output(), 3);
    assert!(
        diagnostics.is_empty(),
        "creating a compatible protocol module should clear import errors: {diagnostics:#?}"
    );

    temp.write("src/contracts.tua", incompatible);
    server.send(watched_files_changed([(&contracts_uri, 2)]));
    server.send(text_document_diagnostic_request(4, &actors_uri));
    let diagnostics = document_diagnostic_items(server.output(), 4);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic["message"].as_str().is_some_and(|message| {
                message.contains("Particle.update") && message.contains("does not satisfy protocol")
            })
        }),
        "editing the created contract should refresh conformance: {diagnostics:#?}"
    );
}

#[test]
fn deleting_a_protocol_from_a_cyclic_import_graph_discards_stale_contracts() {
    let temp = TempProject::new("deleted-cyclic-protocol-dependency-refresh");
    temp.write("tua.toml", "source = \"src\"\n");
    let protocol_a = "import type { B } from \"./b\"\nexport protocol A {\n  f(value: B): nil\n}\n";
    let protocol_b = "import type { A } from \"./a\"\nexport protocol B {\n  g(value: A): nil\n}\n";
    temp.write("src/a.tua", protocol_a);
    temp.write("src/b.tua", protocol_b);
    let actors = r#"import type { A } from "./a"
import type { B } from "./b"
final class Actor implements A, B {
  f(value: B): nil {}
  g(value: A): nil {}
}
"#;
    temp.write("src/actors.tua", actors);
    let a_uri = temp.uri_for("src/a.tua");
    let b_uri = temp.uri_for("src/b.tua");
    let actors_uri = temp.uri_for("src/actors.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(1, temp.uri(), true));
    server.send(did_open(&a_uri, protocol_a));
    server.send(did_open(&b_uri, protocol_b));
    server.send(did_open(&actors_uri, actors));
    server.send(text_document_diagnostic_request(2, &actors_uri));
    assert!(document_diagnostic_items(server.output(), 2).is_empty());

    server.send(did_close(&a_uri));
    temp.remove("src/a.tua");
    server.send(watched_files_changed([(&a_uri, 3)]));
    server.send(text_document_diagnostic_request(3, &actors_uri));
    let diagnostics = document_diagnostic_items(server.output(), 3);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic["code"] == "TL4002"
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("import file not found: `./a`"))
        }),
        "deleting one side of a cyclic graph should report the unresolved import: {diagnostics:#?}"
    );
    assert!(
        diagnostics.iter().all(|diagnostic| {
            !diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("does not satisfy protocol `A.f`"))
        }),
        "the consumer must not receive stale conformance diagnostics from the deleted protocol: {diagnostics:#?}"
    );
}

#[test]
fn inherited_protocol_override_related_ranges_survive_unicode_file_prefixes() {
    let temp = TempProject::new("inherited-protocol-override-utf16-ranges");
    temp.write("tua.toml", "source = \"src\"\n");
    let contracts = "-- 😀 contract\nexport protocol Updatable {\n  update(dt: string): nil\n}\n";
    let actors = r#"import type { Updatable } from "./contracts"
-- 😀 consumer
open class Parent implements Updatable {
  update(dt: string): nil {}
}
final class Child extends Parent {
  override update(dt: number): nil {}
}
"#;
    temp.write("src/contracts.tua", contracts);
    temp.write("src/actors.tua", actors);
    let contracts_uri = temp.uri_for("src/contracts.tua");
    let actors_uri = temp.uri_for("src/actors.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(1, temp.uri(), true));
    server.send(did_open(&contracts_uri, contracts));
    server.send(did_open(&actors_uri, actors));
    server.send(text_document_diagnostic_request(2, &actors_uri));
    let diagnostics = document_diagnostic_items(server.output(), 2);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("method `update` has a signature"))
        })
        .expect("the incompatible inherited override should be diagnosed");

    assert_eq!(diagnostic["range"]["start"]["line"], 6);
    assert_eq!(diagnostic["range"]["start"]["character"], 11);
    assert_eq!(diagnostic["range"]["end"]["line"], 6);
    assert_eq!(diagnostic["range"]["end"]["character"], 17);
    let related = diagnostic["relatedInformation"]
        .as_array()
        .expect("the related protocol declaration should be preserved");
    assert_eq!(related.len(), 1, "{diagnostic:#?}");
    assert_eq!(related[0]["location"]["uri"], contracts_uri);
    assert_eq!(related[0]["location"]["range"]["start"]["line"], 2);
    assert_eq!(related[0]["location"]["range"]["start"]["character"], 2);
    assert_eq!(related[0]["location"]["range"]["end"]["line"], 2);
    assert_eq!(related[0]["location"]["range"]["end"]["character"], 8);
}

#[test]
fn renaming_a_protocol_dependency_refreshes_importing_class_diagnostics() {
    let temp = TempProject::new("renamed-protocol-dependency-refresh");
    temp.write("tua.toml", "source = \"src\"\n");
    let contracts = "export protocol Updatable {\n  update(dt: number): nil\n}\n";
    let changed_contracts = "export protocol Updatable {\n  update(dt: string): nil\n}\n";
    temp.write("src/contracts.tua", contracts);
    let actors = r#"import type { Updatable } from "./contracts"
final class Particle implements Updatable {
  update(dt: number): nil {}
}
"#;
    temp.write("src/actors.tua", actors);
    let contracts_uri = temp.uri_for("src/contracts.tua");
    let renamed_uri = temp.uri_for("src/api.tua");
    let actors_uri = temp.uri_for("src/actors.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(1, temp.uri(), true));
    server.send(did_open(&contracts_uri, contracts));
    server.send(did_open(&actors_uri, actors));
    server.send(text_document_diagnostic_request(2, &actors_uri));
    assert!(document_diagnostic_items(server.output(), 2).is_empty());

    server.send(did_close(&contracts_uri));
    temp.remove("src/contracts.tua");
    temp.write("src/api.tua", changed_contracts);
    server.send(watched_files_changed([
        (&contracts_uri, 3),
        (&renamed_uri, 1),
    ]));
    server.send(did_open(&renamed_uri, changed_contracts));
    server.send(text_document_diagnostic_request(3, &actors_uri));
    let unresolved = document_diagnostic_items(server.output(), 3);
    assert!(
        unresolved
            .iter()
            .any(|diagnostic| diagnostic["code"] == "TL4002"),
        "the old path should become unresolved immediately after the rename: {unresolved:#?}"
    );

    let renamed_actors = actors.replace("./contracts", "./api");
    server.send(did_change(&actors_uri, &renamed_actors, 2));
    let due = server
        .next_slow_diagnostics_due()
        .expect("edited-document diagnostics should be scheduled");
    server.drain_due_diagnostics(due);
    server.send(text_document_diagnostic_request(4, &actors_uri));
    let diagnostics = document_diagnostic_items(server.output(), 4);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic["message"].as_str().is_some_and(|message| {
                message.contains("Particle.update")
                    && message.contains("does not satisfy protocol `Updatable.update`")
                    && message.contains("expected")
                    && message.contains("found")
            })
        }),
        "the renamed dependency's changed contract should be used after watcher and consumer edits: {diagnostics:#?}"
    );
    assert!(
        diagnostics.iter().all(|diagnostic| {
            !diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("import file not found"))
        }),
        "the consumer should no longer resolve the old dependency path: {diagnostics:#?}"
    );
}

#[test]
fn protocol_conformance_resolves_through_type_reexports() {
    let temp = TempProject::new("protocol-conformance-type-reexport");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/contracts.d.tua",
        "export protocol Updatable {\n  update(dt: number): nil\n}\n",
    );
    temp.write(
        "src/public.d.tua",
        "export type { Updatable } from \"./contracts\"\n",
    );
    let actors = r#"import type { Updatable } from "./public"
final class Particle implements Updatable {
  update(dt: number): nil {}
}

final class Broken implements Updatable {}
"#;
    let actors_uri = temp.uri_for("src/actors.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(1, temp.uri(), true));
    server.send(did_open(&actors_uri, actors));
    server.send(text_document_diagnostic_request(2, &actors_uri));
    let diagnostics = document_diagnostic_items(server.output(), 2);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic["message"].as_str().is_some_and(|message| {
                message.contains("Broken") && message.contains("missing method `update`")
            })
        }),
        "re-exported protocol obligation should be diagnosed: {diagnostics:#?}"
    );
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic["message"].as_str().is_some_and(|message| {
                message.contains("Particle") && message.contains("does not satisfy protocol")
            })
        }),
        "valid conformance through a re-export should not be diagnosed: {diagnostics:#?}"
    );
}

#[test]
fn namespace_imported_protocol_conformance_diagnostics_resolve_from_declarations() {
    let temp = TempProject::new("namespace-imported-protocol-diagnostic");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/contracts.d.tua",
        "export protocol Updatable {\n  update(dt: number): nil\n}\n",
    );
    let actors = r#"import type Contracts from "./contracts"
final class Valid implements Contracts.Updatable {
  update(dt: number): nil {}
}
final class Missing implements Contracts.Updatable {}
"#;
    let actors_uri = temp.uri_for("src/actors.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(1, temp.uri(), true));
    server.send(did_open(&actors_uri, actors));
    server.send(text_document_diagnostic_request(2, &actors_uri));
    let diagnostics = document_diagnostic_items(server.output(), 2);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic["message"].as_str().is_some_and(|message| {
                message.contains("Missing") && message.contains("missing method `update`")
            })
        }),
        "a namespace-imported declaration protocol should diagnose missing methods: {diagnostics:#?}"
    );
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic["message"].as_str().is_some_and(|message| {
                message.contains("Valid") && message.contains("does not satisfy protocol")
            })
        }),
        "valid namespace-imported conformance should remain accepted: {diagnostics:#?}"
    );
}

#[test]
fn class_method_diagnostics_point_to_the_original_error_expression() {
    let uri = "file:///tmp/class-method-diagnostic.tua";
    let source = r#"final class Counter {
  value: number

  init() {
    self.value = "wrong"
  }
}
"#;
    let mut server = TestServer::new();

    server.send(did_open(uri, source));

    let diagnostic = last_published_diagnostics(server.output(), uri)
        .into_iter()
        .find(|diagnostic| diagnostic["code"] == "TL1003")
        .expect("class body mismatch should be published");

    assert_eq!(diagnostic["range"]["start"]["line"], 4);
    assert_eq!(diagnostic["range"]["start"]["character"], 17);
    assert_eq!(diagnostic["range"]["end"]["line"], 4);
    assert_eq!(diagnostic["range"]["end"]["character"], 24);
}

#[test]
fn did_open_rejects_luau_syntax_as_tl0004() {
    let uri = "file:///tmp/luau-only.tua";
    let mut server = TestServer::new();

    server.send(did_open(uri, "while running do\n  continue\nend\n"));

    let diagnostics = last_published_diagnostics(server.output(), uri)
        .into_iter()
        .filter(|diagnostic| diagnostic["code"] == "TL0004")
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL0004");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 1);
    assert_eq!(diagnostics[0]["range"]["start"]["character"], 2);
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("`continue` is not part of Tua"));
}

#[test]
fn did_open_accepts_native_array_annotations() {
    let uri = "file:///tmp/native-arrays.tua";
    let mut server = TestServer::new();

    server.send(did_open(
        uri,
        "local values: number[] = { 1, 2 }\nreturn values[1]\n",
    ));

    let diagnostics = last_published_diagnostics(server.output(), uri);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn did_open_rejects_invalid_native_array_indexed_assignments() {
    let uri = "file:///tmp/native-array-assignment.tua";
    let mut server = TestServer::new();

    server.send(did_open(
        uri,
        "local actors: number[] = {}\n\nactors[1] = \"not a number\"\n",
    ));

    let diagnostics = last_published_diagnostics(server.output(), uri)
        .into_iter()
        .filter(|diagnostic| diagnostic["code"] == "TL1003")
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 2);
    assert_eq!(diagnostics[0]["range"]["start"]["character"], 12);
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("expected number, found string"));
}

#[test]
fn did_open_publishes_nested_type_mismatch_paths_and_leaf_ranges() {
    let uri = "file:///tmp/nested-type-mismatch.tua";
    let source = r#"type Animation = { row: number }
type Config = { animations: { [string]: Animation } }
local function configure(config: Config)
  -- The body is irrelevant to this nested type-mismatch fixture.
end

configure({
  animations = {
    green = { row = "two" },
  },
})
"#;
    let mut server = TestServer::new();

    server.send(did_open(uri, source));

    let diagnostics = last_published_diagnostics(server.output(), uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], "TL1003");
    assert_eq!(
        diagnostics[0]["message"],
        "type mismatch at config.animations.green.row: expected number, found string"
    );
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 8);
    assert_eq!(diagnostics[0]["range"]["start"]["character"], 20);
    assert_eq!(diagnostics[0]["range"]["end"]["line"], 8);
    assert_eq!(diagnostics[0]["range"]["end"]["character"], 25);
}

#[test]
fn did_open_publishes_uninitialized_local_arity_and_compound_diagnostics() {
    let uri = "file:///tmp/uninitialized-local.tua";
    let source =
        "local first, missing = 1\nprint(missing)\nlocal total\ntotal += 1\nprint(total)\n";
    let mut server = TestServer::new();

    server.send(did_open(uri, source));

    let diagnostics = last_published_diagnostics(server.output(), uri)
        .into_iter()
        .filter(|diagnostic| diagnostic["code"] == "TL4045")
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 2, "{diagnostics:#?}");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 1);
    assert_eq!(diagnostics[0]["range"]["start"]["character"], 6);
    assert_eq!(
        diagnostics[0]["message"],
        "local `missing` defined at line 1 may be read before it is initialized; initialize with `nil` to silence"
    );
    assert_eq!(diagnostics[1]["range"]["start"]["line"], 3);
    assert_eq!(diagnostics[1]["range"]["start"]["character"], 0);
    assert_eq!(
        diagnostics[1]["message"],
        "local `total` defined at line 3 may be read before it is initialized; initialize with `nil` to silence"
    );
}

#[test]
fn did_open_publishes_inferred_local_reassignment_diagnostics() {
    let uri = "file:///tmp/reassignment.tua";
    let output = run_messages(vec![did_open(
        uri,
        r#"local score = 10
score = "high"
"#,
    )]);
    let diagnostics = published_diagnostics(&output, uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL1003");
    assert!(diagnostics[0]["message"]
        .as_str()
        .is_some_and(|message| message.contains("expected number, found string")));
}

#[test]
fn did_open_publishes_conflicting_signal_event_schemas() {
    let uri = "file:///tmp/signal-schema-diagnostics.tua";
    let source = r#"local events = signal()
events:declare("player.hit", function(player: string, damage: number) end)
events:on("player.hit", function(player: string, damage: string) end)
"#;
    let output = run_messages(vec![did_open(uri, source)]);
    let diagnostics = published_diagnostics(&output, uri)
        .into_iter()
        .filter(|diagnostic| diagnostic["code"] == "TL5005")
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic["message"]
        .as_str()
        .is_some_and(|message| message.contains("player.hit"))));
}

#[test]
fn did_open_publishes_object_pool_unreleased_acquire_diagnostic() {
    let uri = "file:///tmp/object-pool-lifecycle.tua";
    let source = r#"local bullets = objectPool({
  create = function()
    return {}
  end,
})

local leaked = bullets:acquire()
"#;
    let mut server = TestServer::new();

    server.send(did_open(uri, source));

    let diagnostics = last_published_diagnostics(server.output(), uri)
        .into_iter()
        .filter(|diagnostic| diagnostic["code"] == "TL5007")
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 2);
    assert!(diagnostics[0]["message"]
        .as_str()
        .is_some_and(|message| message.contains("leaked")));
}

#[test]
fn did_open_publishes_love_callback_diagnostics() {
    let uri = "file:///tmp/love-callback-diagnostics.tua";
    let source = r#"function love.udpate(dt: number)
end

function love.draw(unexpected: string)
end
"#;
    let output = run_messages(vec![did_open(uri, source)]);
    let diagnostics = published_diagnostics(&output, uri)
        .into_iter()
        .filter(|diagnostic| diagnostic["code"] == "TL5006")
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0]["severity"], 2);
    assert!(diagnostics[0]["message"]
        .as_str()
        .is_some_and(|message| message.contains("did you mean `love.update`")));
    assert_eq!(diagnostics[1]["severity"], 1);
    assert!(diagnostics[1]["message"]
        .as_str()
        .is_some_and(|message| message.contains("receives at most 0 parameters")));
}

#[test]
fn did_open_publishes_love_method_argument_diagnostics() {
    let temp = TempProject::new("love-method-argument-diagnostics");
    let source = r#"
local player: Image = love.graphics.newImage("assets/player.png")

player:setFilter(123)
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/assets.tua", source);
    temp.write("assets/player.png", "");

    let uri = temp.uri_for("src/assets.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
    ]);
    let diagnostics = last_published_diagnostics(&output, &uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL1003");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("type mismatch at min: expected FilterMode, found number"));
}
#[test]
fn did_open_publishes_love_enum_string_argument_diagnostics() {
    let uri = "file:///tmp/draw.tua";
    let source = r#"
function love.update(dt)
  love.graphics.circle("fillter", 100, 100, 2)
end
"#;
    let output = run_messages(vec![did_open(uri, source)]);
    let diagnostics = published_diagnostics(&output, uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL1003");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains(r#"type mismatch at mode: expected DrawMode, found "fillter""#));
}
#[test]
fn did_open_publishes_love_conf_field_assignment_diagnostics() {
    let uri = "file:///tmp/conf.tua";
    let source = r#"
function love.conf(t)
  t.window.title = "Tua LOVE minimal"
  t.window.width = 320
  t.window.msaa = "12"
end
"#;
    let output = run_messages(vec![did_open(uri, source)]);
    let diagnostics = published_diagnostics(&output, uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL1003");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("type mismatch at t.window.msaa: expected number | nil, found string"));
}
#[test]
fn did_open_publishes_love_asset_path_diagnostics() {
    let temp = TempProject::new("missing-asset-diagnostics");
    let source = r#"
local player = image "assets/missing.png"
return player
"#;

    temp.write("tua.toml", "source = \"src\"\nout = \"game\"\n");
    temp.write("src/assets.tua", source);

    let uri = temp.uri_for("src/assets.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
    ]);
    let diagnostics = last_published_diagnostics(&output, &uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL2004");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("image asset not found: assets/missing.png"));
}

#[test]
fn did_open_publishes_asset_path_casing_diagnostics() {
    let temp = TempProject::new("asset-case-diagnostics");
    let source = r#"local player = image "assets/player.png"
return player
"#;

    temp.write("tua.toml", "source = \"src\"\nout = \"game\"\n");
    temp.write("src/assets.tua", source);
    temp.write("src/assets/Player.png", "");

    let uri = temp.uri_for("src/assets.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
    ]);
    let diagnostics = last_published_diagnostics(&output, &uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 2);
    assert_eq!(diagnostics[0]["code"], "TL2008");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("assets/Player.png"));
}

#[test]
fn did_open_publishes_impossible_comparison_warnings() {
    let uri = "file:///tmp/comparison.tua";
    let source = r#"local function recursive(a: string)
  if a == 1 then
    return 1
  end
  return nil
end
recursive("value")
"#;
    let output = run_messages(vec![did_open(uri, source)]);
    let diagnostics = last_published_diagnostics(&output, uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 2);
    assert_eq!(diagnostics[0]["code"], "TL1005");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("comparison is always false: string and number do not overlap"));
}
#[test]
fn did_open_uses_project_diagnostics_config() {
    let temp = TempProject::new("diagnostics-config");

    temp.write(
        "tua.toml",
        r#"
source = "src"

[diagnostics]
duplicate_keys = true
undefined_variables = true
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(
            &uri,
            "local config = { hp = 10, hp = 20 }\nmissing()\nreturn config\n",
        ),
    ]);
    let diagnostics = last_published_diagnostics(&output, &uri);

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["severity"] == 2
            && diagnostic["code"] == "TL4001"
            && diagnostic["message"]
                .as_str()
                .expect("diagnostic message should be a string")
                == "table key `hp` is a duplicate; previously defined at column 18"
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["severity"] == 1
            && diagnostic["code"] == "TL4004"
            && diagnostic["message"]
                .as_str()
                .expect("diagnostic message should be a string")
                .contains("undefined variable `missing`")
    }));
}

#[test]
fn did_open_standard_rules_report_and_preserve_undefined_callback_variables() {
    let temp = TempProject::new("standard-rules-undefined-callback-variable");

    temp.write(
        "tua.toml",
        r#"
source = "src"

[diagnostics]
rules = true
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let source = r#"local controller = input({ controls = {} })

function love.update(dt)
  controller:update()
  local moveX, moveY = controls:get("move")
end
"#;
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
    ]);
    let publications = diagnostics_publications(&output, &uri);
    assert!(!publications.is_empty());
    for publication in [publications[0], publications[publications.len() - 1]] {
        let diagnostics = publication["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics should be an array");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic["severity"] == 1
                && diagnostic["code"] == "TL4004"
                && diagnostic["message"] == "undefined variable `controls`"
        }));
    }
}

#[test]
fn did_change_standard_rules_report_and_preserve_undefined_callback_variables() {
    let temp = TempProject::new("standard-rules-edited-undefined-callback-variable");

    temp.write(
        "tua.toml",
        r#"
source = "src"

[diagnostics]
rules = true
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let original = r#"local controller = input({ controls = {} })

function love.update(dt)
  controller:update()
  local moveX, moveY = controller:get("move")
end
"#;
    let edited = original.replace("controller:get", "controls:get");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, original),
        did_change(&uri, &edited, 2),
    ]);
    let publications = diagnostics_publications(&output, &uri);

    assert!(publications.len() >= 3);
    for publication in &publications[publications.len() - 2..] {
        let diagnostics = publication["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics should be an array");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic["severity"] == 1
                && diagnostic["code"] == "TL4004"
                && diagnostic["message"] == "undefined variable `controls`"
        }));
    }
}

#[test]
fn did_change_republishes_empty_diagnostics_for_clean_document() {
    let uri = "file:///tmp/player.tua";
    let output = run_messages(vec![
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "tua",
                    "version": 1,
                    "text": "local hp: number = \"full\"\nprint(hp)\n"
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
                        "text": "local hp: number = 10\nprint(hp)\n"
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
fn text_document_diagnostic_returns_same_codes_as_push_diagnostics() {
    let temp = TempProject::new("pull-diagnostics-parity");
    temp.write("tua.toml", "source = \"src\"\n");
    let uri = temp.uri_for("src/main.tua");
    let source = "local hp: number = \"full\"\n";
    let push_output = run_messages(vec![did_open(&uri, source)]);
    let push_codes = diagnostic_codes(&last_published_diagnostics(&push_output, &uri));
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(1, temp.uri(), true));
    server.send(did_open(&uri, source));

    assert!(
        diagnostics_publications(server.output(), &uri).is_empty(),
        "pull-capable clients should not receive push diagnostics"
    );

    server.send(text_document_diagnostic_request(2, &uri));

    assert_eq!(
        diagnostic_codes(&document_diagnostic_items(server.output(), 2)),
        push_codes
    );
}
#[test]
fn pull_diagnostics_use_fast_current_file_results_during_edit_window() {
    let temp = TempProject::new("pull-fast-edit-diagnostics");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "local ready = true\nprint(ready)\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(1, temp.uri(), true));
    server.send(did_open(&uri, "local ready = true\nprint(ready)\n"));
    server.send(did_change(
        &uri,
        "local sprite = image \"missing.png\"\nreturn sprite\n",
        2,
    ));
    server.send(text_document_diagnostic_request(2, &uri));

    assert!(
        diagnostics_publications(server.output(), &uri).is_empty(),
        "pull-capable clients should not receive push diagnostics"
    );
    assert!(
        document_diagnostic_items(server.output(), 2).is_empty(),
        "active typing should use fast diagnostics and skip slow asset validation"
    );

    let due = server
        .next_slow_diagnostics_due()
        .expect("pull diagnostics should queue an idle refresh");
    server.drain_due_diagnostics(due - Duration::from_millis(1));
    assert_eq!(diagnostic_refresh_requests(server.output()).len(), 0);

    server.drain_due_diagnostics(due);
    assert_eq!(diagnostic_refresh_requests(server.output()).len(), 1);

    server.send(text_document_diagnostic_request(3, &uri));

    assert_eq!(
        diagnostic_codes(&document_diagnostic_items(server.output(), 3)),
        vec!["TL2004"]
    );
}
#[test]
fn did_change_applies_incremental_edits_and_republishes_diagnostics() {
    let uri = "file:///tmp/incremental-change.tua";
    let source = "local hp: number = 10\nprint(hp)\n";
    let output = run_messages(vec![
        did_open(uri, source),
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
                        "range": {
                            "start": {
                                "line": 0,
                                "character": "local hp: number = ".len()
                            },
                            "end": {
                                "line": 0,
                                "character": "local hp: number = 10".len()
                            }
                        },
                        "rangeLength": 2,
                        "text": "\"full\""
                    }
                ]
            }
        }),
    ]);
    let publications = diagnostics_publications(&output, uri);
    let change_diagnostics = publications[1]["params"]["diagnostics"]
        .as_array()
        .expect("change diagnostics should be an array");

    assert_eq!(publications.len(), 2);
    assert_eq!(change_diagnostics.len(), 1);
    assert_eq!(change_diagnostics[0]["code"], "TL1003");
}
#[test]
fn did_change_publishes_fast_diagnostics_before_slow_diagnostics() {
    let temp = TempProject::new("fast-slow-diagnostics");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "local ready = true\nprint(ready)\n");

    let uri = temp.uri_for("src/main.tua");
    let edited = "local sprite = image \"missing.png\"\nreturn sprite\n";
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, "local ready = true\nprint(ready)\n"),
        did_change(&uri, edited, 2),
    ]);
    let publications = diagnostics_publications(&output, &uri);

    assert_eq!(publications.len(), 3);
    assert_eq!(
        publications[1]["params"]["diagnostics"]
            .as_array()
            .expect("fast diagnostics should be an array")
            .len(),
        0
    );
    assert_eq!(
        publications[2]["params"]["diagnostics"][0]["code"],
        "TL2004"
    );
}

#[test]
fn sequential_project_opens_do_not_flash_source_only_type_mismatches() {
    let temp = TempProject::new("sequential-project-open-diagnostics");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/types.tua", "export type Player = { hp: number }\n");

    let main_uri = temp.uri_for("src/main.tua");
    let types_uri = temp.uri_for("src/types.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(
            &main_uri,
            "import type { Player } from \"./types\"\nlocal player: Player = { hp = \"high\" }\nreturn player\n",
        ),
        did_open(&types_uri, "export type Player = { hp: string }\n"),
    ]);

    for publication in diagnostics_publications(&output, &main_uri) {
        let diagnostics = publication["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics should be an array");
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic["code"] != "TL1003"),
            "source-only project facts must not flash a mismatch: {publication:#?}"
        );
    }
}
#[test]
fn did_change_debounces_slow_diagnostics_until_idle_window() {
    let temp = TempProject::new("debounced-slow-diagnostics");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "local ready = true\nprint(ready)\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, "local ready = true\nprint(ready)\n"));
    server.send_without_drain(did_change(
        &uri,
        "local sprite = image \"missing.png\"\nreturn sprite\n",
        2,
    ));

    let publications = diagnostics_publications(server.output(), &uri);
    assert_eq!(publications.len(), 2);
    assert_eq!(
        publications[1]["params"]["diagnostics"]
            .as_array()
            .expect("fast diagnostics should be an array")
            .len(),
        0
    );

    let due = server
        .next_slow_diagnostics_due()
        .expect("slow diagnostics should be queued");
    server.drain_due_diagnostics(due - Duration::from_millis(1));
    assert_eq!(diagnostics_publications(server.output(), &uri).len(), 2);

    server.drain_due_diagnostics(due);
    let publications = diagnostics_publications(server.output(), &uri);
    assert_eq!(publications.len(), 3);
    assert_eq!(
        publications[2]["params"]["diagnostics"][0]["code"],
        "TL2004"
    );
}
#[test]
fn did_change_cancels_stale_slow_diagnostics_for_newer_versions() {
    let temp = TempProject::new("cancel-stale-slow-diagnostics");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "local ready = true\nprint(ready)\n");

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, "local ready = true\nprint(ready)\n"),
        did_change(
            &uri,
            "local sprite = image \"missing.png\"\nreturn sprite\n",
            2,
        ),
        did_change(&uri, "local ready = true\nprint(ready)\n", 3),
    ]);
    let publications = diagnostics_publications(&output, &uri);
    let published_codes = publications
        .iter()
        .flat_map(|publication| {
            publication["params"]["diagnostics"]
                .as_array()
                .into_iter()
                .flat_map(|diagnostics| diagnostics.iter())
        })
        .filter_map(|diagnostic| diagnostic["code"].as_str())
        .collect::<Vec<_>>();

    assert_eq!(publications.len(), 3);
    assert!(
        !published_codes.contains(&"TL2004"),
        "stale missing-asset diagnostic should not publish: {published_codes:?}"
    );
}
#[test]
fn did_save_accepts_text_and_republishes_diagnostics() {
    let uri = "file:///tmp/save.tua";
    let output = run_messages(vec![
        did_open(uri, "local hp: number = 10\nprint(hp)\n"),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": {
                "textDocument": {
                    "uri": uri
                },
                "text": "local hp: number = \"full\"\nprint(hp)\n"
            }
        }),
    ]);
    let publications = diagnostics_publications(&output, uri);
    let save_diagnostics = publications[1]["params"]["diagnostics"]
        .as_array()
        .expect("save diagnostics should be an array");

    assert_eq!(publications.len(), 2);
    assert_eq!(save_diagnostics.len(), 1);
    assert_eq!(save_diagnostics[0]["code"], "TL1003");
}
#[test]
fn did_close_removes_document_and_clears_diagnostics() {
    let uri = "file:///tmp/close.tua";
    let output = run_messages(vec![
        did_open(uri, "local hp: number = \"full\"\n"),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": {
                "textDocument": {
                    "uri": uri
                }
            }
        }),
        hover_request(2, uri, "local hp: number = \"full\"\n", "hp"),
    ]);
    let publications = diagnostics_publications(&output, uri);

    assert_eq!(publications.len(), 2);
    assert_eq!(
        publications[1]["params"]["diagnostics"]
            .as_array()
            .expect("close diagnostics should be an array")
            .len(),
        0
    );
    assert!(response_with_id(&output, 2)["result"].is_null());
}

#[test]
fn reserved_function_names_publish_specific_keyword_diagnostics() {
    let uri = "file:///tmp/reserved-function-name.tua";
    let mut server = TestServer::new();
    server.send(did_open(uri, "local function end() return 1 end\n"));
    let diagnostics = last_published_diagnostics(server.output(), uri);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic["message"] == "reserved keyword `end` cannot be used as a function name"
        })
        .expect("reserved keyword diagnostic should be published");
    assert_eq!(diagnostic["code"], "TL0001");
    assert_eq!(diagnostic["severity"], 1);
    assert_eq!(
        diagnostic["range"]["start"],
        json!({ "line": 0, "character": 15 })
    );
    assert_eq!(
        diagnostic["range"]["end"],
        json!({ "line": 0, "character": 18 })
    );
}

#[test]
fn multi_return_assignment_errors_keep_distinct_destination_ranges() {
    let uri = "file:///tmp/return-slots.tua";
    let source = "local function values(): (number, string, nil) return 1, 'bad', nil end\nlocal a, b, c = 0, 0, 0\na, b, c = values()\n";
    let output = run_messages(vec![did_open(uri, source)]);
    let diagnostics = published_diagnostics(&output, uri);
    let errors = diagnostics
        .iter()
        .filter(|d| d["code"] == "TL1003")
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 2, "{diagnostics:#?}");
    for (error, name, slot, column) in [(errors[0], "b", 2, 3), (errors[1], "c", 3, 6)] {
        assert!(error["message"]
            .as_str()
            .unwrap()
            .contains(&format!("type mismatch at {name}:")));
        assert!(error["message"]
            .as_str()
            .unwrap()
            .contains(&format!("return value {slot} of `values`")));
        assert_eq!(
            error["range"],
            json!({"start": {"line": 2, "character": column}, "end": {"line": 2, "character": column + 1}})
        );
    }
}

#[test]
fn function_contract_diagnostics_publish_context_and_actionable_ranges() {
    let uri = "file:///tmp/contracts.tua";
    let source = "local function dimensions(): (number, number)\n  return 12, 'tall'\nend\nlocal function move(speed: number = 'fast') end\nlocal function draw(width: number, height: number) end\ndraw(12)\n";
    let output = run_messages(vec![did_open(uri, source)]);
    let diagnostics = published_diagnostics(&output, uri);
    for (fragment, line, start, end) in [
        ("return value 2 of `dimensions`", 1, 13, 19),
        ("default for parameter `speed`", 3, 20, 25),
        ("missing required parameters: `height`", 5, 0, 8),
    ] {
        let error = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains(fragment))
            })
            .unwrap_or_else(|| panic!("missing {fragment}: {diagnostics:#?}"));
        assert_eq!(
            error["range"],
            json!({"start": {"line": line, "character": start}, "end": {"line": line, "character": end}})
        );
    }
}
