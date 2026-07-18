use std::path::PathBuf;

use super::*;
use crate::{
    infer_module_exports, lower_parsed_source, parse_tua_source, AnalysisSourceKind,
    ModuleGraphInput, ModuleGraphOptions,
};

#[test]
fn indexes_hir_symbols_modules_and_exports() {
    let parsed = parse_tua_source(
        "src/player.tua",
        r#"
type Player = {
  hp: number,
}

local count: number = 1

local function damage(player: Player, amount: number): Player
  return player
end

return {
  damage = damage,
}
"#,
    )
    .expect("source should parse");
    let hir = lower_parsed_source(&parsed);
    let exports = infer_module_exports(&parsed).expect("exports should infer");
    let graph = ModuleGraph::build(
        [ModuleGraphInput {
            path: &parsed.path,
            parsed: Some(&parsed),
            hir: Some(&hir),
            exports: Some(&exports),
            source_kind: AnalysisSourceKind::Tua,
        }],
        ModuleGraphOptions {
            source_roots: vec![PathBuf::from("src")],
            output_roots: Vec::new(),
            extension: "tua".to_owned(),
            require_path_separator: crate::vfs::ModulePathSeparator::Dot,
        },
    );
    let index = SymbolIndex::build(
        [(&parsed.path, Some(&hir))],
        &graph,
        &[PathBuf::from("src")],
    );
    let entries = index
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.name.as_str(),
                entry.kind,
                entry.container_name.as_deref(),
            )
        })
        .collect::<Vec<_>>();

    assert!(entries.contains(&("player", SymbolIndexKind::Module, None)));
    assert!(entries.contains(&("Player", SymbolIndexKind::TypeAlias, Some("player"))));
    assert!(entries.contains(&("hp", SymbolIndexKind::Field, Some("Player"))));
    assert!(entries.contains(&("count", SymbolIndexKind::Local, Some("player"))));
    assert!(entries.contains(&("damage", SymbolIndexKind::Function, Some("player"))));
    assert!(entries.contains(&("player", SymbolIndexKind::Parameter, Some("damage"))));
    assert!(entries.contains(&("amount", SymbolIndexKind::Parameter, Some("damage"))));
    assert!(entries.contains(&("damage", SymbolIndexKind::Export, Some("player"))));
}

#[test]
fn workspace_symbols_are_served_from_indexable_entries() {
    let parsed = parse_tua_source(
        "src/player.tua",
        r#"
type Player = { hp: number }

local function damage(player: Player): Player
  return player
end
"#,
    )
    .expect("source should parse");
    let hir = lower_parsed_source(&parsed);
    let graph = ModuleGraph::build(
        [ModuleGraphInput {
            path: &parsed.path,
            parsed: Some(&parsed),
            hir: Some(&hir),
            exports: None,
            source_kind: AnalysisSourceKind::Tua,
        }],
        ModuleGraphOptions {
            source_roots: vec![PathBuf::from("src")],
            output_roots: Vec::new(),
            extension: "tua".to_owned(),
            require_path_separator: crate::vfs::ModulePathSeparator::Dot,
        },
    );
    let index = SymbolIndex::build(
        [(&parsed.path, Some(&hir))],
        &graph,
        &[PathBuf::from("src")],
    );
    let symbols = index.workspace_symbols("plr");

    assert!(symbols
        .iter()
        .any(|symbol| symbol.name == "Player" && symbol.kind == WorkspaceSymbolKind::TypeAlias));
    assert!(symbols
        .iter()
        .all(|symbol| symbol.name != "player" || symbol.kind == WorkspaceSymbolKind::Module));
    assert!(!symbols.iter().any(|symbol| symbol.name == "hp"));
}

#[test]
fn workspace_symbol_index_preserves_class_kind_and_method_container() {
    let parsed = parse_tua_source(
        "src/beats.tua",
        r#"
final class BeatManager {
  steps: number[]

  init() {
    self.steps = {}
  }

  add(step: number) {
    table.insert(self.steps, step)
  }
}
"#,
    )
    .expect("class source should parse");
    let hir = lower_parsed_source(&parsed);
    let graph = ModuleGraph::build(
        [ModuleGraphInput {
            path: &parsed.path,
            parsed: Some(&parsed),
            hir: Some(&hir),
            exports: None,
            source_kind: AnalysisSourceKind::Tua,
        }],
        ModuleGraphOptions {
            source_roots: vec![PathBuf::from("src")],
            output_roots: Vec::new(),
            extension: "tua".to_owned(),
            require_path_separator: crate::vfs::ModulePathSeparator::Dot,
        },
    );
    let index = SymbolIndex::build(
        [(&parsed.path, Some(&hir))],
        &graph,
        &[PathBuf::from("src")],
    );
    let symbols = index.workspace_symbols("");

    assert!(symbols.iter().any(|symbol| {
        symbol.name == "BeatManager" && symbol.kind == WorkspaceSymbolKind::Class
    }));
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "add" && symbol.container_name.as_deref() == Some("BeatManager")
    }));
}

