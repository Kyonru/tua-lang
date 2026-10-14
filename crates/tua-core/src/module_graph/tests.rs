use super::*;
use crate::{infer_module_exports, parse_lua_source, parse_tua_source, Type};

fn tua_input(path: &str, source: &str) -> (ParsedSource, HirModule, ModuleExports) {
    let parsed = parse_tua_source(path, source).expect("Tua source should parse");
    let hir = lower_parsed_source(&parsed);
    let exports = infer_module_exports(&parsed).expect("exports should be inferred");

    (parsed, hir, exports)
}

fn lua_input(path: &str, source: &str) -> (ParsedSource, HirModule, ModuleExports) {
    let parsed = parse_lua_source(path, source).expect("Lua source should parse");
    let hir = lower_parsed_source(&parsed);
    let exports = infer_module_exports(&parsed).expect("exports should be inferred");

    (parsed, hir, exports)
}

fn graph<'a>(
    inputs: impl IntoIterator<
        Item = (
            &'a ParsedSource,
            &'a HirModule,
            &'a ModuleExports,
            AnalysisSourceKind,
        ),
    >,
) -> ModuleGraph {
    graph_with_options(
        inputs,
        ModuleGraphOptions {
            source_roots: vec![PathBuf::from("src")],
            output_roots: Vec::new(),
            extension: "tua".to_owned(),
            require_path_separator: crate::vfs::ModulePathSeparator::Dot,
            declaration_modules: BTreeMap::new(),
        },
    )
}

fn graph_with_options<'a>(
    inputs: impl IntoIterator<
        Item = (
            &'a ParsedSource,
            &'a HirModule,
            &'a ModuleExports,
            AnalysisSourceKind,
        ),
    >,
    options: ModuleGraphOptions,
) -> ModuleGraph {
    ModuleGraph::build(
        inputs
            .into_iter()
            .map(|(parsed, hir, exports, source_kind)| ModuleGraphInput {
                path: &parsed.path,
                parsed: Some(parsed),
                hir: Some(hir),
                exports: Some(exports),
                source_kind,
            }),
        options,
    )
}

#[test]
fn indexes_require_edges_and_module_exports() {
    let main = parse_tua_source(
        "src/main.tua",
        r#"
local Player = require("player")
return { ok = true }
"#,
    )
    .expect("main should parse");
    let main_hir = lower_parsed_source(&main);
    let main_exports = infer_module_exports(&main).expect("main exports should infer");
    let (player, player_hir, player_exports) = tua_input(
        "src/player.tua",
        r#"
local function new()
  return { hp = 100 }
end

return {
  new = new,
}
"#,
    );
    let graph = graph([
        (&main, &main_hir, &main_exports, AnalysisSourceKind::Tua),
        (
            &player,
            &player_hir,
            &player_exports,
            AnalysisSourceKind::Tua,
        ),
    ]);
    let module = graph
        .module("src/main.tua")
        .expect("main module should be indexed");

    assert_eq!(module.require_edges.len(), 1);
    assert_eq!(
        module.require_edges[0].resolution,
        RequireResolution::Resolved {
            path: PathBuf::from("src/player.tua")
        }
    );

    let export = graph
        .export("src/player.tua", "new")
        .expect("public export should be indexed");

    assert_eq!(export.public_name, "new");
    assert_eq!(export.origin, ModuleExportOrigin::Tua);
    assert!(matches!(export.field.ty, Type::Function(_)));
    assert!(export.symbol_id.is_some());
}

