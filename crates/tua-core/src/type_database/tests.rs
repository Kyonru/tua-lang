use std::{
    collections::BTreeSet,
    fmt::Write as _,
    path::{Path, PathBuf},
};

use crate::{AnalysisHost, AnalysisSourceKind, Fixture, ProjectTypeOrigin, TableField, Type};

fn host_from_fixture(fixture: &Fixture) -> AnalysisHost {
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

    host
}

const REEXPORT_DEPTHS: [usize; 5] = [1, 16, 64, 65, 256];

fn reexport_depth_fixture() -> Fixture {
    let mut source = String::from("-- file: src/main.tua\n");

    for depth in REEXPORT_DEPTHS {
        writeln!(
            source,
            "import type {{ Player{depth} }} from \"./chain_{depth:03}/layer_000\""
        )
        .unwrap();
        writeln!(
            source,
            "local _player_{depth}: Player{depth} = {{ health = {depth}, name = \"player\" }}"
        )
        .unwrap();
    }

    for depth in REEXPORT_DEPTHS {
        for layer in 0..depth {
            let target = if layer + 1 == depth {
                "./leaf".to_owned()
            } else {
                format!("./layer_{:03}", layer + 1)
            };
            writeln!(
                source,
                "-- file: src/chain_{depth:03}/layer_{layer:03}.d.tua"
            )
            .unwrap();
            writeln!(source, "export type {{ Player{depth} }} from \"{target}\"").unwrap();
        }
        writeln!(source, "-- file: src/chain_{depth:03}/leaf.d.tua").unwrap();
        writeln!(
            source,
            "export type Player{depth} = {{ health: number, name: string }}"
        )
        .unwrap();
    }

    Fixture::parse(&source)
}

#[test]
fn named_type_reexports_resolve_across_depth_boundaries() {
    let fixture = reexport_depth_fixture();
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();

    assert!(
        snapshot.diagnostics("src/main.tua").is_empty(),
        "unexpected diagnostics: {:#?}",
        snapshot.diagnostics("src/main.tua")
    );

    for depth in REEXPORT_DEPTHS {
        let binding = snapshot
            .type_database()
            .resolve_type_name(
                snapshot.module_graph(),
                "src/main.tua",
                &format!("Player{depth}"),
            )
            .unwrap_or_else(|| panic!("{depth}-hop re-export should resolve"));

        assert_eq!(
            binding.module_path,
            PathBuf::from(format!("src/chain_{depth:03}/leaf.d.tua"))
        );
        assert_eq!(
            binding.ty,
            Type::table([
                ("health", TableField::required(Type::number())),
                ("name", TableField::required(Type::string())),
            ]),
            "{depth}-hop re-export should preserve its structural type"
        );
    }
}

#[test]
fn generic_aliases_resolve_across_named_imports_and_reexports() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type { Box } from "./middle"
local box: Box<number> = { value = 3 }
local _result: number = box.value
-- file: src/middle.tua
export type { Box } from "./box"
-- file: src/box.tua
export type Box<T> = { value: T }
"#,
    );
    let snapshot = host_from_fixture(&fixture).snapshot();
    assert!(
        snapshot
            .type_database()
            .generic_alias_definition(snapshot.module_graph(), "src/main.tua", "Box")
            .is_some(),
        "generic alias should resolve through named re-export"
    );
    assert!(
        snapshot.diagnostics("src/main.tua").is_empty(),
        "unexpected diagnostics: {:#?}",
        snapshot.diagnostics("src/main.tua")
    );

    let environment = snapshot
        .type_environment("src/main.tua")
        .expect("main module should have a type environment");
    let application = environment
        .local("box")
        .expect("annotated local should retain its generic application");
    let resolved =
        snapshot
            .type_database()
            .resolve_type(snapshot.module_graph(), "src/main.tua", application);
    let Type::Table(table) = resolved else {
        panic!("expected a record after generic import resolution, got {resolved}");
    };
    assert_eq!(
        table.semantic_field("value").map(|field| &field.ty),
        Some(&Type::number())
    );
}

