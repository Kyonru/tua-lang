mod support;

use std::fs;
use std::process::Command;

use support::{lua_executable, tua, TempDir};
use tua_core::{CONFIG_FILE, DEFAULT_CONFIG_TEXT};

// Exercise the emitted program, rather than asserting on formatting of Lua text.
// These cases deliberately cross module, annotation, and lowering boundaries.
#[test]
fn emitted_language_matrix_runs_with_lua() {
    let Some(lua) = lua_executable() else {
        eprintln!("skipping emitted runtime matrix: no Lua interpreter available");
        return;
    };
    let temp = TempDir::new("emission-matrix");
    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).unwrap();
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(
        temp.path().join("src/values.tua"),
        r#"type Pair = { left: number, right: number }
enum Direction
  "left"
  "right"
end
local function pair(left: number, right: number = 4): Pair
  return { left = left, right = right }
end
local function two(): (number, number)
  return 3, 7
end
return { pair = pair, two = two }
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local Values = require("values")
local plain = require("plain")
local pair = Values.pair(2)
assert(pair.left == 2 and pair.right == 4)
assert(plain.value == "lua")

local first, second = Values.two()
assert(first == 3 and second == 7)

local state = { score = 2, nested = { count = 3 } }
state.score *= first + second
state.nested.count += 2
assert(state.score == 20 and state.nested.count == 5)

local function counter(start: number): () -> number
  local count = start
  return function(): number
    count += 1
    return count
  end
end
local nextValue = counter(4)
assert(nextValue() == 5 and nextValue() == 6)

local value: string | number = "ready"
if value is string then
  assert(value == "ready")
else
  error("string is check failed")
end
value = 9
if value is number then
  assert(value == 9)
else
  error("number is check failed")
end

local total = 0
for i = 1, 3 do
  total += i
end
assert(total == 6)
print("emission matrix passed")
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("src/plain.lua"),
        "return { value = 'lua' }\n",
    )
    .unwrap();

    for operation in ["check", "build"] {
        let output = tua()
            .arg(operation)
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{operation}:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = Command::new(lua)
        .arg("main.lua")
        .current_dir(temp.path().join("build"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "emission matrix passed"
    );
}
