use std::path::PathBuf;

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

#[test]
fn resolves_aliases_from_required_tua_modules() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Types = require("types")
local player: Types.Player = { hp = 10 }
-- file: src/types.tua
type Player = { hp: number }

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
fn required_tua_aliases_do_not_report_unknown_type_diagnostics() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Types = require("types")
local player: Types.Player = { hp = 10 }
-- file: src/types.tua
type Player = { hp: number }

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
type Player = { hp: number }

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
type Player = { hp: number }

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
type Player = { name: string }

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