#[test]
fn changing_argument_generic_alias_recursion_is_bounded_in_project_snapshots() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type { Grow } from "./aliases"
local node: Grow<number>
-- file: src/aliases.tua
export type Grow<T> = { next: Grow<T[]> }
"#,
    );
    let snapshot = host_from_fixture(&fixture).snapshot();
    let environment = snapshot
        .project_type_environment("src/main.tua")
        .expect("main project environment should exist");
    let mut current = environment.resolve_aliases(environment.local("node").unwrap());
    let mut depth = 0;
    loop {
        match current {
            Type::Table(table) => {
                current = table
                    .semantic_field("next")
                    .expect("each recursive expansion has next")
                    .ty
                    .clone();
                depth += 1;
            }
            Type::GenericAliasApplication { .. } => break,
            other => panic!("expected bounded unresolved project alias, got {other:?}"),
        }
    }
    assert!(
        depth <= 128,
        "project expansion exceeded its depth guard: {depth}"
    );
    assert!(
        depth >= 64,
        "project expansion should preserve useful shape: {depth}"
    );
}

#[test]
fn resolves_aliases_from_required_tua_modules() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Types = require("types")
local player: Types.Player = { hp = 10 }
-- file: src/types.tua
export type Player = { hp: number }

return {}
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let binding = snapshot
        .type_database()
        .resolve_type_name(snapshot.module_graph(), "src/main.tua", "Types.Player")
        .expect("required module alias should resolve");

    assert_eq!(binding.name, "Player");
    assert_eq!(binding.module_path, PathBuf::from("src/types.tua"));
    assert_eq!(binding.origin, ProjectTypeOrigin::Tua);
    assert_eq!(
        binding.ty,
        Type::table([("hp", TableField::required(Type::number()))])
    );
}

#[test]
fn resolves_only_exported_aliases_from_type_only_imports() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Shared from "./types"
local _player: Shared.Player = { hp = 10 }
-- file: src/types.tua
export type Player = { hp: number }
type PrivateState = { debug: boolean }
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let player = snapshot
        .type_database()
        .resolve_type_name(snapshot.module_graph(), "src/main.tua", "Shared.Player")
        .expect("exported type-only alias should resolve");

    assert_eq!(player.name, "Player");
    assert_eq!(player.module_path, PathBuf::from("src/types.tua"));
    assert_eq!(
        player.ty,
        Type::table([("hp", TableField::required(Type::number()))])
    );
    assert!(snapshot
        .type_database()
        .resolve_type_name(
            snapshot.module_graph(),
            "src/main.tua",
            "Shared.PrivateState"
        )
        .is_none());
    let diagnostics = snapshot.diagnostics("src/main.tua");
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn exported_protocols_resolve_through_type_only_imports() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type { Updatable } from "./actor"
local function _tick(value: Updatable, dt: number)
  value:update(dt)
end
final class Panel implements Updatable {
  update(dt: number): nil {
  }
}

local _panel: Updatable = Panel.new()
final class BrokenPanel implements Updatable {
}
final class BadPanel implements Updatable {
  update(dt: string): nil {
  }
}
-- file: src/actor.tua
export protocol Updatable {
  update(dt: number): nil
}
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let binding = snapshot
        .type_database()
        .resolve_type_name(snapshot.module_graph(), "src/main.tua", "Updatable")
        .expect("exported protocol should be available to type-only imports");
    assert_eq!(binding.name, "Updatable");
    assert_eq!(binding.module_path, PathBuf::from("src/actor.tua"));
    assert!(matches!(binding.ty, Type::Table(_)));
    assert!(
        snapshot
            .diagnostics("src/main.tua")
            .iter()
            .any(|diagnostic| {
                diagnostic.message.contains("BrokenPanel")
                    && diagnostic.message.contains("missing method `update`")
            }),
        "project analysis should validate imported protocol implementations: {:#?}",
        snapshot.diagnostics("src/main.tua")
    );
    assert!(
        snapshot
            .diagnostics("src/main.tua")
            .iter()
            .any(|diagnostic| {
                diagnostic.message.contains("BadPanel.update")
                    && diagnostic.message.contains("does not satisfy protocol")
            }),
        "project analysis should validate imported protocol signatures: {:#?}",
        snapshot.diagnostics("src/main.tua")
    );
    assert!(
        !snapshot
            .diagnostics("src/main.tua")
            .iter()
            .any(|diagnostic| {
                diagnostic.message.contains("class `Panel`")
                    && diagnostic.message.contains("does not implement protocol")
            }),
        "the valid implementation should remain accepted: {:#?}",
        snapshot.diagnostics("src/main.tua")
    );
}

#[test]
fn imported_protocol_conformance_ignores_trailing_method_comments() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type { Updatable } from "./contracts"
final class Actor implements Updatable {
  update(dt: number): nil {}
}
local _actor: Updatable = Actor.new()
-- file: src/contracts.d.tua
export protocol Updatable {
  update(dt: number): nil -- advance one simulation step
}
"#,
    );
    let host = host_from_fixture(&fixture);
    let diagnostics = host.snapshot().diagnostics("src/main.tua");

    assert!(
        diagnostics.is_empty(),
        "a trailing contract comment must not change its signature: {diagnostics:#?}"
    );
}

