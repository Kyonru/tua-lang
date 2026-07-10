use super::*;
use crate::{
    DiagnosticCode, DiagnosticSeverity, Fixture, PluginContext, PluginDeclaration,
    PluginDeclarationKind, PluginRegistry, TuaPlugin,
};
use std::env;
use std::fs;
use std::path::Path;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "tua-analysis-cache-{name}-{}-{unique}",
            process::id()
        ));

        fs::create_dir_all(&path).expect("failed to create temp project");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, path: &str, source: impl AsRef<[u8]>) -> PathBuf {
        let path = self.path.join(path);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent directory");
        }

        fs::write(&path, source).expect("failed to write file");
        path
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug)]
struct AnalysisTestPlugin;

impl TuaPlugin for AnalysisTestPlugin {
    fn name(&self) -> &'static str {
        "tua.analysis-test"
    }

    fn declarations(&self) -> Vec<PluginDeclaration> {
        vec![PluginDeclaration::new(
            "testdecl",
            PluginDeclarationKind::TopLevelDeclaration,
        )]
    }

    fn diagnostics(&self, context: PluginContext<'_>) -> Vec<Diagnostic> {
        if context.source.contains("plugin_problem") {
            vec![Diagnostic::new(
                context.path,
                1,
                1,
                DiagnosticSeverity::Warning,
                "analysis test plugin diagnostic",
            )
            .with_code(DiagnosticCode::PLUGIN_DIAGNOSTIC)]
        } else {
            Vec::new()
        }
    }
}

#[test]
fn snapshot_prefers_open_document_over_file_contents() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, "local value: number = 1\n");
    host.open_document(&path, "local value: number = \"open\"\n");

    let snapshot = host.snapshot();

    assert!(snapshot.is_open_document(&path));
    assert_eq!(
        snapshot.source(&path),
        Some("local value: number = \"open\"\n")
    );
    assert_eq!(snapshot.diagnostics(&path).len(), 1);
}

#[test]
fn plugin_registry_is_inert_until_registered_and_flows_into_diagnostics() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, "plugin_problem()\n");

    let snapshot = host.snapshot();

    assert!(snapshot.plugin_registry().is_empty());
    assert!(!snapshot
        .diagnostics(&path)
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::PLUGIN_DIAGNOSTIC)));

    host.set_plugin_registry(PluginRegistry::new().with_plugin(AnalysisTestPlugin));

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics(&path);

    assert_eq!(
        snapshot.plugin_registry().declarations()[0].keyword,
        "testdecl"
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::PLUGIN_DIAGNOSTIC)
            && diagnostic.message == "analysis test plugin diagnostic"
    }));
}

#[test]
fn open_documents_drive_hover_and_closed_documents_fall_back_to_disk() {
    let path = PathBuf::from("src/main.tua");
    let disk = Fixture::parse(
        r#"type Player = { hp: number }
local player: <|hover|>Player = { hp = 10 }
"#,
    );
    let open = Fixture::parse(
        r#"type Enemy = { hp: number }
local enemy: <|hover|>Enemy = { hp = 5 }
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_file(&path, disk.source("src/main.tua"));
    host.open_document(&path, open.source("src/main.tua"));

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover(&path, open.position("hover"))
        .expect("open hover should resolve");

    assert!(hover.contents.contains("type Enemy"));

    host.close_document(&path);

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover(&path, disk.position("hover"))
        .expect("disk hover should resolve after close");

    assert!(hover.contents.contains("type Player"));
}

#[test]
fn snapshot_answers_parse_diagnostics_hover_completion_and_definition() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
type <|alias_def|>Player<|/alias_def|> = { hp: number }

local player: <|hover|>Player = { hp = 10 }
local broken: number = "nope"

player.hp = 11
-- file: src/complete.tua
type Player = { hp: number }
local typed: Pl<|complete|> = nil
"#,
    );
    let path = PathBuf::from("src/main.tua");
    let completion_path = PathBuf::from("src/complete.tua");
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let parsed = snapshot
        .parse(&path)
        .expect("file should be cached")
        .expect("source should parse");
    let diagnostics = snapshot.diagnostics(&path);
    let hover = snapshot
        .hover(&path, fixture.position("hover"))
        .expect("alias hover should resolve");
    let completions = snapshot.completion(&completion_path, fixture.position("complete"));
    let definition = snapshot
        .definition(&path, fixture.position("hover"))
        .expect("alias definition should resolve");

    assert_eq!(parsed.path, path);
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("type mismatch")));
    assert!(hover.contents.contains("type Player"));
    assert!(completions
        .iter()
        .any(|completion| completion.label == "Player"));
    assert_eq!(definition.span, fixture.range("alias_def"));
}

