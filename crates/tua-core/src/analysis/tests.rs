use super::*;
use crate::Fixture;

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
