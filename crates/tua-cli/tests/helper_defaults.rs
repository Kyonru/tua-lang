mod support;

use std::{fs, process::Command};
use support::{tua, TempDir};
use tua_core::{CONFIG_FILE, DEFAULT_CONFIG_TEXT};

#[test]
fn nested_helper_defaults_build_and_run_with_lexical_shadows() {
    let temp = TempDir::new("nested-helper-defaults");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).unwrap();
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local function make(value: any = store({position = vec2(1, 2)}))
  return value
end
local result = make()
assert(result:get().position.x == 1)
assert(result:get().position.y == 2)
local function callbacks(handler: any = function(image) return image(2) end)
  return handler
end
assert(callbacks()(function(n) return n + 1 end) == 3)
local function builtinCallback(handler: any = function() return vec2(5, 6) end)
  return handler
end
assert(builtinCallback()().x == 5)
local function laterParameter(value: any = vec2(1, 2), vec2: any = nil)
  return value
end
assert(laterParameter(nil, function(x, y) return x + y end) == 3)
local function laterCallback(handler: any = function() return image(2) end, image: any = nil)
  return handler
end
assert(laterCallback(nil, function(n) return n + 1 end)() == 3)
final class Later {
  run(value: any = vec2(3, 4), vec2: any = nil): any { return value }
}
assert(Later.new():run(nil, function(x, y) return x + y end) == 7)
local function vec2(x: number, y: number): number return x + y end
local function shadowed(value: any = store({position = vec2(3, 4)}))
  return value
end
assert(shadowed():get().position == 7)
print("nested helper defaults passed")
"#,
    )
    .unwrap();
    for command in ["check", "build"] {
        let result = tua()
            .arg(command)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{command}: {} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert!(temp.path().join("build/brinevector.lua").is_file());
    let Some(lua) = support::lua_executable() else {
        eprintln!("skipping runtime check: Lua is not installed");
        return;
    };
    let result = Command::new(lua)
        .arg("main.lua")
        .current_dir(temp.path().join("build"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        "nested helper defaults passed"
    );
}