#[test]
fn snapshot_reuses_cached_project_artifacts_until_revision_changes() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, "local value: number = 1\n");

    let first_revision = host.revision();
    let _first = host.snapshot();
    let stats_after_first = host.cache_stats();
    let _second = host.snapshot();

    assert_eq!(host.revision(), first_revision);
    assert_eq!(host.cache_stats(), stats_after_first);

    host.open_document(&path, "local value: number = 2\n");
    assert!(host.revision() > first_revision);

    let _third = host.snapshot();
    assert!(host.cache_stats().invalidations > stats_after_first.invalidations);
}

#[test]
fn setting_identical_file_source_does_not_bump_revision() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, "local value: number = 1\n");
    host.snapshot();

    let revision = host.revision();
    let stats = host.cache_stats();

    host.set_file(&path, "local value: number = 1\n");
    host.snapshot();

    assert_eq!(host.revision(), revision);
    assert_eq!(host.cache_stats(), stats);
}

#[test]
fn snapshot_clones_share_warmed_definition_cache() {
    let fixture = Fixture::parse(
        r#"type <|alias_def|>Player<|/alias_def|> = { hp: number }
local player: <|alias_ref|>Player = { hp = 10 }
"#,
    );
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, fixture.source("src/main.tua"));

    let snapshot = host.snapshot();
    assert!(snapshot.definition_cache.borrow().is_empty());

    let definition = snapshot
        .definition(&path, fixture.position("alias_ref"))
        .expect("alias definition should resolve");
    assert_eq!(definition.span, fixture.range("alias_def"));
    let cache_len = snapshot.definition_cache.borrow().len();
    assert!(cache_len > 0);

    let warm_snapshot = host.snapshot();

    assert!(Rc::ptr_eq(
        &snapshot.definition_cache,
        &warm_snapshot.definition_cache
    ));
    assert_eq!(warm_snapshot.definition_cache.borrow().len(), cache_len);

    let repeated = warm_snapshot
        .definition(&path, fixture.position("alias_ref"))
        .expect("cached alias definition should resolve");

    assert_eq!(repeated.span, fixture.range("alias_def"));
    assert_eq!(warm_snapshot.definition_cache.borrow().len(), cache_len);
}

#[test]
fn snapshot_warms_semantic_facts_per_requested_file() {
    let main = PathBuf::from("src/main.tua");
    let other = PathBuf::from("src/other.tua");
    let mut host = AnalysisHost::new();

    host.set_file(
        &main,
        r#"type Player = { hp: number }
local player: Player = { hp = 10 }
local hp = player.hp
"#,
    );
    host.set_file(
        &other,
        r#"type Enemy = { hp: number }
local enemy: Enemy = { hp = 5 }
local hp = enemy.hp
"#,
    );

    let snapshot = host.snapshot();
    assert!(snapshot.semantic_facts.borrow().is_empty());

    let main_index = snapshot
        .semantic_index(&main)
        .expect("main semantic index should warm");
    assert!(main_index.cache_len() > 0);
    assert_eq!(snapshot.semantic_facts.borrow().len(), 1);
    assert!(snapshot
        .semantic_facts
        .borrow()
        .contains_key(&normalize_path(&main)));
    assert!(!snapshot
        .semantic_facts
        .borrow()
        .contains_key(&normalize_path(&other)));

    assert!(host.warm_file(&other, AnalysisWarmPriority::High));
    let warmed = host.snapshot();

    assert_eq!(warmed.semantic_facts.borrow().len(), 2);
    assert!(warmed
        .semantic_facts
        .borrow()
        .contains_key(&normalize_path(&other)));
}

#[test]
fn snapshot_keeps_project_inputs() {
    let mut host = AnalysisHost::with_config(ProjectConfig {
        source: PathBuf::from("game"),
        extension: "tua".to_owned(),
        ..ProjectConfig::default()
    });

    host.set_source_roots([PathBuf::from("game")]);
    host.set_builtin_catalogs(BuiltinCatalogs {
        lua: true,
        love: false,
    });

    let snapshot = host.snapshot();

    assert_eq!(snapshot.config().source, PathBuf::from("game"));
    assert_eq!(snapshot.source_roots(), &[PathBuf::from("game")]);
    assert!(!snapshot.builtin_catalogs().love);
}