#[test]
fn generic_aliases_resolve_in_imported_protocol_requirements_and_class_methods() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type { Box, Consumer } from "./contracts"
final class GoodConsumer implements Consumer {
  consume(value: Box<number>): nil {
  }
}
final class BadConsumer implements Consumer {
  consume(value: Box<string>): nil {
  }
}
-- file: src/contracts.tua
export type Box<T> = { value: T }
export protocol Consumer {
  consume(value: Box<number>): nil
}
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/main.tua");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("BadConsumer")
                && diagnostic.message.contains("consume")
        }),
        "generic alias in an imported protocol requirement must distinguish incompatible implementations: {diagnostics:#?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("GoodConsumer")),
        "matching generic alias method signature should conform: {diagnostics:#?}"
    );
}

#[test]
fn namespace_imported_generic_aliases_resolve_in_types_and_protocols() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Contracts from "./contracts"
local _box: Contracts.Box<number> = { value = 1 }
local _bad: Contracts.Box<number, string>
local _bare: Contracts.Box
final class GoodConsumer implements Contracts.Consumer {
  consume(value: Contracts.Box<number>): nil {
  }
}
final class BadConsumer implements Contracts.Consumer {
  consume(value: Contracts.Box<string>): nil {
  }
}
-- file: src/contracts.tua
export type Box<T> = { value: T }
export protocol Consumer {
  consume(value: Box<number>): nil
}
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/main.tua");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(crate::DiagnosticCode::TYPE_ARGUMENT_COUNT)
                && diagnostic
                    .message
                    .contains("expects 1 type argument(s), found 2")
        }),
        "wrong namespace generic arity should be diagnosed: {diagnostics:#?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(crate::DiagnosticCode::TYPE_ARGUMENT_COUNT)
                && diagnostic
                    .message
                    .contains("expects 1 type argument(s), found 0")
        }),
        "bare namespace generic alias should be diagnosed: {diagnostics:#?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("BadConsumer")
                && diagnostic.message.contains("consume")
        }),
        "namespace imported protocol should reject incompatible method signatures: {diagnostics:#?}"
    );
    assert!(
        !diagnostics.iter().any(|diagnostic| diagnostic.message.contains("GoodConsumer")),
        "namespace imported protocol should accept matching generic alias method signatures: {diagnostics:#?}"
    );
}

#[test]
fn namespace_protocol_reexports_preserve_the_original_contract_identity() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Relay from "./relay"
final class Valid implements Relay.Updatable {
  update(dt: number): nil {}
}
final class Missing implements Relay.Updatable {
}
-- file: src/relay.d.tua
import type { Updatable } from "./contracts"
export type { Updatable } from "./contracts"
-- file: src/contracts.d.tua
export protocol Updatable {
  update(dt: number): nil
}
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let binding = snapshot
        .type_database()
        .resolve_type_name(snapshot.module_graph(), "src/main.tua", "Relay.Updatable")
        .expect("a namespace should expose a re-exported protocol");
    assert_eq!(binding.name, "Updatable");
    assert_eq!(binding.module_path, PathBuf::from("src/contracts.d.tua"));

    let diagnostics = snapshot.diagnostics("src/main.tua");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Missing")
                && diagnostic.message.contains("missing method `update`")
        }),
        "the namespace re-export should retain the source protocol contract: {diagnostics:#?}"
    );
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Valid")
                && diagnostic.message.contains("does not satisfy protocol")
        }),
        "a valid namespace re-export implementation should remain accepted: {diagnostics:#?}"
    );
}

#[test]
fn declaration_modules_support_named_imports_aliases_and_reexports() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type { Hero as Player, PublicRoom } from "./api"
local _player: Player = { hp = 10 }
local _room: PublicRoom = { name = "start" }
-- file: src/api.d.tua
export type { Player as Hero } from "./models"
export type PublicRoom = { name: string }
-- file: src/models.d.tua
export type Player = { hp: number }
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();

    assert!(
        snapshot.diagnostics("src/main.tua").is_empty(),
        "unexpected diagnostics: {:#?}",
        snapshot.diagnostics("src/main.tua")
    );
    let player = snapshot
        .type_database()
        .resolve_type_name(snapshot.module_graph(), "src/main.tua", "Player")
        .expect("named re-export should resolve");
    assert_eq!(
        player.ty,
        Type::table([("hp", TableField::required(Type::number()))])
    );
    let api_exports = snapshot
        .type_database()
        .exported_types(snapshot.module_graph())
        .into_iter()
        .filter(|candidate| candidate.module_path == Path::new("src/api.d.tua"))
        .map(|candidate| candidate.name)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        api_exports,
        BTreeSet::from(["Hero".to_owned(), "PublicRoom".to_owned()])
    );
}