#[test]
fn declaration_modules_overlay_require_without_changing_import_resolution() {
    let (main, main_hir, main_exports) = tua_input(
        "src/main.tua",
        r#"
local required = require("lib.feel")
local imported = import("lib.feel")
return { required = required, imported = imported }
"#,
    );
    let (runtime, runtime_hir, runtime_exports) =
        lua_input("src/lib/feel.lua", "return { runtime = true }\n");
    let (declaration, declaration_hir, declaration_exports) = lua_input(
        "lib/feel/types/feel.lua",
        "---@meta feel\nreturn { declared = true }\n",
    );
    let declaration_path = PathBuf::from("lib/feel/types/feel.lua");
    let graph = graph_with_options(
        [
            (&main, &main_hir, &main_exports, AnalysisSourceKind::Tua),
            (
                &runtime,
                &runtime_hir,
                &runtime_exports,
                AnalysisSourceKind::Lua,
            ),
            (
                &declaration,
                &declaration_hir,
                &declaration_exports,
                AnalysisSourceKind::Lua,
            ),
        ],
        ModuleGraphOptions {
            source_roots: vec![PathBuf::from("src")],
            output_roots: Vec::new(),
            extension: "tua".to_owned(),
            require_path_separator: crate::vfs::ModulePathSeparator::Dot,
            declaration_modules: BTreeMap::from([(
                "lib.feel".to_owned(),
                declaration_path.clone(),
            )]),
        },
    );
    let module = graph
        .module("src/main.tua")
        .expect("main module should be indexed");

    assert_eq!(
        module.require_edges[0].resolution,
        RequireResolution::Resolved {
            path: declaration_path,
        }
    );
    assert_eq!(
        module.require_edges[1].resolution,
        RequireResolution::Resolved {
            path: PathBuf::from("src/lib/feel.lua"),
        }
    );
}

#[test]
fn reverse_dependency_closure_includes_transitive_importers() {
    let (model, model_hir, model_exports) = tua_input("src/model.tua", "return { hp = 100 }\n");
    let (player, player_hir, player_exports) = tua_input(
        "src/player.tua",
        "local Model = require(\"model\")\nreturn { hp = Model.hp }\n",
    );
    let (main, main_hir, main_exports) = tua_input(
        "src/main.tua",
        "local Player = require(\"player\")\nreturn { player = Player }\n",
    );
    let graph = graph([
        (&model, &model_hir, &model_exports, AnalysisSourceKind::Tua),
        (
            &player,
            &player_hir,
            &player_exports,
            AnalysisSourceKind::Tua,
        ),
        (&main, &main_hir, &main_exports, AnalysisSourceKind::Tua),
    ]);

    assert_eq!(
        graph.reverse_dependency_closure(["src/model.tua"]),
        BTreeSet::from([
            PathBuf::from("src/main.tua"),
            PathBuf::from("src/model.tua"),
            PathBuf::from("src/player.tua"),
        ])
    );
}

#[test]
fn resolves_static_import_edges_like_require_edges() {
    let (main, main_hir, main_exports) = tua_input(
        "src/systems/main.tua",
        r#"
local Enemy = import("./enemy")
local Player = import("player")
local Legacy = import("legacy")
local Shared = import("shared")
return { ok = true }
"#,
    );
    let (enemy, enemy_hir, enemy_exports) =
        tua_input("src/systems/enemy.tua", "return { hp = 50 }\n");
    let (player, player_hir, player_exports) = tua_input("src/player.tua", "return { hp = 100 }\n");
    let (shared, shared_hir, shared_exports) =
        tua_input("src/shared/init.tua", "return { enabled = true }\n");
    let (legacy, legacy_hir, legacy_exports) = lua_input(
        "src/legacy.lua",
        "return { clamp = function(value) return value end }\n",
    );
    let graph = graph([
        (&main, &main_hir, &main_exports, AnalysisSourceKind::Tua),
        (&enemy, &enemy_hir, &enemy_exports, AnalysisSourceKind::Tua),
        (
            &player,
            &player_hir,
            &player_exports,
            AnalysisSourceKind::Tua,
        ),
        (
            &legacy,
            &legacy_hir,
            &legacy_exports,
            AnalysisSourceKind::Lua,
        ),
        (
            &shared,
            &shared_hir,
            &shared_exports,
            AnalysisSourceKind::Tua,
        ),
    ]);
    let module = graph
        .module("src/systems/main.tua")
        .expect("main module should be indexed");

    assert_eq!(module.require_edges.len(), 4);
    assert_eq!(module.require_edges[0].kind, ModuleSpecifierKind::Import);
    assert_eq!(
        module.require_edges[0].resolution,
        RequireResolution::Resolved {
            path: PathBuf::from("src/systems/enemy.tua")
        }
    );
    assert_eq!(
        module.require_edges[1].resolution,
        RequireResolution::Resolved {
            path: PathBuf::from("src/player.tua")
        }
    );
    assert_eq!(
        module.require_edges[2].resolution,
        RequireResolution::Resolved {
            path: PathBuf::from("src/legacy.lua")
        }
    );
    assert_eq!(
        module.require_edges[3].resolution,
        RequireResolution::Resolved {
            path: PathBuf::from("src/shared/init.tua")
        }
    );
}