#[test]
fn pipeline_diagnostics_deduplicate_overlapping_project_require_diagnostics() {
    let path = PathBuf::from("src/main.tua");
    let mut config = ProjectConfig::default();

    config.diagnostics.missing_requires = true;

    let mut host = AnalysisHost::with_config(config);
    host.set_source_roots([PathBuf::from("src")]);
    host.set_file(&path, "local Missing = require(\"missing\")\n");

    let diagnostics = host.snapshot().pipeline_diagnostics(&path);
    let missing_requires = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_MISSING_REQUIRE))
        .collect::<Vec<_>>();

    assert_eq!(missing_requires.len(), 1, "{diagnostics:?}");
}

#[test]
fn pipeline_diagnostics_use_configured_asset_roots() {
    let temp = TempProject::new("asset-roots");
    let path = temp.write(
        "src/main.tua",
        "local player = image \"assets/player.png\"\n",
    );

    temp.write("assets/player.png", "");

    let mut host = AnalysisHost::new();
    host.set_source_roots([temp.path().join("src")]);
    host.set_asset_roots([temp.path().to_path_buf()]);
    host.set_file(
        &path,
        fs::read_to_string(&path).expect("source should be readable"),
    );

    assert!(host.snapshot().pipeline_diagnostics(&path).is_empty());
}

#[test]
fn snapshot_surfaces_love_asset_metadata_in_hover_and_completion() {
    let temp = TempProject::new("asset-metadata");
    let source = r#"local player = image "assets/<|hover|>player.png"
local next = image "assets/p<|complete|>"
"#;
    let fixture = Fixture::parse(&format!("-- file: {}\n{source}", "src/main.tua"));

    temp.write("assets/player.png", png_header(64, 32));
    temp.write("assets/jump.wav", b"placeholder");

    let mut host = AnalysisHost::new();
    host.set_source_roots([temp.path().join("src")]);
    host.set_asset_roots([temp.path().to_path_buf()]);

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("asset literal should hover");
    let completions = snapshot.completion("src/main.tua", fixture.position("complete"));

    assert!(hover.contents.contains("asset `assets/player.png`"));
    assert!(hover.contents.contains("LOVE type: Image"));
    assert!(hover.contents.contains("status: valid"));
    assert!(hover.contents.contains("dimensions: 64x32"));
    assert_eq!(
        completion_labels(&completions),
        vec!["assets/player.png".to_owned()]
    );
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("image asset path -> Image")
    );
}

#[test]
fn snapshot_filters_love_asset_path_completion_by_helper_kind() {
    let temp = TempProject::new("asset-kind-completion");
    let source = r#"local player = image "assets/<|image|>"
local jump = sound "assets/<|sound|>"
local ui = font("assets/<|font|>", 18)
local wave = shader "assets/<|shader|>"
"#;
    let fixture = Fixture::parse(&format!("-- file: {}\n{source}", "src/main.tua"));

    temp.write("assets/player.png", png_header(16, 16));
    temp.write("assets/jump.wav", b"placeholder");
    temp.write("assets/ui.ttf", b"placeholder");
    temp.write("assets/wave.glsl", b"placeholder");

    let mut host = AnalysisHost::new();
    host.set_source_roots([temp.path().join("src")]);
    host.set_asset_roots([temp.path().to_path_buf()]);

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();

    assert_eq!(
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("image"))),
        vec!["assets/player.png".to_owned()]
    );
    assert_eq!(
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("sound"))),
        vec!["assets/jump.wav".to_owned()]
    );
    assert_eq!(
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("font"))),
        vec!["assets/ui.ttf".to_owned()]
    );
    assert_eq!(
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("shader"))),
        vec!["assets/wave.glsl".to_owned()]
    );
}

#[test]
fn pipeline_diagnostics_run_formatting_lints_for_lua_sources() {
    let mut config = ProjectConfig::default();

    config.diagnostics.formatting_lints = true;

    let mut host = AnalysisHost::with_config(config);

    host.set_file_with_kind(
        "src/legacy.lua",
        "local value=1   \nreturn { value=value }\n",
        AnalysisSourceKind::Lua,
    );

    let diagnostics = host.snapshot().pipeline_diagnostics("src/legacy.lua");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::LINT_FORMAT_TRAILING_WHITESPACE)
    }));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == Some(DiagnosticCode::LINT_FORMAT_TOKEN_SPACING) }));
}

#[test]
fn pipeline_diagnostics_run_jit_guidance_from_project_config() {
    let path = PathBuf::from("src/main.tua");
    let mut config = ProjectConfig::default();

    config.jit.enabled = true;

    let mut host = AnalysisHost::with_config(config);
    host.set_file(
        &path,
        r#"
local value = 1
value = "shape drift"
"#,
    );

    let diagnostics = host.snapshot().diagnostics(&path);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_JIT_TYPE_DRIFT)));
}

