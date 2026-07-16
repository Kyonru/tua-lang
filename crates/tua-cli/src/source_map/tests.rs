use super::*;

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