#[test]
fn reports_missing_require_with_module_name_and_search_roots() {
    let main = parse_tua_source(
        "src/main.tua",
        r#"
local Missing = require("missing.player")
return { ok = true }
"#,
    )
    .expect("main should parse");
    let main_hir = lower_parsed_source(&main);
    let main_exports = infer_module_exports(&main).expect("main exports should infer");
    let graph = graph([(&main, &main_hir, &main_exports, AnalysisSourceKind::Tua)]);
    let diagnostics = graph.diagnostics("src/main.tua");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LINT_MISSING_REQUIRE)
    );
    assert!(diagnostics[0]
        .message
        .contains("require file not found: `missing.player`"));
    assert!(diagnostics[0].message.contains("searched: src"));
}

#[test]
fn reports_missing_child_imports() {
    let main = parse_tua_source(
        "src/main.tua",
        r#"
local Missing = import("./missing")
return { ok = true }
"#,
    )
    .expect("main should parse");
    let main_hir = lower_parsed_source(&main);
    let main_exports = infer_module_exports(&main).expect("main exports should infer");
    let graph = graph([(&main, &main_hir, &main_exports, AnalysisSourceKind::Tua)]);
    let diagnostics = graph.diagnostics("src/main.tua");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LINT_MISSING_REQUIRE)
    );
    assert!(diagnostics[0]
        .message
        .contains("import file not found: `./missing`"));
}

#[test]
fn reports_invalid_import_specifiers() {
    let main = parse_tua_source(
        "src/main.tua",
        r#"
local Player = import("../player")
return { ok = true }
"#,
    )
    .expect("main should parse");
    let main_hir = lower_parsed_source(&main);
    let main_exports = infer_module_exports(&main).expect("main exports should infer");
    let graph = graph([(&main, &main_hir, &main_exports, AnalysisSourceKind::Tua)]);
    let diagnostics = graph.diagnostics("src/main.tua");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::LINT_BAD_REQUIRE));
    assert!(diagnostics[0]
        .message
        .contains("invalid import specifier `../player`"));
    assert!(diagnostics[0]
        .message
        .contains("cannot traverse parent directories"));
}

#[test]
fn reports_duplicate_candidate_modules_deterministically() {
    let main = parse_tua_source(
        "src/main.tua",
        r#"
local Player = require("player")
return { ok = true }
"#,
    )
    .expect("main should parse");
    let main_hir = lower_parsed_source(&main);
    let main_exports = infer_module_exports(&main).expect("main exports should infer");
    let (player_file, player_file_hir, player_file_exports) =
        tua_input("src/player.tua", "return { hp = 100 }\n");
    let (player_init, player_init_hir, player_init_exports) =
        tua_input("src/player/init.tua", "return { hp = 200 }\n");
    let graph = graph([
        (&main, &main_hir, &main_exports, AnalysisSourceKind::Tua),
        (
            &player_file,
            &player_file_hir,
            &player_file_exports,
            AnalysisSourceKind::Tua,
        ),
        (
            &player_init,
            &player_init_hir,
            &player_init_exports,
            AnalysisSourceKind::Tua,
        ),
    ]);
    let edge = &graph
        .module("src/main.tua")
        .expect("main should be indexed")
        .require_edges[0];

    assert_eq!(
        edge.resolution,
        RequireResolution::Duplicate {
            candidates: vec![
                PathBuf::from("src/player.tua"),
                PathBuf::from("src/player/init.tua"),
            ],
            selected: PathBuf::from("src/player.tua"),
        }
    );
    assert_eq!(
        graph.diagnostics("src/main.tua")[0].code,
        Some(DiagnosticCode::LINT_DUPLICATE_REQUIRE_CANDIDATE)
    );
}