#[test]
fn ecs_domain_support_is_disabled_by_default() {
    let path = PathBuf::from("src/ecs.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, "ecs.\n");

    let snapshot = host.snapshot();

    assert!(snapshot.plugin_registry().is_empty());
    assert!(
        completion_labels(&snapshot.completion(&path, SourcePosition { line: 1, column: 5 }))
            .is_empty()
    );
}

#[test]
fn ecs_domain_support_flows_through_plugin_and_semantic_surfaces() {
    let path = PathBuf::from("src/ecs.tua");
    let completion_path = PathBuf::from("src/ecs_completion.tua");
    let mut config = ProjectConfig::default();

    config.ecs.enabled = true;

    let mut host = AnalysisHost::with_config(config);
    host.set_file(
        &path,
        r#"local Position = ecs.component("Position", { x = "number", y = "number" })
local world = ecs.world()
local entity = world:create()
world:add(entity, Position, { x = 1, y = "slow" })
ecs.system(world, { Position, Missing }, function() end)
"#,
    );
    host.set_file(
        &completion_path,
        r#"local world = ecs.world()
local api = ecs.w
local call = world.a
"#,
    );

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics(&path);
    let ecs_labels = completion_labels(&snapshot.completion(
        &completion_path,
        SourcePosition {
            line: 2,
            column: 18,
        },
    ));
    let world_labels = completion_labels(&snapshot.completion(
        &completion_path,
        SourcePosition {
            line: 3,
            column: 21,
        },
    ));
    let hover = snapshot
        .hover(&path, SourcePosition { line: 2, column: 7 })
        .expect("world local should hover");
    let signature = snapshot
        .signature_help(
            &path,
            SourcePosition {
                line: 4,
                column: 11,
            },
        )
        .expect("world:add should have signature help");

    assert!(snapshot
        .plugin_registry()
        .declarations()
        .iter()
        .any(|declaration| declaration.keyword == "ecs"));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::PLUGIN_ECS_COMPONENT_VALUE)
            && diagnostic.message.contains("expected number")
    }));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::PLUGIN_ECS_QUERY)));
    assert!(ecs_labels.contains(&"world".to_owned()));
    assert!(world_labels.contains(&"add".to_owned()));
    assert_eq!(hover.contents, "local world: EcsWorld");
    assert_eq!(
        signature.signatures[0].label,
        "function world:add(EcsEntityId, EcsComponent, {  }): nil"
    );
}

#[test]
fn pipeline_diagnostics_apply_inline_ignores_from_project_config() {
    let mut config = ProjectConfig::default();

    config.diagnostics.inline_ignores = true;

    let mut host = AnalysisHost::with_config(config);

    host.set_file(
        "src/main.tua",
        r#"
-- tua-ignore-next-line TL1003
local value: number = "legacy"

-- tua-ignore-next-line TL4018
local clean = true
"#,
    );

    let diagnostics = host.snapshot().pipeline_diagnostics("src/main.tua");

    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_INLINE_IGNORE_UNUSED)));
}

fn completion_labels(completions: &[Completion]) -> Vec<String> {
    completions
        .iter()
        .map(|completion| completion.label.clone())
        .collect()
}

#[test]
fn snapshot_recovers_semantics_when_field_chain_statement_is_incomplete() {
    let fixture = Fixture::parse(
        r#"-- file: src/conf.tua
function love.conf(t)
  local bad: number = "oops"
  local title = t.window.<|hover|>title
  t.modules.font<|complete|>
end
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let diagnostics = snapshot.pipeline_diagnostics("src/conf.tua");
    let hover = snapshot
        .hover("src/conf.tua", fixture.position("hover"))
        .expect("hover should recover before incomplete field chain");
    let completions = snapshot.completion("src/conf.tua", fixture.position("complete"));
    let hints = snapshot.inlay_hints("src/conf.tua");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
        "{diagnostics:?}"
    );
    assert!(hover.contents.contains("title"), "{}", hover.contents);
    assert_eq!(completion_labels(&completions), vec!["font".to_owned()]);
    assert!(
        hints.iter().any(|hint| hint.label == ": string"),
        "{hints:?}"
    );
}