#[test]
fn cyclic_type_reexports_report_a_dedicated_diagnostic() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type { Player } from "./a"
local player: Player
-- file: src/a.d.tua
export type { Player } from "./b"
-- file: src/b.d.tua
export type { Player } from "./a"
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/main.tua");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == Some(crate::DiagnosticCode::TYPE_IMPORT_CYCLE) }));
}

#[test]
fn deeply_nested_type_reexports_resolve_without_using_the_call_stack() {
    const REEXPORT_COUNT: usize = 512;

    let mut source = String::from(
        "-- file: src/main.tua\nimport type { Player } from \"./layer_000\"\nlocal _player: Player = { hp = 10 }\n",
    );
    for index in 0..REEXPORT_COUNT {
        source.push_str(&format!("-- file: src/layer_{index:03}.d.tua\n"));
        if index + 1 == REEXPORT_COUNT {
            source.push_str("export type Player = { hp: number }\n");
        } else {
            source.push_str(&format!(
                "export type {{ Player }} from \"./layer_{:03}\"\n",
                index + 1
            ));
        }
    }

    let fixture = Fixture::parse(&source);
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let binding = snapshot
        .type_database()
        .resolve_type_name(snapshot.module_graph(), "src/main.tua", "Player")
        .expect("deeply re-exported type should resolve");

    assert_eq!(
        binding.ty,
        Type::table([("hp", TableField::required(Type::number()))])
    );
    assert!(
        snapshot.diagnostics("src/main.tua").is_empty(),
        "unexpected diagnostics: {:#?}",
        snapshot.diagnostics("src/main.tua")
    );
}

#[test]
fn type_import_names_cannot_shadow_runtime_or_local_type_names() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Shared from "./types"
local Shared = {}
-- file: src/types.d.tua
export type Player = { hp: number }
"#,
    );
    let host = host_from_fixture(&fixture);
    let diagnostics = host.snapshot().diagnostics("src/main.tua");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(crate::DiagnosticCode::TYPE_IMPORT_COLLISION)
    }));
}

#[test]
fn runtime_module_aliases_obey_tua_export_visibility() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Types = require("types")
local _player: Types.PrivatePlayer
-- file: src/types.tua
type PrivatePlayer = { hp: number }
return {}
"#,
    );
    let host = host_from_fixture(&fixture);
    let diagnostics = host.snapshot().diagnostics("src/main.tua");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(crate::DiagnosticCode::TYPE_UNKNOWN_NAME)
            && diagnostic.message.contains("Types.PrivatePlayer")
    }));
}

#[test]
fn type_only_imports_participate_in_project_type_checking() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Shared from "./types"
local player: Shared.Player = { hp = "wrong" }
-- file: src/types.tua
export type Player = { hp: number }
"#,
    );
    let host = host_from_fixture(&fixture);
    let diagnostics = host.snapshot().diagnostics("src/main.tua");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::DiagnosticCode::TYPE_MISMATCH) }),
        "expected imported contract mismatch: {diagnostics:#?}"
    );
}

#[test]
fn type_only_imports_remain_precise_in_function_signatures() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Shared from "./types"
local function keep(player: Shared.Player): Shared.Player
  return player
end
keep({ hp = "wrong" })
-- file: src/types.tua
export type Player = { hp: number }
"#,
    );
    let diagnostics = host_from_fixture(&fixture)
        .snapshot()
        .diagnostics("src/main.tua");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::DiagnosticCode::TYPE_MISMATCH) }),
        "expected imported function contract mismatch: {diagnostics:#?}"
    );
}

#[test]
fn type_only_imports_reject_private_or_missing_aliases() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Shared from "./types"
local hidden: Shared.PrivateState
local missing: Shared.DoesNotExist
-- file: src/types.tua
type PrivateState = { debug: boolean }
"#,
    );
    let diagnostics = host_from_fixture(&fixture)
        .snapshot()
        .diagnostics("src/main.tua");
    let unknown = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(crate::DiagnosticCode::TYPE_UNKNOWN_NAME))
        .count();

    assert_eq!(
        unknown, 2,
        "expected private and missing type errors: {diagnostics:#?}"
    );
}

