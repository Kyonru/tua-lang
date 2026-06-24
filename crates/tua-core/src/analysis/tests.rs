use super::*;
use crate::{DiagnosticCode, Fixture};
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

    fn write(&self, path: &str, source: &str) -> PathBuf {
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

    let second = host.snapshot();
    assert!(second.parse(&main).is_some());
    assert!(second.parse(&other).is_some());
    assert_eq!(host.cache_stats().parse_misses, 2);
    assert_eq!(host.cache_stats().parse_hits, 2);
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
  return { hp = 200 }
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
    assert_eq!(after_snapshot.parse_misses, 4);
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
    assert!(snapshot.module_exports(&path).is_some());
    assert!(snapshot
        .symbol_index(&path)
        .iter()
        .any(|symbol| symbol.name == "Player"));
    assert!(snapshot.require_edges(&path).is_empty());

    let stats = host.cache_stats();
    assert_eq!(stats.type_environment_misses, 1);
    assert_eq!(stats.diagnostics_misses, 1);
    assert_eq!(stats.module_export_misses, 1);
    assert_eq!(stats.require_edge_misses, 1);
    assert_eq!(stats.symbol_index_misses, 1);
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
        "type LuaDocTypes.Enemy = { hp: number, name: string, type: string }"
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
        "local enemyFromLuaDocModule: { hp: number, name: string, type: string }"
    );
    assert!(literal_labels.contains(&"hp".to_owned()));
    assert!(literal_labels.contains(&"name".to_owned()));
    assert!(field_labels.contains(&"hp".to_owned()));
    assert!(field_labels.contains(&"name".to_owned()));
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
        "local nilEnemy: { hp: number, name: string, type: string }"
    );
    assert_eq!(
        table_hover.contents,
        "local tableEnemy: { hp: number, name: string, type: string }"
    );
    assert_eq!(
        nil_decl_hover.contents,
        "local nilEnemy: { hp: number, name: string, type: string }"
    );
    assert_eq!(
        table_decl_hover.contents,
        "local tableEnemy: { hp: number, name: string, type: string }"
    );
}