#[test]
fn love_conf_parameter_type_does_not_escape_callback_body() {
    let fixture = Fixture::parse(
        r#"-- file: src/conf.tua
function love.conf(t)
  t.window.title = "inside"
end

local outside = t.window.<|hover|>title
local field = t.<|complete|>
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();

    assert!(snapshot
        .hover("src/conf.tua", fixture.position("hover"))
        .is_none());
    assert!(snapshot
        .completion("src/conf.tua", fixture.position("complete"))
        .is_empty());
}

#[test]
fn fast_diagnostics_include_current_file_errors_without_slow_asset_validation() {
    let path = PathBuf::from("src/main.tua");
    let mut config = ProjectConfig::default();
    config.diagnostics.undefined_variables = true;
    let mut host = AnalysisHost::with_config(config);

    host.set_file(
        &path,
        r#"
type Broken = MissingType
local hp: number = "full"
missing()
local sprite = image "missing.png"
"#,
    );

    let diagnostics = host.fast_diagnostics(&path);
    let codes = diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();

    assert!(codes.contains(&DiagnosticCode::TYPE_UNKNOWN_NAME));
    assert!(codes.contains(&DiagnosticCode::TYPE_MISMATCH));
    assert!(codes.contains(&DiagnosticCode::LINT_UNDEFINED_VARIABLE));
    assert!(!codes.contains(&DiagnosticCode::ASSET_MISSING));
}

#[test]
fn cache_reuses_unedited_files_across_snapshots() {
    let main = PathBuf::from("src/main.tua");
    let other = PathBuf::from("src/other.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&main, "local main: number = 1\n");
    host.set_file(&other, "local other: number = 2\n");

    let first = host.snapshot();
    assert!(first.parse(&main).is_some());
    assert!(first.parse(&other).is_some());
    assert_eq!(host.cache_stats().parse_misses, 2);
    assert_eq!(host.cache_stats().parse_hits, 0);

    let stats_after_first = host.cache_stats();
    let second = host.snapshot();
    assert!(second.parse(&main).is_some());
    assert!(second.parse(&other).is_some());

    // A warm host snapshot is memoized by revision, so unchanged snapshots do
    // not need to touch the per-file query cache at all.
    assert_eq!(host.cache_stats(), stats_after_first);
}

#[test]
fn editing_one_file_does_not_reparse_unrelated_files() {
    let main = PathBuf::from("src/main.tua");
    let other = PathBuf::from("src/other.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&main, "local main: number = 1\n");
    host.set_file(&other, "local other: number = 2\n");
    host.snapshot();

    host.set_file(&main, "local main: number = 3\n");
    host.snapshot();

    let stats = host.cache_stats();

    assert_eq!(stats.parse_misses, 3);
    assert_eq!(stats.parse_hits, 1);
    assert_eq!(stats.invalidations, 1);
}

#[test]
fn changing_exported_module_invalidates_known_importers() {
    let temp = TempProject::new("module-dependency");

    temp.write("tua.toml", "source = \"src\"\n");
    let main = temp.write(
        "src/main.tua",
        r#"
local Player = require("player")
return Player.new()
"#,
    );
    let player = temp.write(
        "src/player.tua",
        r#"
local M = {}
function M.new()
  return { hp = 100 }
end
return M
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([temp.path().join("src")]);
    host.set_file(&main, fs::read_to_string(&main).expect("main should read"));
    host.set_file(
        &player,
        fs::read_to_string(&player).expect("player should read"),
    );
    host.snapshot();

    fs::write(
        &player,
        r#"
local M = {}
function M.new()
  return { hp = "full" }
end
return M
"#,
    )
    .expect("player should update");
    host.set_file(
        &player,
        fs::read_to_string(&player).expect("player should read"),
    );

    let after_update = host.cache_stats();
    assert_eq!(after_update.invalidations, 2);
    assert_eq!(after_update.dependent_invalidations, 1);

    host.snapshot();

    let after_snapshot = host.cache_stats();
    assert_eq!(after_snapshot.parse_misses, 3);
    assert_eq!(after_snapshot.parse_hits, 2);
    assert_eq!(after_snapshot.diagnostics_misses, 4);
}

#[test]
fn changing_required_module_body_without_surface_change_keeps_importers_cached() {
    let temp = TempProject::new("module-body-dependency");

    temp.write("tua.toml", "source = \"src\"\n");
    let main = temp.write(
        "src/main.tua",
        r#"
local Player = require("player")
local player = Player.new()
return player.hp
"#,
    );
    let player = temp.write(
        "src/player.tua",
        r#"
local M = {}
function M.new()
  return { hp = 100 }
end
return M
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([temp.path().join("src")]);
    host.set_file(&main, fs::read_to_string(&main).expect("main should read"));
    host.set_file(
        &player,
        fs::read_to_string(&player).expect("player should read"),
    );
    host.snapshot();

    fs::write(
        &player,
        r#"
local M = {}
function M.new()
  local base = 100
  return { hp = base }
end
return M
"#,
    )
    .expect("player should update");
    host.set_file(
        &player,
        fs::read_to_string(&player).expect("player should read"),
    );

    let after_update = host.cache_stats();
    assert_eq!(after_update.invalidations, 1);
    assert_eq!(after_update.dependent_invalidations, 0);

    host.snapshot();

    let after_snapshot = host.cache_stats();
    assert_eq!(after_snapshot.diagnostics_hits, 1);
    assert_eq!(after_snapshot.diagnostics_misses, 3);
}

#[test]
fn cache_stores_derived_query_products() {
    let path = PathBuf::from("src/module.tua");
    let mut host = AnalysisHost::new();

    host.set_file(
        &path,
        r#"
type Player = { hp: number }

local M = {}

function M.new(): Player
  return { hp = 100 }
end

return M
"#,
    );

    let snapshot = host.snapshot();

    assert!(snapshot.type_environment(&path).is_some());
    assert!(snapshot.scope_map(&path).is_some());
    let semantic_file = snapshot
        .semantic_file(&path)
        .expect("semantic file should include parsed source");
    assert!(semantic_file.hir.is_some());
    assert!(semantic_file.scope.is_some());
    assert!(snapshot.module_exports(&path).is_some());
    assert!(snapshot
        .symbol_index_for_path(&path)
        .iter()
        .any(|symbol| symbol.name == "Player"));
    assert!(snapshot.require_edges(&path).is_empty());

    let stats = host.cache_stats();
    assert_eq!(stats.hir_misses, 1);
    assert_eq!(stats.scope_misses, 1);
    assert_eq!(stats.type_environment_misses, 1);
    assert_eq!(stats.diagnostics_misses, 1);
    assert_eq!(stats.module_export_misses, 1);
    assert_eq!(stats.require_edge_misses, 1);
    assert_eq!(stats.symbol_index_misses, 1);
}

#[test]
fn reverse_reference_index_collects_local_and_project_global_references() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local <|local_def|>value = 1
local other = value
-- file: src/globals.tua
function <|global_def|>greet(name: string): string
  return name
end

local own = greet("Local")
-- file: src/use.tua
local message = greet("Ada")
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let local_target = snapshot
        .definition("src/main.tua", fixture.position("local_def"))
        .expect("local definition should resolve");
    let global_target = snapshot
        .definition("src/globals.tua", fixture.position("global_def"))
        .expect("global function definition should resolve");
    let local_references = snapshot
        .reverse_reference_index()
        .references(&local_target, true)
        .expect("local references should be indexed");
    let global_references = snapshot
        .reverse_reference_index()
        .references(&global_target, true)
        .expect("global references should be indexed");

    assert_eq!(local_references.len(), 2);
    assert!(global_references
        .iter()
        .any(|reference| reference.path == Path::new("src/use.tua")));
}

#[test]
fn snapshot_definition_uses_indexed_module_exports_without_reading_files() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Player = require("player")
local player = Player.<|usage|>new()
-- file: src/player.tua
local function <|def|>new<|/def|>()
  return { hp = 100 }
end

return {
  new = new,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let definition = snapshot
        .definition("src/main.tua", fixture.position("usage"))
        .expect("required module export should resolve from snapshot index");

    assert_eq!(definition.path, PathBuf::from("src/player.tua"));
    assert_eq!(definition.span, fixture.range("def"));
}

#[test]
fn snapshot_hover_uses_indexed_module_exports_without_reading_files() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Audio = require("audio")
local ok = Audio.<|hover|>play("theme")
-- file: src/audio.lua
---@param source string audio source
---@return boolean success flag
local function play(source)
  return true
end

return {
  play = play,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("lua")
        {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };

        host.set_file_with_kind(&file.path, file.source.clone(), kind);
    }

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("required module export hover should resolve from snapshot index");

    assert!(hover
        .contents
        .contains("function Audio.play(string): boolean"));
    assert_eq!(
        hover.documentation,
        Some("audio source\nsuccess flag".to_owned())
    );
}

#[test]
fn snapshot_hover_prefers_project_type_aliases_in_type_annotations() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Types = require("types")
local LuaDocTypes = require("lua_doc_types")

local player: Types.<|player_type|>Player = nil
local enemy: LuaDocTypes.<|enemy_type|>Enemy = nil
-- file: src/types.tua
type Player = { hp: number }

return {}
-- file: src/lua_doc_types.lua
---@class Enemy
---@field hp number
---@field name string
---@field type 'friendly' | 'mad'
local Enemy = {}

return {
  Enemy = Enemy,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("lua")
        {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };

        host.set_file_with_kind(&file.path, file.source.clone(), kind);
    }

    let snapshot = host.snapshot();
    let player = snapshot
        .hover("src/main.tua", fixture.position("player_type"))
        .expect("required tua alias should hover");
    let enemy = snapshot
        .hover("src/main.tua", fixture.position("enemy_type"))
        .expect("required lua doc alias should hover");

    assert_eq!(player.contents, "type Types.Player = { hp: number }");
    assert_eq!(
        enemy.contents,
        "type LuaDocTypes.Enemy = { hp: number, name: string, type: \"friendly\" | \"mad\" }"
    );
    assert!(snapshot.diagnostics("src/main.tua").is_empty());
}

#[test]
fn snapshot_uses_project_types_for_local_hover_and_field_completion() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local LuaDocTypes = require("lua_doc_types")

local enemyFromLuaDocModule: LuaDocTypes.Enemy = {
  <|literal|>
}

local a = enemyFromLuaDocModule
a.<|field|>
local copied = <|hover|>enemyFromLuaDocModule
-- file: src/lua_doc_types.lua
---@class Enemy
---@field hp number
---@field name string
---@field type 'friendly' | 'mad'
local Enemy = {}

return {
  Enemy = Enemy,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("lua")
        {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };

        host.set_file_with_kind(&file.path, file.source.clone(), kind);
    }

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("annotated project local should hover");
    let literal_labels = snapshot
        .completion("src/main.tua", fixture.position("literal"))
        .into_iter()
        .map(|completion| completion.label)
        .collect::<Vec<_>>();
    let field_labels = snapshot
        .completion("src/main.tua", fixture.position("field"))
        .into_iter()
        .map(|completion| completion.label)
        .collect::<Vec<_>>();

    assert_eq!(
        hover.contents,
        "local enemyFromLuaDocModule: { hp: number, name: string, type: \"friendly\" | \"mad\" }"
    );
    assert!(literal_labels.contains(&"hp".to_owned()));
    assert!(literal_labels.contains(&"name".to_owned()));
    assert!(literal_labels.contains(&"type".to_owned()));
    assert!(field_labels.contains(&"hp".to_owned()));
    assert!(field_labels.contains(&"name".to_owned()));
}

#[test]
fn snapshot_caches_completion_and_shares_inferred_love_facts() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local function loadPlayerImage()
  return image "assets/player.png"
end

local <|hover|>player = loadPlayerImage()
player:<|complete|>setFilter(<|signature|>"nearest")
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("inferred LOVE local should hover");

    assert_eq!(hover.contents, "local player: Image");
    assert_eq!(snapshot.completion_cache.borrow().len(), 0);

    let completions = snapshot.completion("src/main.tua", fixture.position("complete"));
    let labels = completions
        .iter()
        .map(|completion| completion.label.clone())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"setFilter".to_owned()));
    assert_eq!(snapshot.completion_cache.borrow().len(), 1);
    assert_eq!(
        snapshot
            .completion("src/main.tua", fixture.position("complete"))
            .iter()
            .map(|completion| completion.label.clone())
            .collect::<Vec<_>>(),
        labels
    );
    assert_eq!(snapshot.completion_cache.borrow().len(), 1);
    assert!(snapshot
        .resolve_completion(
            "src/main.tua",
            fixture.position("complete"),
            "setFilter",
            Some(CompletionKind::Function),
            None,
            None,
        )
        .is_some());
    assert_eq!(snapshot.completion_cache.borrow().len(), 1);

    let hints = snapshot.inlay_hints("src/main.tua");

    assert!(hints.iter().any(|hint| hint.label == ": Image"));

    let signature = snapshot
        .signature_help("src/main.tua", fixture.position("signature"))
        .expect("LOVE method signature should resolve");

    assert!(signature.signatures[0].label.contains("setFilter"));
}

