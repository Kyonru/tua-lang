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
        ModuleGraphOptions {
            source_roots: vec![PathBuf::from("src")],
            output_roots: Vec::new(),
            extension: "tua".to_owned(),
        },
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
