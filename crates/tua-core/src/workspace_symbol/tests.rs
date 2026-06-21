use super::*;
use std::env;
use std::fs;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn scans_configured_source_tree_for_modules_aliases_and_functions() {
    let temp = TempProject::new("workspace-symbols-scan");

    temp.write("tua.toml", "source = \"game\"\nextension = \"tua\"\n");
    temp.write(
        "game/player.tua",
        r#"type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 10 }
end
"#,
    );
    temp.write(
        "game/systems/combat.tua",
        r#"type CombatEvent = { amount: number }

local function damage(player: Player, amount: number): Player
  return player
end
"#,
    );

    let symbols = workspace_symbols_from_dir(temp.path(), "").expect("workspace should index");

    assert_eq!(
        symbol_names(&symbols),
        vec![
            "player",
            "Player",
            "makePlayer",
            "systems.combat",
            "CombatEvent",
            "damage"
        ]
    );
    assert_symbol(&symbols, "player", WorkspaceSymbolKind::Module, None);
    assert_symbol(
        &symbols,
        "Player",
        WorkspaceSymbolKind::TypeAlias,
        Some("player"),
    );
    assert_symbol(
        &symbols,
        "damage",
        WorkspaceSymbolKind::Function,
        Some("systems.combat"),
    );
}

#[test]
fn fuzzy_matches_symbol_names_and_qualified_module_names() {
    let temp = TempProject::new("workspace-symbols-fuzzy");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", "type Player = { hp: number }\n");
    temp.write(
        "src/systems/combat.tua",
        "local function damage(amount: number): number\n  return amount\nend\n",
    );

    let player_matches =
        workspace_symbols_from_dir(temp.path(), "plr").expect("workspace should index");
    let combat_matches =
        workspace_symbols_from_dir(temp.path(), "syscmb").expect("workspace should index");

    assert_eq!(symbol_names(&player_matches), vec!["player", "Player"]);
    assert_eq!(
        symbol_names(&combat_matches),
        vec!["systems.combat", "damage"]
    );
}

#[test]
fn honors_custom_extension() {
    let temp = TempProject::new("workspace-symbols-extension");

    temp.write("tua.toml", "source = \"src\"\nextension = \"blua\"\n");
    temp.write("src/player.blua", "type Player = { hp: number }\n");
    temp.write("src/ignored.tua", "type Ignored = { hp: number }\n");

    let symbols = workspace_symbols_from_dir(temp.path(), "").expect("workspace should index");

    assert_eq!(symbol_names(&symbols), vec!["player", "Player"]);
}

fn assert_symbol(
    symbols: &[WorkspaceSymbol],
    name: &str,
    kind: WorkspaceSymbolKind,
    container: Option<&str>,
) {
    let symbol = symbols
        .iter()
        .find(|symbol| symbol.name == name && symbol.kind == kind)
        .unwrap_or_else(|| panic!("expected {kind:?} symbol `{name}`"));

    assert_eq!(symbol.container_name.as_deref(), container);
}

fn symbol_names(symbols: &[WorkspaceSymbol]) -> Vec<&str> {
    symbols.iter().map(|symbol| symbol.name.as_str()).collect()
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
        let path = env::temp_dir().join(format!("tua-{name}-{}-{unique}", process::id()));

        fs::create_dir_all(&path).expect("temp project should be created");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory should be created");
        }

        fs::write(path, contents).expect("fixture should be written");
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