#[test]
fn importable_symbols_are_export_entries_sorted_by_name_and_module() {
    let player = parse_tua_source(
        "src/player.tua",
        r#"
local function makePlayer()
  return { hp = 100 }
end

return { makePlayer = makePlayer }
"#,
    )
    .expect("player should parse");
    let enemy = parse_tua_source(
        "src/enemy.tua",
        r#"
local function makeEnemy()
  return { hp = 10 }
end

return { makeEnemy = makeEnemy }
"#,
    )
    .expect("enemy should parse");
    let player_hir = lower_parsed_source(&player);
    let enemy_hir = lower_parsed_source(&enemy);
    let player_exports = infer_module_exports(&player).expect("player exports should infer");
    let enemy_exports = infer_module_exports(&enemy).expect("enemy exports should infer");
    let graph = ModuleGraph::build(
        [
            ModuleGraphInput {
                path: &player.path,
                parsed: Some(&player),
                hir: Some(&player_hir),
                exports: Some(&player_exports),
                source_kind: AnalysisSourceKind::Tua,
            },
            ModuleGraphInput {
                path: &enemy.path,
                parsed: Some(&enemy),
                hir: Some(&enemy_hir),
                exports: Some(&enemy_exports),
                source_kind: AnalysisSourceKind::Tua,
            },
        ],
        ModuleGraphOptions {
            source_roots: vec![PathBuf::from("src")],
            output_roots: Vec::new(),
            extension: "tua".to_owned(),
            require_path_separator: crate::vfs::ModulePathSeparator::Dot,
        },
    );
    let index = SymbolIndex::build(
        [
            (&player.path, Some(&player_hir)),
            (&enemy.path, Some(&enemy_hir)),
        ],
        &graph,
        &[PathBuf::from("src")],
    );
    let imports = index.importable_symbols("make");
    let labels = imports
        .iter()
        .map(|entry| (entry.name.as_str(), entry.module_name.as_deref()))
        .collect::<Vec<_>>();

    assert_eq!(
        labels,
        vec![("makeEnemy", Some("enemy")), ("makePlayer", Some("player"))]
    );
    assert!(imports.iter().all(|entry| entry.export.is_some()));

    let case_insensitive = index.importable_symbols("PLAYER");

    assert_eq!(case_insensitive.len(), 1);
    assert_eq!(case_insensitive[0].name, "makePlayer");

    let contains = index.importable_symbols("Enemy");

    assert_eq!(contains.len(), 1);
    assert_eq!(contains[0].name, "makeEnemy");
}

#[test]
fn exact_candidate_sets_are_available_for_reference_indexing() {
    let parsed = parse_tua_source(
        "src/player.tua",
        r#"
type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 100 }
end

return { makePlayer = makePlayer }
"#,
    )
    .expect("source should parse");
    let hir = lower_parsed_source(&parsed);
    let exports = infer_module_exports(&parsed).expect("exports should infer");
    let graph = ModuleGraph::build(
        [ModuleGraphInput {
            path: &parsed.path,
            parsed: Some(&parsed),
            hir: Some(&hir),
            exports: Some(&exports),
            source_kind: AnalysisSourceKind::Tua,
        }],
        ModuleGraphOptions {
            source_roots: vec![PathBuf::from("src")],
            output_roots: Vec::new(),
            extension: "tua".to_owned(),
            require_path_separator: crate::vfs::ModulePathSeparator::Dot,
        },
    );
    let index = SymbolIndex::build(
        [(&parsed.path, Some(&hir))],
        &graph,
        &[PathBuf::from("src")],
    );

    assert!(index
        .names_for_kind(SymbolIndexKind::TypeAlias)
        .contains("Player"));
    assert!(index
        .export_names_for_module_path("src/player.tua")
        .contains("makePlayer"));
    assert!(index
        .export_names_for_module_path("src/missing.tua")
        .is_empty());
}
