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
    let output = "local player = { hp = 10 }\nplayer.hp = player.hp + 1\n";
    let parsed = parse_tua_source("src/main.tua", source).expect("source should parse");
    let mappings = line_mappings(source, output, Some(&parsed));

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
    let output = "local function greet(name)\n  if name == nil then\n    name = \"Ada\"\n  end\n  return name\nend\n";
    let parsed = parse_tua_source("src/main.tua", source).expect("source should parse");
    let mappings = line_mappings(source, output, Some(&parsed));

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