#[test]
fn snapshot_definition_cache_covers_the_whole_identifier_token() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
type MetaVector = {
  x: number,
  y: number,
  length: number,
}

local function combineVector(left: MetaVector, right: MetaVector): MetaVector
  return {
    x = left.x + right.x,
    y = left.y + right.y,
    length = left.length + right.<|lookup|>length,
  }
end
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let first_position = fixture.position("lookup");
    let second_position = SourcePosition {
        line: first_position.line,
        column: first_position.column + 4,
    };
    let first = snapshot
        .definition("src/main.tua", first_position)
        .expect("field definition should resolve");
    let cache_len = snapshot.definition_cache.borrow().len();
    let second = snapshot
        .definition("src/main.tua", second_position)
        .expect("field definition should resolve from token cache");

    assert_eq!(first, second);
    assert_eq!(first.kind, DefinitionKind::TableField);
    assert_eq!(
        snapshot.definition_cache.borrow().len(),
        cache_len,
        "lookup inside the same identifier should reuse the warmed cache"
    );
}

#[test]
fn snapshot_definition_resolves_colon_self_field_to_type_shape_field() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
type Account = {
  name: string,
  <|field_def|>balance<|/field_def|>: number,
}

local Account = {}
Account.__index = Account

function Account:new(name, balance)
  local instance = {
    name = name,
    balance = balance,
  }

  return setmetatable(instance, self)