#[test]
fn ignores_output_candidate_when_source_module_exists() {
    let (main, main_hir, main_exports) = tua_input(
        "src/main.tua",
        r#"
local Player = require("player")
local Background = require("background")
local Legacy = require("legacy")
return { player = Player, background = Background, legacy = Legacy }
"#,
    );
    let (player_source, player_source_hir, player_source_exports) =
        tua_input("src/player.tua", "return { hp = 100 }\n");
    let (player_output, player_output_hir, player_output_exports) =
        lua_input("game/player.lua", "return { hp = 100 }\n");
    let (background, background_hir, background_exports) =
        lua_input("game/background.lua", "return { draw = function() end }\n");
    let (legacy_source, legacy_source_hir, legacy_source_exports) = lua_input(
        "src/legacy.lua",
        "return { clamp = function(value) return value end }\n",
    );
    let (legacy_output, legacy_output_hir, legacy_output_exports) = lua_input(
        "game/legacy.lua",
        "return { clamp = function(value) return value end }\n",
    );
    let graph = graph_with_options(
        [
            (&main, &main_hir, &main_exports, AnalysisSourceKind::Tua),
            (
                &player_source,
                &player_source_hir,
                &player_source_exports,
                AnalysisSourceKind::Tua,
            ),
            (
                &player_output,
                &player_output_hir,
                &player_output_exports,
                AnalysisSourceKind::Lua,
            ),
            (
                &background,
                &background_hir,
                &background_exports,
                AnalysisSourceKind::Lua,
            ),
            (
                &legacy_source,
                &legacy_source_hir,
                &legacy_source_exports,
                AnalysisSourceKind::Lua,
            ),
            (
                &legacy_output,
                &legacy_output_hir,
                &legacy_output_exports,
                AnalysisSourceKind::Lua,
            ),
        ],
        ModuleGraphOptions {
            source_roots: vec![PathBuf::from("src")],
            output_roots: vec![PathBuf::from("game")],
            extension: "tua".to_owned(),
            require_path_separator: crate::vfs::ModulePathSeparator::Dot,
            declaration_modules: BTreeMap::new(),
        },
    );
    let module = graph
        .module("src/main.tua")
        .expect("main module should be indexed");

    assert_eq!(
        module.require_edges[0].resolution,
        RequireResolution::Resolved {
            path: PathBuf::from("src/player.tua"),
        }
    );
    assert_eq!(
        module.require_edges[1].resolution,
        RequireResolution::Resolved {
            path: PathBuf::from("game/background.lua"),
        }
    );
    assert_eq!(
        module.require_edges[2].resolution,
        RequireResolution::Resolved {
            path: PathBuf::from("src/legacy.lua"),
        }
    );
    assert!(graph.diagnostics("src/main.tua").is_empty());
}

