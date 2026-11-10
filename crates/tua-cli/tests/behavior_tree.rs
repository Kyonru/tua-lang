use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod support;

#[test]
fn behavior_tree_is_typed_bundled_on_use_and_runs_independent_actors() {
    let root = std::env::temp_dir().join(format!(
        "tua-behavior-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("tua.toml"), "source = \"src\"\n").unwrap();
    fs::write(root.join("src/main.tua"), r#"
local exits = 0
local tree = BehaviorTree.new()
tree:setRoot(BehaviorTree.sequence({
  BehaviorTree.condition(function(actor) return actor.enabled end),
  BehaviorTree.wait(0.2),
  BehaviorTree.action(function(actor, dt) actor.hits = actor.hits + 1; return "success" end,
    nil, function(actor, reason) exits = exits + 1 end),
}))
local first = { enabled = true, hits = 0, seesPlayer = false }
local second = { enabled = true, hits = 0 }
local a, b = tree:instance(first), tree:instance(second)
assert(a:update(0.1) == "running")
assert(b:update(0.2) == "success" and second.hits == 1)
assert(a:update(0.1) == "success" and first.hits == 1)
assert(exits == 2)
local waiting = BehaviorTree.new(BehaviorTree.action(function() return "running" end,
  nil, function(_, reason) assert(reason == "cancel"); exits = exits + 1 end)):instance(first)
assert(waiting:update(0) == "running")
waiting:cancel()
assert(exits == 3)
local reactive = BehaviorTree.new(BehaviorTree.reactiveSelector({
  BehaviorTree.condition(function(actor) return actor.seesPlayer end),
  BehaviorTree.action(function() return "running" end, nil,
    function(_, reason) assert(reason == "interrupted"); exits = exits + 1 end),
})):instance(first)
assert(reactive:update(0) == "running")
first.seesPlayer = true
assert(reactive:update(0) == "success" and exits == 4)
local parallel = BehaviorTree.new(BehaviorTree.parallelAll({BehaviorTree.wait(0.1), BehaviorTree.wait(0.3)})):instance(first)
assert(parallel:update(0.1) == "running")
assert(parallel:update(0.2) == "success")
assert(parallel:update(0.1) == "running")
assert(parallel:update(0.2) == "success")
local any = BehaviorTree.new(BehaviorTree.parallelAny({BehaviorTree.wait(1), BehaviorTree.condition(function() return true end)})):instance(first)
assert(any:update(0) == "success")
local repeated = BehaviorTree.new(BehaviorTree.repeatN(2, BehaviorTree.action(function() return "success" end))):instance(first)
assert(repeated:update(0) == "running" and repeated:update(0) == "success")
local resettable = BehaviorTree.new(BehaviorTree.wait(0.2)):instance(first)
assert(resettable:update(0.1) == "running")
resettable:reset()
assert(resettable:update(0.1) == "running")
local shared = BehaviorTree.wait(0)
assert(not pcall(function() BehaviorTree.new(BehaviorTree.sequence({shared, shared})) end))
local limited = BehaviorTree.new(BehaviorTree.sequence({BehaviorTree.wait(0)})):instance(first)
limited.maxSteps = 1
assert(not pcall(function() limited:update(0) end))
local bad: any = "oops"
assert(not pcall(function() BehaviorTree.new(BehaviorTree.action(function() return bad end)):instance(first):update(0) end))
print("behavior-ok")
"#).unwrap();
    let build = Command::new(env!("CARGO_BIN_EXE_tua"))
        .arg("build")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(root.join("build/tuabehavior.lua").is_file());
    let emitted = fs::read_to_string(root.join("build/main.lua")).unwrap();
    assert!(emitted.contains("require(\"tuabehavior\")"));
    if let Some(lua) = support::lua_executable() {
        let run = Command::new(&lua)
            .arg("main.lua")
            .current_dir(root.join("build"))
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "behavior-ok");
        let steady = Command::new(&lua)
            .args([
                "-e",
                r#"
local BehaviorTree = require("tuabehavior")
local tree = BehaviorTree.new(BehaviorTree.reactiveSelector({
  BehaviorTree.condition(function(actor) return actor.alert end),
  BehaviorTree.action(function() return "running" end),
}))
local runners = {}
for i = 1, 100 do runners[i] = tree:instance({ alert = false }) end
for i = 1, 100 do runners[i]:update(1 / 60) end
collectgarbage("collect")
collectgarbage("stop")
local before = collectgarbage("count")
for tick = 1, 100 do
  for i = 1, 100 do runners[i]:update(1 / 60) end
end
assert(collectgarbage("count") - before < 128,
  "steady behavior updates should reuse node state")
"#,
            ])
            .current_dir(root.join("build"))
            .output()
            .unwrap();
        assert!(
            steady.status.success(),
            "{}",
            String::from_utf8_lossy(&steady.stderr)
        );
    } else {
        eprintln!("skipping emitted behavior-tree runtime checks: Lua is unavailable");
    }
    fs::write(root.join("src/main.tua"), "print(\"plain\")\n").unwrap();
    let unused = Command::new(env!("CARGO_BIN_EXE_tua"))
        .arg("build")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        unused.status.success(),
        "{}",
        String::from_utf8_lossy(&unused.stderr)
    );
    assert!(!root.join("build/tuabehavior.lua").exists());
    fs::remove_dir_all(root).unwrap();
}
