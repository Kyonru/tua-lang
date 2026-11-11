use std::fs;
use std::process::Command;

mod support;

use support::{copy_dir_all, example_root, lua_executable, tua, TempDir};

#[test]
fn behavior_tree_ecs_pool_example_runs_after_build() {
    let temp = TempDir::new("behavior-tree-ecs-pool-runtime");
    let example = example_root("features");
    fs::copy(example.join("tua.toml"), temp.path().join("tua.toml"))
        .expect("copy feature project config");
    copy_dir_all(&example.join("src"), &temp.path().join("src"))
        .expect("copy feature sources without stale build output");

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("build feature project");
    assert!(
        build.status.success(),
        "feature build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let Some(lua) = lua_executable() else {
        eprintln!("skipping feature runtime execution; no Lua interpreter available");
        return;
    };
    let runtime = Command::new(lua)
        .arg("-e")
        .arg(
            "local game = require('behavior_tree_ecs'); assert(game.shots >= 2); assert(game.flashes:activeCount() == 0)",
        )
        .env(
            "LUA_PATH",
            format!("{}/?.lua;;", temp.path().join("build").display()),
        )
        .current_dir(temp.path())
        .output()
        .expect("run generated feature Lua");
    assert!(
        runtime.status.success(),
        "generated behavior-tree/ECS/pool example failed: {}",
        String::from_utf8_lossy(&runtime.stderr)
    );
}

#[test]
fn generic_store_and_ecs_signal_payloads_agree_in_check_and_lua() {
    let temp = TempDir::new("generic-store-ecs-signal");
    fs::write(
        temp.path().join("tua.toml"),
        "source = \"src\"\nout = \"build\"\n",
    )
    .expect("write config");
    fs::create_dir_all(temp.path().join("src")).expect("create source directory");
    let source = r#"type Box<T> = { value: T }
local initial: Box<number> = { value = 2 }
local score = store({ current = initial })
local Position = ecs.component("Position", { x = "number" })
local world = ecs.world()
local entity = world:create()
world:add(entity, Position, { x = 3 })
local events = signal()
events:on("move", function(id: number, amount: number)
  local position = world:get(id, Position)
  if position ~= nil then position.x += amount end
  score:update(function(state) state.current.value += amount end)
end)
events:emit("move", entity, 5)
assert(score:get().current.value == 7)
local final = world:get(entity, Position)
assert(final ~= nil)
if final ~= nil then assert(final.x == 8) end
return score:get().current.value
"#;
    let main = temp.path().join("src/main.tua");
    fs::write(&main, source).expect("write interaction source");
    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("build interaction source");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    if let Some(lua) = lua_executable() {
        let runtime = Command::new(lua)
            .arg("-e")
            .arg("assert(dofile('build/main.lua') == 7)")
            .env(
                "LUA_PATH",
                format!("{}/?.lua;;", temp.path().join("build").display()),
            )
            .current_dir(temp.path())
            .output()
            .expect("run generated interaction Lua");
        assert!(
            runtime.status.success(),
            "generated interaction failed: {}",
            String::from_utf8_lossy(&runtime.stderr)
        );
    } else {
        eprintln!("skipping interaction runtime execution; no Lua interpreter available");
    }

    for bad_source in [
        source.replace(
            "events:emit(\"move\", entity, 5)",
            "events:emit(\"move\", entity, \"wrong\")",
        ),
        source.replace(
            "state.current.value += amount",
            "state.current.value = \"wrong\"",
        ),
    ] {
        fs::write(&main, bad_source).expect("write invalid interaction source");
        let check = tua()
            .arg("check")
            .current_dir(temp.path())
            .output()
            .expect("check invalid interaction source");
        assert!(!check.status.success(), "invalid interaction should fail");
        assert!(
            String::from_utf8_lossy(&check.stderr).contains("expected number, found string"),
            "wrong diagnostic: {}",
            String::from_utf8_lossy(&check.stderr)
        );
    }
}

#[test]
fn imported_ecs_component_get_keeps_its_field_type() {
    let temp = TempDir::new("imported-ecs-component-field");
    fs::write(
        temp.path().join("tua.toml"),
        "source = \"src\"\nout = \"build\"\n",
    )
    .expect("write config");
    fs::create_dir_all(temp.path().join("src")).expect("create source directory");
    fs::write(
        temp.path().join("src/components.tua"),
        r#"local Position = ecs.component("Position", { x = "number" })
local world = ecs.world()
return { Position = Position, world = world }
"#,
    )
    .expect("write component module");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local Shared = require("components")
local entity = Shared.world:create()
Shared.world:add(entity, Shared.Position, { x = 1 })
local position = Shared.world:get(entity, Shared.Position)
position.x = "wrong"
"#,
    )
    .expect("write consumer");

    let check = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("check project");
    assert!(
        !check.status.success(),
        "imported component payload should reject a string field assignment"
    );
    assert!(
        String::from_utf8_lossy(&check.stderr).contains("expected number, found string"),
        "expected payload mismatch, got: {}",
        String::from_utf8_lossy(&check.stderr)
    );
}