#[test]
fn import_cycles_report_diagnostics() {
    let (a, a_hir, a_exports) = tua_input(
        "src/a.tua",
        r#"
local B = import("./b")
return { name = "a" }
"#,
    );
    let (b, b_hir, b_exports) = tua_input(
        "src/b.tua",
        r#"
local A = import("./a")
return { name = "b" }
"#,
    );
    let graph = graph([
        (&a, &a_hir, &a_exports, AnalysisSourceKind::Tua),
        (&b, &b_hir, &b_exports, AnalysisSourceKind::Tua),
    ]);

    assert!(graph
        .all_diagnostics()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_REQUIRE_CYCLE)));
    assert_eq!(
        graph.cycles(),
        &[vec![PathBuf::from("src/a.tua"), PathBuf::from("src/b.tua")]]
    );
}

#[test]
fn require_cycles_report_diagnostics_and_keep_partial_results() {
    let (a, a_hir, a_exports) = tua_input(
        "src/a.tua",
        r#"
local B = require("b")
return { name = "a" }
"#,
    );
    let (b, b_hir, b_exports) = tua_input(
        "src/b.tua",
        r#"
local A = require("a")
return { name = "b" }
"#,
    );
    let graph = graph([
        (&a, &a_hir, &a_exports, AnalysisSourceKind::Tua),
        (&b, &b_hir, &b_exports, AnalysisSourceKind::Tua),
    ]);

    assert!(graph.export("src/a.tua", "name").is_some());
    assert!(graph.export("src/b.tua", "name").is_some());
    assert!(graph
        .all_diagnostics()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_REQUIRE_CYCLE)));
}

#[test]
fn deeply_nested_require_graphs_do_not_use_the_call_stack() {
    const MODULE_COUNT: usize = 4_096;

    let paths = (0..MODULE_COUNT)
        .map(|index| PathBuf::from(format!("src/module_{index:04}.tua")))
        .collect::<Vec<_>>();
    let span = SourceSpan {
        start: crate::SourcePosition { line: 1, column: 1 },
        end: crate::SourcePosition { line: 1, column: 2 },
    };
    let mut graph = ModuleGraph::default();

    for (index, path) in paths.iter().enumerate() {
        let dependency = paths[(index + 1) % MODULE_COUNT].clone();
        graph.modules.insert(
            path.clone(),
            ModuleGraphModule {
                path: path.clone(),
                source_kind: AnalysisSourceKind::Tua,
                require_edges: vec![RequireGraphEdge {
                    source: path.clone(),
                    module: format!("module_{:04}", (index + 1) % MODULE_COUNT),
                    kind: ModuleSpecifierKind::Require,
                    type_only: false,
                    span,
                    resolution: RequireResolution::Resolved { path: dependency },
                }],
                exports: Vec::new(),
            },
        );
    }

    graph.collect_cycle_diagnostics();

    assert_eq!(graph.cycles.len(), 1);
    assert_eq!(graph.cycles[0].len(), MODULE_COUNT);
    assert_eq!(graph.cycles[0].first(), paths.first());
    assert_eq!(graph.cycles[0].last(), paths.last());
    let diagnostic = graph
        .diagnostics(&paths[0])
        .first()
        .expect("cycle should be reported at its deterministic entry module");
    assert_eq!(diagnostic.code, Some(DiagnosticCode::LINT_REQUIRE_CYCLE));
}

#[test]
fn tracks_lua_doc_export_origins() {
    let (audio, audio_hir, audio_exports) = lua_input(
        "src/audio.lua",
        r#"---@param source string audio source
---@return boolean success flag
local function play(source)
  return true
end

return {
  play = play,
}
"#,
    );
    let graph = graph([(&audio, &audio_hir, &audio_exports, AnalysisSourceKind::Lua)]);
    let export = graph
        .export("src/audio.lua", "play")
        .expect("LuaDoc export should be indexed");

    assert_eq!(export.origin, ModuleExportOrigin::LuaDoc);
    assert_eq!(
        export.field.ty,
        Type::function([Type::string()], Type::boolean())
    );
    assert_eq!(
        graph.exports_for_symbol("src/audio.lua", export.symbol_id.unwrap()),
        std::slice::from_ref(export)
    );
}