end

function Account:deposit(amount: number): number
  self.<|field_ref|>balance = self.balance + amount

  return self.balance
end
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let first = snapshot
        .definition("src/main.tua", fixture.position("field_ref"))
        .expect("self field definition should resolve through synthetic receiver");
    let cache_len = snapshot.definition_cache.borrow().len();
    let second = snapshot
        .definition("src/main.tua", fixture.position("field_ref"))
        .expect("repeated self field definition should stay cached");

    assert_eq!(first, second);
    assert_eq!(first.kind, DefinitionKind::TableField);
    assert_eq!(first.span, fixture.range("field_def"));
    assert_eq!(snapshot.definition_cache.borrow().len(), cache_len);
}

#[test]
fn snapshot_completion_uses_recovered_semantics_after_incomplete_colon() {
    let valid = r#"type Account = {
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:deposit(amount: number): number
  self.balance = self.balance + amount

  return self.balance
end

local account = setmetatable({ balance = 50 }, Account)
"#;
    let invalid = Fixture::parse(&format!("{valid}account:<|complete|>\n"));
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.open_document(&path, valid);
    let _ = host.snapshot();
    host.update_open_document(&path, invalid.source("src/main.tua"));

    let completions = host
        .snapshot()
        .completion(&path, invalid.position("complete"));

    assert!(
        completion_labels(&completions)
            .iter()
            .any(|label| label == "deposit"),
        "incomplete colon access should preserve metatable method completion"
    );
}

