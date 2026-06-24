use super::*;

#[test]
fn parses_single_file_markers_and_expectations() {
    let fixture = Fixture::parse(
        r#"local <|name|>player<|/name|> = <|cursor|>1
-- expect completions: Player, Vec2
"#,
    );

    assert_eq!(fixture.files().len(), 1);
    assert_eq!(fixture.files()[0].path, PathBuf::from("src/main.tua"));
    assert_eq!(fixture.source("src/main.tua"), "local player = 1\n\n");
    assert_eq!(
        fixture.position("cursor"),
        SourcePosition {
            line: 1,
            column: 16
        }
    );
    assert_eq!(
        fixture.range("name"),
        SourceSpan {
            start: SourcePosition { line: 1, column: 7 },
            end: SourcePosition {
                line: 1,
                column: 13
            },
        }
    );
    assert_eq!(
        fixture.expected_list("completions"),
        vec!["Player".to_owned(), "Vec2".to_owned()]
    );
}

#[test]
fn parses_multiple_files() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Player = require("<|module|>player")
-- file: src/player.tua
local function <|make|>makePlayer<|/make|>() end
return { makePlayer = makePlayer }
"#,
    );

    assert_eq!(fixture.files().len(), 2);
    assert_eq!(fixture.marker_path("module"), Path::new("src/main.tua"));
    assert_eq!(fixture.marker_path("make"), Path::new("src/player.tua"));
    assert_eq!(
        fixture.source("src/main.tua"),
        "local Player = require(\"player\")\n"
    );
}

#[test]
fn writes_files_to_temp_project() {
    let fixture = Fixture::parse(
        r#"-- file: tua.toml
source = "src"
-- file: src/main.tua
return {}
"#,
    );
    let temp = fixture
        .write_to_temp("fixture-write")
        .expect("fixture should write");

    assert_eq!(
        fs::read_to_string(temp.join("tua.toml")).expect("config should exist"),
        "source = \"src\"\n"
    );
    assert_eq!(
        fs::read_to_string(temp.join("src/main.tua")).expect("source should exist"),
        "return {}\n"
    );
}

#[test]
fn rejects_duplicate_markers() {
    let error = Fixture::try_parse("local <|cursor|>a = <|cursor|>1\n")
        .expect_err("duplicate markers should fail");

    assert!(error.contains("duplicate fixture marker `cursor`"));
}