#[test]
fn recursive_type_only_imports_do_not_create_runtime_require_cycles() {
    let fixture = Fixture::parse(
        r#"-- file: src/a.tua
import type BTypes from "./b"
export type A = { b: BTypes.B }
-- file: src/b.tua
import type ATypes from "./a"
export type B = { a: ATypes.A | nil }
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();

    assert!(snapshot.module_graph().cycles().is_empty());
    assert!(snapshot.diagnostics("src/a.tua").is_empty());
    assert!(snapshot.diagnostics("src/b.tua").is_empty());
    assert!(snapshot
        .module_graph()
        .reverse_dependency_closure(["src/a.tua"])
        .contains(&PathBuf::from("src/b.tua")));
}

#[test]
fn required_tua_aliases_do_not_report_unknown_type_diagnostics() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Types = require("types")
local player: Types.Player = { hp = 10 }
-- file: src/types.tua
export type Player = { hp: number }

return {}
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/main.tua");

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unknown type `Types.Player`")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn resolves_aliases_from_required_lua_doc_modules() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Types = require("types")
local player: Types.Player = { hp = 10 }
-- file: src/types.lua
---@class Player
---@field hp number hit points
local Player = {}

return {
  Player = Player,
}
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let binding = snapshot
        .type_database()
        .resolve_type_name(snapshot.module_graph(), "src/main.tua", "Types.Player")
        .expect("lua doc class should resolve");

    assert_eq!(binding.origin, ProjectTypeOrigin::LuaDoc);
    assert_eq!(
        binding.ty,
        Type::table([("hp", TableField::required(Type::number()))])
    );
}

#[test]
fn resolves_required_module_export_field_types() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Factory = require("factory")
local player = Factory.newPlayer()
-- file: src/factory.tua
export type Player = { hp: number }

local function newPlayer(): Player
  return { hp = 10 }
end

return {
  newPlayer = newPlayer,
}
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let ty = snapshot
        .type_database()
        .module_export_type(
            snapshot.module_graph(),
            "src/main.tua",
            "Factory",
            &[String::from("newPlayer")],
        )
        .expect("export should resolve");

    assert!(matches!(ty, Type::Function(_)));
}

#[test]
fn keeps_multi_return_types_for_module_exported_functions() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Geometry = require("geometry")
local w, h = Geometry.size()
-- file: src/geometry.tua
local function size()
  return 16, 24
end

return {
  size = size,
}
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let function = snapshot
        .type_database()
        .typed_function_for_fields(
            snapshot.module_graph(),
            "src/main.tua",
            "Geometry",
            &[String::from("size")],
        )
        .expect("exported function should resolve");

    assert_eq!(function.return_types, vec![Type::number(), Type::number()]);
}

#[test]
fn invalidates_project_type_data_when_required_module_changes() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Types = require("types")
local player: Types.Player = { hp = 10 }
-- file: src/types.tua
export type Player = { hp: number }

return {}
"#,
    );
    let mut host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let initial = snapshot
        .type_database()
        .resolve_type_name(snapshot.module_graph(), "src/main.tua", "Types.Player")
        .expect("initial alias should resolve");

    assert_eq!(
        initial.ty,
        Type::table([("hp", TableField::required(Type::number()))])
    );

    host.set_file(
        "src/types.tua",
        r#"
export type Player = { name: string }

return {}
"#,
    );

    let snapshot = host.snapshot();
    let updated = snapshot
        .type_database()
        .resolve_type_name(snapshot.module_graph(), "src/main.tua", "Types.Player")
        .expect("updated alias should resolve");

    assert_eq!(
        updated.ty,
        Type::table([("name", TableField::required(Type::string()))])
    );
}

#[test]
fn recursive_aliases_terminate_at_cycle() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Types = require("types")
local node: Types.Node
-- file: src/types.lua
---@meta
---@alias Node { value: number, next: Node }
return {}
"#,
    );
    let host = host_from_fixture(&fixture);
    let snapshot = host.snapshot();
    let binding = snapshot
        .type_database()
        .resolve_type_name(snapshot.module_graph(), "src/main.tua", "Types.Node")
        .expect("recursive alias should resolve");

    let Type::Table(table) = binding.ty else {
        panic!("expected recursive alias to retain outer table shape");
    };
    assert_eq!(
        table.semantic_field("value").map(|field| &field.ty),
        Some(&Type::number())
    );
    let Type::Table(next) = &table.semantic_field("next").expect("next field").ty else {
        panic!("expected recursive edge to retain one shallow table");
    };
    assert_eq!(
        next.semantic_field("next").map(|field| &field.ty),
        Some(&Type::Any)
    );
}