#[test]
fn snapshot_hovers_parseable_lua_doc_project_locals_as_class_shapes() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local LuaDocTypes = require("lua_doc_types")

local <|nil_decl_hover|>nilEnemy: LuaDocTypes.Enemy = nil
local <|table_decl_hover|>tableEnemy: LuaDocTypes.Enemy = {
  hp = 1,
  name = "hello",
}

local nilCopy = <|nil_hover|>nilEnemy
local tableCopy = <|table_hover|>tableEnemy
-- file: src/lua_doc_types.lua
---@class Enemy
---@field hp number
---@field name string
---@field type 'friendly' | 'mad'
local Enemy = {}

return {
  Enemy = Enemy,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("lua")
        {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };

        host.set_file_with_kind(&file.path, file.source.clone(), kind);
    }

    let snapshot = host.snapshot();
    let nil_hover = snapshot
        .hover("src/main.tua", fixture.position("nil_hover"))
        .expect("nil-initialized lua doc local should hover");
    let table_hover = snapshot
        .hover("src/main.tua", fixture.position("table_hover"))
        .expect("table-initialized lua doc local should hover");
    let nil_decl_hover = snapshot
        .hover("src/main.tua", fixture.position("nil_decl_hover"))
        .expect("nil-initialized lua doc local declaration should hover");
    let table_decl_hover = snapshot
        .hover("src/main.tua", fixture.position("table_decl_hover"))
        .expect("table-initialized lua doc local declaration should hover");

    assert_eq!(
        nil_hover.contents,
        "local nilEnemy: { hp: number, name: string, type: \"friendly\" | \"mad\" }"
    );
    assert_eq!(
        table_hover.contents,
        "local tableEnemy: { hp: number, name: string, type: \"friendly\" | \"mad\" }"
    );
    assert_eq!(
        nil_decl_hover.contents,
        "local nilEnemy: { hp: number, name: string, type: \"friendly\" | \"mad\" }"
    );
    assert_eq!(
        table_decl_hover.contents,
        "local tableEnemy: { hp: number, name: string, type: \"friendly\" | \"mad\" }"
    );
}

#[test]
fn snapshot_workspace_symbols_update_after_file_changes() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    host.set_file(
        &path,
        r#"
type Player = { hp: number }
"#,
    );

    let first = host.snapshot();

    assert!(first
        .workspace_symbols("Player")
        .iter()
        .any(|symbol| symbol.name == "Player"));

    host.set_file(
        &path,
        r#"
type Enemy = { hp: number }
"#,
    );

    let second = host.snapshot();

    assert!(second
        .workspace_symbols("Enemy")
        .iter()
        .any(|symbol| symbol.name == "Enemy"));
    assert!(!second
        .workspace_symbols("Player")
        .iter()
        .any(|symbol| symbol.name == "Player"));
}

fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();

    bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    bytes.extend_from_slice(&13u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes.extend_from_slice(&0u32.to_be_bytes());

    bytes
}
