use super::*;
use tua_core::AnnotationMode;

#[test]
fn line_map_skips_erased_type_alias_lines() {
    let source = r#"
type Player = {
  hp: number,
}

local player: Player = { hp = 10 }
player.hp += 1
"#;
    let emission = tua_core::emit_tua_source_artifact("src/main.tua", source)
        .expect("source should emit with provenance");
    let mappings = line_mappings(&emission.provenance);

    assert_eq!(
        mappings,
        vec![
            LineMapping {
                generated_line: 1,
                source_line: 6,
            },
            LineMapping {
                generated_line: 2,
                source_line: 7,
            },
        ]
    );
}

#[test]
fn line_map_keeps_default_parameter_guards_on_parameter_source_line() {
    let source = "local function greet(name: string = \"Ada\")\n  return name\nend\n";
    let emission = tua_core::emit_tua_source_artifact("src/main.tua", source)
        .expect("source should emit with provenance");
    let mappings = line_mappings(&emission.provenance);

    assert_eq!(
        mappings,
        vec![
            LineMapping {
                generated_line: 1,
                source_line: 1,
            },
            LineMapping {
                generated_line: 2,
                source_line: 1,
            },
            LineMapping {
                generated_line: 3,
                source_line: 1,
            },
            LineMapping {
                generated_line: 4,
                source_line: 1,
            },
            LineMapping {
                generated_line: 5,
                source_line: 2,
            },
            LineMapping {
                generated_line: 6,
                source_line: 3,
            },
        ]
    );
}

#[test]
fn source_map_serializes_static_import_provenance() {
    let source = "local Shared: any = import(\"shared\")\nreturn Shared\n";
    let emission = tua_core::emit_tua_source_artifact("src/main.tua", source)
        .expect("static import should emit with provenance");
    let map = source_map_json(
        Path::new("project"),
        Path::new("project/src/main.tua"),
        Path::new("project/build/main.lua"),
        line_mappings(&emission.provenance),
    );

    assert_eq!(
        map,
        json!({
            "version": 1,
            "generated": "build/main.lua",
            "source": "src/main.tua",
            "lineMappings": [
                { "generatedLine": 1, "sourceLine": 1 },
                { "generatedLine": 2, "sourceLine": 2 },
            ],
        })
    );
}

#[test]
fn debugger_source_map_serializes_standard_v3_mappings() {
    let mappings = vec![
        LineMapping {
            generated_line: 1,
            source_line: 1,
        },
        LineMapping {
            generated_line: 2,
            source_line: 2,
        },
        LineMapping {
            generated_line: 3,
            source_line: 8,
        },
        LineMapping {
            generated_line: 4,
            source_line: 3,
        },
    ];
    let map = debugger_source_map_json(
        Path::new("project"),
        Path::new("project/src/main.tua"),
        Path::new("project/build/main.lua"),
        &mappings,
    );

    assert_eq!(
        map,
        json!({
            "version": 3,
            "file": "main.lua",
            "sourceRoot": "..",
            "sources": ["src/main.tua"],
            "names": [],
            "mappings": "AAAA;AACA;AAMA;AALA",
        })
    );
}

#[test]
fn debugger_source_map_uses_nested_output_depth_for_source_root() {
    let map = debugger_source_map_json(
        Path::new("project"),
        Path::new("project/src/systems/player.tua"),
        Path::new("project/build/systems/player.lua"),
        &[LineMapping {
            generated_line: 1,
            source_line: 5,
        }],
    );

    assert_eq!(map["sourceRoot"], "../..");
    assert_eq!(map["sources"], json!(["src/systems/player.tua"]));
    assert_eq!(map["mappings"], "AAIA");
}

#[test]
fn source_map_serializes_luacats_annotation_provenance() {
    let source = "type Label = string\nlocal title: Label = \"Olá\"\n";
    let emission = tua_core::emit_tua_source_artifact_with_mode(
        "src/main.tua",
        source,
        AnnotationMode::Luacats,
    )
    .expect("LuaCATS annotations should emit with provenance");
    let map = source_map_json(
        Path::new("project"),
        Path::new("project/src/main.tua"),
        Path::new("project/build/main.lua"),
        line_mappings(&emission.provenance),
    );

    assert_eq!(
        emission.code,
        "---@alias Label string\n\n---@type Label\nlocal title = \"Olá\"\n"
    );
    assert_eq!(
        map,
        json!({
            "version": 1,
            "generated": "build/main.lua",
            "source": "src/main.tua",
            "lineMappings": [
                { "generatedLine": 1, "sourceLine": 1 },
                { "generatedLine": 2, "sourceLine": 1 },
                { "generatedLine": 3, "sourceLine": 2 },
                { "generatedLine": 4, "sourceLine": 2 },
            ],
        })
    );
}

#[test]
fn source_map_keeps_multiline_and_non_ascii_source_lines() {
    let source = r#"type Message = {
  text: string,
}

local greeting: string = "Olá"
"#;
    let emission = tua_core::emit_tua_source_artifact("src/main.tua", source)
        .expect("multiline non-ASCII source should emit with provenance");
    let map = source_map_json(
        Path::new("project"),
        Path::new("project/src/main.tua"),
        Path::new("project/build/main.lua"),
        line_mappings(&emission.provenance),
    );

    assert_eq!(emission.code, "local greeting = \"Olá\"\n");
    assert_eq!(
        map,
        json!({
            "version": 1,
            "generated": "build/main.lua",
            "source": "src/main.tua",
            "lineMappings": [
                { "generatedLine": 1, "sourceLine": 5 },
            ],
        })
    );
}
