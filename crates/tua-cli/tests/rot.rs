use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

mod support;

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("tua-rot-{}-{nonce}-{sequence}", std::process::id()));
        fs::create_dir_all(path.join("src")).expect("create source directory");
        fs::write(path.join("tua.toml"), "source = \"src\"\n").expect("write config");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn run(&self, command: &str) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_tua"))
            .arg(command)
            .current_dir(self.path())
            .output()
            .expect("run Tua CLI")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn lua_runtime() -> Option<&'static str> {
    ["luajit", "lua5.1", "lua", "lua5.4", "lua5.3", "lua5.2"]
        .into_iter()
        .find(|binary| {
            Command::new(binary)
                .arg("-v")
                .output()
                .is_ok_and(|out| out.status.success())
        })
}

#[test]
fn roguelike_helpers_are_typed_bundled_only_when_used_and_run_on_lua() {
    let Some(lua) = lua_runtime() else {
        eprintln!("skipping emitted Rot runtime test: Lua is unavailable");
        return;
    };
    let project = Project::new();
    fs::write(
        project.path().join("src/main.tua"),
        r##"
local rng = Rot.Rng.new(817)
local same = Rot.Rng.new(817)
assert(rng:int(1, 100) == same:int(1, 100))
local count, rolls = Rot.Dice.roll("3d6+2", rng)
assert(count >= 5 and count <= 20 and #rolls == 3)
assert(not pcall(function()
  Rot.Dice.roll("1d6+" .. string.rep("9", 400), rng)
end))

local maze = Rot.Map.generate(31, 21, "sidewinder", rng)
assert(not maze:get(1, 1) and maze:get(2, 2))
local path = Rot.Path.astar(maze, 2, 2, 30, 20)
assert(path and path[1].x == 2 and path[#path].x == 30)
for seed = 1, 20 do
  local mapRng = Rot.Rng.new(seed)
  for _, algorithm in ipairs({ "binaryTree", "sidewinder" }) do
    local generated = Rot.Map.generate(13, 11, algorithm, mapRng)
    assert(Rot.Path.astar(generated, 2, 2, 12, 10))
  end
end
local disconnected = Rot.Map.arena(2, 2)
disconnected:set(1, 1, true)
disconnected:set(2, 2, true)
local noPath = Rot.Path.dijkstra(disconnected, 1, 1, 2, 2)
assert(noPath == nil)

local visible = Rot.FOV.compute(7, 5, 2, 3, 4, function(x, y)
  return x == 3 and y == 3
end)
assert(visible:get(3, 2) and visible:get(3, 3) and not visible:get(4, 3))

local color = Rot.Color.parse("#33AAFF88")
assert(color.r > 0.18 and color.r <= 0.2 and color.a > 0.53 and color.a < 0.54)
local short = Rot.Color.parse("#3af")
assert(short.r == color.r and short.g > short.r)
local mixed = Rot.Color.mix(color, Rot.Color.parse("white"), 0.5)
assert(mixed.r > color.r)

local noise = Rot.Noise.new(22)
local noise2 = Rot.Noise.new(22)
assert(noise:sample(0.25, 0.75) == noise2:sample(0.25, 0.75))
local phrase = Rot.StringGenerator.generate({ root = { "a {creature}" }, creature = { "red slime", "blue bat" } }, "root", rng)
assert(phrase == "a red slime" or phrase == "a blue bat")
local light = Rot.Lighting.compute(3, 3, { { x = 2, y = 2, radius = 2, color = "#ff0000" } })
assert(light:get(2, 2).r == 1 and light:get(2, 2).g == 0)
local transparent = Rot.Lighting.compute(1, 1, { { x = 1, y = 1, radius = 0, color = "#ff000000" } })
assert(transparent:get(1, 1).r == 0)
print("rot-ok")
"##,
    )
    .expect("write source");

    let build = project.run("build");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = fs::read_to_string(project.path().join("build/main.lua")).unwrap();
    assert!(output.contains("require(\"tuarot\")"), "{output}");
    assert!(project.path().join("build/tuarot.lua").is_file());
    let run = Command::new(lua)
        .arg("main.lua")
        .current_dir(project.path().join("build"))
        .output()
        .expect("run built game helpers");
    assert!(
        run.status.success(),
        "generated source:\n{}\nstderr:\n{}",
        fs::read_to_string(project.path().join("build/main.lua")).unwrap(),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "rot-ok");
}

#[test]
fn unused_roguelike_runtime_is_not_copied() {
    let project = Project::new();
    fs::write(project.path().join("src/main.tua"), "print(\"plain\")\n").unwrap();
    let build = project.run("build");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(!project.path().join("build/tuarot.lua").exists());
    assert!(!fs::read_to_string(project.path().join("build/main.lua"))
        .unwrap()
        .contains("tuarot"));
}

#[test]
fn rng_collections_keep_imported_payloads_and_runtime_identity() {
    let project = Project::new();
    fs::write(
        project.path().join("src/items.tua"),
        r#"export type Item = { id: string }
local items: Item[] = { { id = "rifle" }, { id = "cannon" } }
return { items = items }
"#,
    )
    .unwrap();
    let source = r#"local module = require("items")
local items = module.items
local rng = Rot.Rng.new(42)
local selected = rng:choice(items)
local _id: string = selected.id
local shuffled = rng:shuffle(items)
local _other: string = shuffled[1].id
assert(selected == items[1] or selected == items[2])
assert(shuffled ~= items and #shuffled == #items)
assert(items[1].id == "rifle" and items[2].id == "cannon")
assert(shuffled[1] ~= shuffled[2])
local empty: string[] = {}
assert(not pcall(function() rng:choice(empty) end))
assert(#rng:shuffle(empty) == 0)
"#;
    fs::write(project.path().join("src/main.tua"), source).unwrap();
    let build = project.run("build");
    assert!(
        build.status.success(),
        "{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    if let Some(lua) = lua_runtime() {
        let output = Command::new(lua)
            .arg("main.lua")
            .current_dir(project.path().join("build"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::write(
        project.path().join("src/main.tua"),
        source
            .replace("local _id: string", "local _id: number")
            .replace("local _other: string", "local _other: number"),
    )
    .unwrap();
    let check = project.run("check");
    assert!(!check.status.success());
    assert!(
        String::from_utf8_lossy(&check.stderr).contains("expected number, found string"),
        "{check:?}"
    );
}

#[test]
fn named_rng_streams_keep_legacy_sequences_and_run_on_emitted_lua() {
    let project = Project::new();
    let source = include_str!("fixtures/rot_stream.tua");
    fs::write(project.path().join("src/main.tua"), source).unwrap();
    let build = project.run("build");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    if let Some(lua) = lua_runtime() {
        let output = Command::new(lua)
            .arg("main.lua")
            .current_dir(project.path().join("build"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "rot-stream-ok"
        );
    }
    if let Some(love) = support::love_executable() {
        fs::rename(
            project.path().join("build/main.lua"),
            project.path().join("build/probe.lua"),
        )
        .unwrap();
        fs::write(
            project.path().join("build/conf.lua"),
            r#"function love.conf(t)
  t.window = false
  t.modules.audio = false
  t.modules.graphics = false
  t.modules.joystick = false
end
"#,
        )
        .unwrap();
        fs::write(
            project.path().join("build/main.lua"),
            r#"function love.load()
  for _, enabled in ipairs({ false, true }) do
    if enabled then jit.on() else jit.off() end
    print("ROT_RUNTIME", love.getVersion(), jit.version, jit.status())
    love.filesystem.load("probe.lua")()
  end
  love.event.quit(0)
end
"#,
        )
        .unwrap();
        let child = Command::new(love)
            .arg(project.path().join("build"))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let output = support::wait_for_child_output(child, std::time::Duration::from_secs(10));
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.matches("rot-stream-ok").count(), 2, "{stdout}");
        eprintln!("{stdout}");
    }
    for source in [
        "local _rng = Rot.Rng.stream(817, 12)\n",
        "local _rng = Rot.Rng.stream(\"817\", \"rewards\")\n",
        "local _rng = Rot.Rng.stream(817)\n",
    ] {
        fs::write(project.path().join("src/main.tua"), source).unwrap();
        assert!(!project.run("check").status.success(), "accepted {source}");
    }
}
