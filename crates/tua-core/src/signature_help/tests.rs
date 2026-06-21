use super::*;
use crate::source_text::position_at;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

const SOURCE: &str = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number }

local player: Player = {
  pos = { x = 0, y = 0 },
  hp = 10,
}

local function damage(target: Player, amount: number): Player
  target.hp -= amount
  return target
end

damage(player, 2)
"#;

#[test]
fn returns_known_function_signature_and_active_parameter() {
    let first = signature_at_last(SOURCE, "damage(").expect("first parameter should have help");
    let second =
        signature_at_last(SOURCE, "damage(player, ").expect("second parameter should have help");

    assert_eq!(
        first.signatures[0].label,
        "function damage(target: Player, amount: number): Player"
    );
    assert_eq!(
        parameter_labels(&first),
        vec!["target: Player", "amount: number"]
    );
    assert_eq!(first.active_parameter, 0);
    assert_eq!(second.active_parameter, 1);
}

#[test]
fn returns_signature_for_incomplete_call() {
    let source = r#"type Player = { hp: number }

local function damage(target: Player, amount: number): Player
  return target
end

damage(
"#;
    let help = signature_at_last(source, "damage(").expect("incomplete call should have help");

    assert_eq!(
        help.signatures[0].label,
        "function damage(target: Player, amount: number): Player"
    );
}

#[test]
fn returns_lua_and_love_builtin_signatures() {
    let lua =
        signature_at("local value = math.sin(\n", "math.sin(").expect("math.sin should have help");
    let love = signature_at(
        "love.graphics.print(\"hi\", \n",
        "love.graphics.print(\"hi\", ",
    )
    .expect("love.graphics.print should have help");

    assert_eq!(
        lua.signatures[0].label,
        "function math.sin(x: number): number"
    );
    assert_eq!(parameter_labels(&lua), vec!["x: number"]);
    assert_eq!(
        love.signatures[0].label,
        "function love.graphics.print(text, x?, y?)"
    );
    assert_eq!(love.active_parameter, 1);
}

#[test]
fn returns_required_module_function_signature() {
    let temp = TempProject::new("required-module-signature");
    let source = r#"local Player = require("player")

local player = Player.makePlayer(
"#;
    let player = r#"type Player = { name: string, hp: number }

local function makePlayer(name: string): Player
  return { name = name, hp = 10 }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/player.tua", player);

    let help = signature_help_tua_source(
        temp.path.join("src/main.tua"),
        source,
        position_after(source, "Player.makePlayer("),
    )
    .expect("signature help should succeed")
    .expect("module signature should resolve");

    assert_eq!(
        help.signatures[0].label,
        "function Player.makePlayer(string): Player"
    );
    assert_eq!(parameter_labels(&help), vec!["string"]);
}

#[test]
fn unknown_call_returns_none() {
    let source = "missing(\n";
    let help =
        signature_help_tua_source("src/main.tua", source, position_after(source, "missing("))
            .expect("unknown call should not fail");

    assert_eq!(help, None);
}

fn signature_at(source: &str, needle: &str) -> Option<SignatureHelp> {
    signature_help_tua_source("src/main.tua", source, position_after(source, needle))
        .expect("signature help should not fail")
}

fn signature_at_last(source: &str, needle: &str) -> Option<SignatureHelp> {
    signature_help_tua_source("src/main.tua", source, position_after_last(source, needle))
        .expect("signature help should not fail")
}

fn parameter_labels(help: &SignatureHelp) -> Vec<&str> {
    help.signatures[0]
        .parameters
        .iter()
        .map(|parameter| parameter.label.as_str())
        .collect()
}

fn position_after(source: &str, needle: &str) -> SourcePosition {
    let mut position = position_at(source, needle);

    position.column += needle.chars().count();
    position
}

fn position_after_last(source: &str, needle: &str) -> SourcePosition {
    let index = source.rfind(needle).expect("needle should exist in source");
    let before = &source[..index];
    let mut position = SourcePosition {
        line: before.bytes().filter(|byte| *byte == b'\n').count() + 1,
        column: before
            .rsplit_once('\n')
            .map(|(_, line)| line.len())
            .unwrap_or(before.len())
            + 1,
    };

    position.column += needle.chars().count();
    position
}

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "tua-core-signature-help-{name}-{}-{unique}",
            process::id()
        ));

        fs::create_dir_all(&path).expect("temp project should be created");

        Self { path }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory should be created");
        }

        fs::write(path, contents).expect("file should be written");
    }
}
