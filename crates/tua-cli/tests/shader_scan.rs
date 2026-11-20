use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

mod support;

struct TempProject(PathBuf);

static NEXT_PROJECT_ID: AtomicU64 = AtomicU64::new(0);

impl TempProject {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let id = NEXT_PROJECT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tua-shader-scan-{}-{nonce}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create temporary project directory");
        fs::create_dir_all(path.join("src")).expect("create source directory");
        fs::write(path.join("tua.toml"), "source = \"src\"\n").expect("write config");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempProject {
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
                .is_ok_and(|output| output.status.success())
        })
}

fn build_and_run(temp: &TempProject, lua: &str) -> (String, String) {
    let build = Command::new(env!("CARGO_BIN_EXE_tua"))
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("build ShaderScan project");
    assert!(
        build.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let build_dir = temp.path().join("build");
    assert!(
        build_dir.is_dir(),
        "build output missing at {}; stdout: {} stderr: {}",
        build_dir.display(),
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(lua)
        .arg("main.lua")
        .current_dir(&build_dir)
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "run ShaderScan project with {lua} in {} (directory exists: {}): {error}",
                build_dir.display(),
                build_dir.is_dir()
            )
        });
    assert!(
        run.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    (
        fs::read_to_string(temp.path().join("build/main.lua")).expect("read emitted Lua"),
        String::from_utf8_lossy(&run.stdout).into_owned(),
    )
}

#[test]
fn safe_send_skips_missing_uniforms_and_warns_once() {
    let Some(lua) = lua_runtime() else {
        eprintln!("skipping emitted ShaderScan runtime test: Lua is unavailable");
        return;
    };

    let temp = TempProject::new();
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local sent = {}
local shader: any = {
  hasUniform = function(self, name) return name == "active" end,
  send = function(self, name, ...) table.insert(sent, { name, ... }) end,
}

assert(ShaderScan.safe_send(shader, "active", 2, 3))
assert(not ShaderScan.safe_send(shader, "optimized_out", 1))
assert(not ShaderScan.safe_send(shader, "optimized_out", 2))
assert(#sent == 1)
assert(sent[1][1] == "active" and sent[1][2] == 2 and sent[1][3] == 3)
"#,
    )
    .expect("write safe send source");

    let (main, output) = build_and_run(&temp, lua);
    assert!(main.contains("require(\"tuashaderscan\")"));
    assert!(temp.path().join("build/tuashaderscan.lua").is_file());
    assert_eq!(
        output
            .matches("has no active uniform \"optimized_out\"")
            .count(),
        1,
        "missing uniforms should warn once per shader and name"
    );
}

#[test]
fn shader_scan_suggests_the_built_game_path_for_a_source_root_prefix() {
    let Some(lua) = lua_runtime() else {
        eprintln!("skipping emitted ShaderScan runtime test: Lua is unavailable");
        return;
    };

    let temp = TempProject::new();
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local files = {
  ["assets/effect.glsl"] = "vec4 effect(vec4 color, Image image, vec2 uv, vec2 pixel) { return color; }",
}
_G.love = {
  filesystem = {
    read = function(path) return files[path], "not found" end,
    getInfo = function(path) if files[path] then return { type = "file" } end end,
  },
  graphics = { newShader = function(code) return { code = code } end },
}

local scanner = ShaderScan.new()
local ok, message = pcall(function()
  scanner:load_shader("wrong", "src/assets/effect.glsl")
end)
assert(not ok)
assert(tostring(message):find('"assets/effect.glsl" exists in the built game', 1, true))
assert(scanner:load_shader("right", "assets/effect.glsl"))
print("SHADER_PATHS_OK")
"#,
    )
    .expect("write ShaderScan path source");

    let (_, output) = build_and_run(&temp, lua);
    assert!(output.contains("SHADER_PATHS_OK"), "{output}");
}

#[test]
fn includes_expand_once_and_compiler_errors_map_to_the_included_file() {
    let Some(lua) = lua_runtime() else {
        eprintln!("skipping emitted ShaderScan runtime test: Lua is unavailable");
        return;
    };

    let temp = TempProject::new();
    fs::write(
        temp.path().join("src/main.tua"),
        r##"
local files = {
  ["shaders/root.glsl"] = '#include "shaders/lib/a.glsl"\nvec4 effect() { return vec4(helper()); }',
  ["shaders/lib/a.glsl"] = '#include "shaders/lib/b.glsl"\nfloat helper() { return nested(); }',
  ["shaders/lib/b.glsl"] = '#include "shaders/lib/a.glsl"\nfloat nested() { return 1.0; }',
  ["shaders/broken.glsl"] = '#include "shaders/lib/error.glsl"',
  ["shaders/lib/error.glsl"] = "BROKEN",
  ["shaders/missing.glsl"] = '#include "shaders/lib/nope.glsl"',
}

local captured = {}
local messages = {}
print = function(message) messages[#messages + 1] = message end
_G.love = {
  filesystem = { read = function(path) return files[path], "not found" end },
  graphics = {
    newShader = function(code)
      captured[#captured + 1] = code
      if code:find("BROKEN", 1, true) then
        error("Error validating pixel shader code:\nLine 1: ERROR: invalid token")
      end
      return { release = function(shader) shader.released = true end }
    end,
  },
}

local scanner = ShaderScan.new()
scanner:load_shader("water", "shaders/root.glsl")
local expanded = captured[1]
assert(expanded:find("float helper", 1, true))
assert(expanded:find("float nested", 1, true))
assert(not expanded:find("#include", 1, true))
local _, helper_count = expanded:gsub("float helper", "")
assert(helper_count == 1)

local original = scanner.s.water
files["shaders/lib/b.glsl"] = '#include "shaders/lib/a.glsl"\nfloat nested() { return 2.0; }'
scanner:update(0.3)
assert(scanner.s.water ~= original and original.released)
assert(captured[2]:find("return 2.0", 1, true))

local last_good = scanner.s.water
files["shaders/lib/b.glsl"] = '#include "shaders/lib/a.glsl"\nBROKEN'
scanner:update(0.3)
assert(scanner.s.water == last_good)
scanner:update(0.3)
assert(scanner.s.water == last_good)
local error_count = 0
for _, message in ipairs(messages) do
  if message:find("shaders/lib/b.glsl:2", 1, true) then error_count = error_count + 1 end
end
assert(error_count == 1)

files["shaders/lib/b.glsl"] = '#include "shaders/lib/a.glsl"\nfloat nested() { return 3.0; }'
scanner:update(0.3)
assert(scanner.s.water ~= last_good)
assert(captured[4]:find("return 3.0", 1, true))

local after_fix = scanner.s.water
files["shaders/root.glsl"] = '#include "shaders/lib/new.glsl"\nvec4 effect() { return vec4(helper()); }'
scanner:update(0.3)
assert(scanner.s.water == after_fix)
files["shaders/lib/new.glsl"] = "float helper() { return 4.0; }"
scanner:update(0.3)
assert(scanner.s.water ~= after_fix)

local ok, compile_error = pcall(function()
  scanner:load_shader("broken", "shaders/broken.glsl")
end)
assert(not ok and tostring(compile_error):find("shaders/lib/error.glsl:1", 1, true))
local missing_ok, missing_error = pcall(function()
  scanner:load_shader("missing", "shaders/missing.glsl")
end)
assert(not missing_ok and tostring(missing_error):find("shaders/missing.glsl:1", 1, true))
assert(scanner.s.broken == nil and scanner.s.missing == nil)
files["shaders/lib/error.glsl"] = "float repaired() { return 1.0; }"
files["shaders/lib/nope.glsl"] = "float included() { return 1.0; }"
scanner:update(0.3)
assert(scanner.s.broken and scanner.s.missing)
assert(scanner.loaded.broken.failed == nil and scanner.loaded.missing.failed == nil)
"##,
    )
    .expect("write include expansion source");

    let (_, output) = build_and_run(&temp, lua);
    assert!(output.is_empty());
}

#[test]
fn compiled_callback_precedes_publication_and_failed_setup_keeps_last_shader() {
    let Some(lua) = lua_runtime() else {
        eprintln!("skipping emitted ShaderScan runtime test: Lua is unavailable");
        return;
    };
    let temp = TempProject::new();
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local files = { ["shaders/effect.glsl"] = "FIRST" }
local messages = {}
print = function(message) messages[#messages + 1] = message end
_G.love = {
  filesystem = { read = function(path) return files[path], "not found" end },
  graphics = { newShader = function(code)
    if code:find("BAD", 1, true) then error("Line 1: compile failure") end
    return {
      code = code,
      send = function(self, name, value) self[name] = value end,
      release = function(self) self.released = true end,
    }
  end },
}

local scanner = ShaderScan.new()
local fail_setup = true
local callbacks = 0
scanner.on_compiled = function(name: string, path: string, shader: Shader, previous: Shader | nil)
  callbacks = callbacks + 1
  assert(name == "effect" and path == "shaders/effect.glsl")
  assert(scanner.s.effect == previous)
  if previous then assert(not previous.released) end
  if fail_setup then error("uniform setup failed") end
  shader:send("gain", callbacks)
end

local ok, message = pcall(function() scanner:load_shader("effect", "shaders/effect.glsl") end)
assert(not ok and tostring(message):find("uniform setup failed", 1, true))
assert(scanner.s.effect == nil)
assert(callbacks == 1)
fail_setup = false
files["shaders/effect.glsl"] = "SECOND"
scanner:update(0.3)
local first = scanner.s.effect
assert(first and first.gain == 2)
fail_setup = true
files["shaders/effect.glsl"] = "THIRD"
scanner:update(0.3)
assert(scanner.s.effect == first and not first.released)
assert(callbacks == 3)
scanner:update(0.3)
assert(callbacks == 3)
fail_setup = false
files["shaders/effect.glsl"] = "BAD"
scanner:update(0.3)
assert(scanner.s.effect == first and not first.released)
files["shaders/effect.glsl"] = "FOURTH"
scanner:update(0.3)
assert(scanner.s.effect ~= first and first.released)
local final = scanner.s.effect
assert(final and final.gain == 4)
local setup_errors = 0
for _, entry in ipairs(messages) do
  if entry:find("uniform setup failed", 1, true) then setup_errors = setup_errors + 1 end
end
assert(setup_errors == 1)
print("CALLBACK_OK")
"#,
    )
    .expect("write callback source");

    let (_, output) = build_and_run(&temp, lua);
    assert!(output.is_empty(), "{output}");
}

#[test]
fn preload_reports_each_result_and_cancel_keeps_completed_shaders() {
    let Some(lua) = lua_runtime() else {
        eprintln!("skipping emitted ShaderScan runtime test: Lua is unavailable");
        return;
    };
    let temp = TempProject::new();
    fs::write(
        temp.path().join("src/main.tua"),
        r##"
local files = {
  ["shaders/shared.glsl"] = "float helper() { return 1.0; }",
  ["shaders/one.glsl"] = '#include "shaders/shared.glsl"\nvec4 effect() { return vec4(helper()); }',
  ["shaders/bad.glsl"] = '#include "shaders/shared.glsl"\nBAD',
  ["shaders/two.glsl"] = '#include "shaders/shared.glsl"\nvec4 effect() { return vec4(helper() * 2.0); }',
}

local compilations = 0
local notices = {}
print = function(message) notices[#notices + 1] = message end
_G.love = {
  filesystem = { read = function(path) return files[path], "not found" end },
  graphics = { newShader = function(code)
    compilations = compilations + 1
    if code:find("BAD", 1, true) then error("Line 2: compile failure") end
    return { code = code }
  end },
}

local scanner = ShaderScan.new()
local activated = 0
scanner.on_compiled = function(name: string, path: string, shader: Shader, previous: Shader | nil)
  activated = activated + 1
end
local progress = {}
local completions = 0
local job = scanner:preload({
  { name = "one", path = "shaders/one.glsl" },
  { name = "bad", path = "shaders/bad.glsl" },
  { name = "two", path = "shaders/two.glsl" },
}, function(state)
  progress[#progress + 1] = { state.completed, state.successful, state.failed, state.current_name }
end, function(state)
  completions = completions + 1
  assert(state.done and not state.cancelled)
end)
assert(job.total == 3 and job.completed == 0 and not job.done)
assert(job.current_name == "one" and job.current_path == "shaders/one.glsl")
assert(scanner.s.one == nil and compilations == 0)
local concurrent_ok = pcall(function() scanner:preload({}) end)
assert(not concurrent_ok)
scanner:update(0)
local first = scanner.s.one
assert(first and first.code:find("float helper", 1, true))
assert(job.completed == 1 and job.successful == 1 and job.failed == 0)
assert(compilations == 1 and completions == 0)
scanner:update(0)
assert(job.completed == 2 and job.successful == 1 and job.failed == 1)
assert(scanner.s.bad == nil and #job.errors == 1)
assert(job.errors[1].name == "bad" and job.errors[1].path == "shaders/bad.glsl")
assert(job.errors[1].message:find("shaders/bad.glsl:2", 1, true))
scanner:update(0)
assert(job.done and not job.cancelled and completions == 1)
assert(job.completed == 3 and job.successful == 2 and job.failed == 1)
assert(scanner.s.one == first and scanner.s.two ~= nil and compilations == 3 and activated == 2)
assert(progress[1][1] == 1 and progress[1][2] == 1 and progress[1][3] == 0 and progress[1][4] == "one")
assert(progress[2][1] == 2 and progress[2][3] == 1 and progress[2][4] == "bad")
assert(progress[3][1] == 3 and progress[3][2] == 2 and progress[3][4] == "two")
job:cancel()
assert(completions == 1)
local duplicate_ok, duplicate_error = pcall(function()
  scanner:preload({ { name = "same", path = "shaders/one.glsl" }, { name = "same", path = "shaders/two.glsl" } })
end)
assert(not duplicate_ok and tostring(duplicate_error):find("occurs twice", 1, true))

local cancelled_completions = 0
local second = scanner:preload({
  { name = "new", path = "shaders/one.glsl" },
  { name = "later", path = "shaders/two.glsl" },
}, function() error("progress display failed") end, function(state)
  cancelled_completions = cancelled_completions + 1
  assert(state.done and state.cancelled)
end)
scanner:update(0)
assert(second.completed == 1 and scanner.s.new ~= nil)
second:cancel()
second:cancel()
scanner:update(0)
assert(second.done and second.cancelled and second.completed == 1)
assert(second.successful == 1 and second.failed == 0 and second.total == 2)
assert(cancelled_completions == 1 and scanner.s.later == nil and compilations == 4 and activated == 3)
assert(#notices == 1 and notices[1]:find("progress display failed", 1, true))

local empty_completions = 0
local empty = scanner:preload({}, nil, function(state)
  empty_completions = empty_completions + 1
  assert(state.done and not state.cancelled)
end)
assert(empty.done and empty.total == 0 and empty_completions == 1)

local cancel_during_compile = function() end
scanner.on_compiled = function(name: string, path: string, shader: Shader, previous: Shader | nil)
  if name == "during" then cancel_during_compile() end
end
local during_completions = 0
local during = scanner:preload({
  { name = "during", path = "shaders/one.glsl" },
  { name = "skipped", path = "shaders/two.glsl" },
}, nil, function(state)
  during_completions = during_completions + 1
  assert(state.completed == 1 and state.successful == 1 and scanner.s.during ~= nil)
end)
cancel_during_compile = function() during:cancel() end
scanner:update(0)
assert(during.done and during.cancelled and during.completed == 1)
assert(during_completions == 1 and scanner.s.skipped == nil)

local final_completions = 0
local final = scanner:preload({ { name = "last", path = "shaders/one.glsl" } }, function(state)
  state:cancel()
end, function(state)
  final_completions = final_completions + 1
  assert(state.done and not state.cancelled and state.completed == state.total)
end)
scanner:update(0)
assert(final.done and not final.cancelled and final_completions == 1)
"##,
    )
    .expect("write preload source");

    let (_, output) = build_and_run(&temp, lua);
    assert!(output.is_empty(), "{output}");
}

#[test]
fn shader_scan_preloads_with_real_love() {
    let Some(love) = support::love_executable() else {
        eprintln!("skipping ShaderScan LÖVE smoke; set TUA_TEST_LOVE_RUNTIME=1");
        return;
    };
    let temp = TempProject::new();
    let assets = temp.path().join("src/assets");
    fs::create_dir_all(&assets).expect("create shader assets");
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/features/src/assets");
    for name in ["solid.glsl", "wave.glsl"] {
        fs::copy(examples.join(name), assets.join(name)).expect("copy shader example asset");
    }
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local scanner = ShaderScan.new()
local compiled = 0
local draws = 0
scanner.on_compiled = function(name: string, path: string, replacement: Shader, previous: Shader | nil)
  compiled += 1
  if name == "wave" then replacement:send("time", 0) end
end
local job = scanner:preload({
  { name = "solid", path = "assets/solid.glsl" },
  { name = "wave", path = "assets/wave.glsl" },
})

function love.update(dt)
  scanner:update(dt)
  if job.done then
    assert(job.completed == 2 and job.successful == 2 and job.failed == 0)
    assert(compiled == 2 and draws >= 1)
    assert(scanner.s.solid and scanner.s.wave)
    love.event.quit(0)
  end
end

function love.draw()
  draws += 1
  local solid = scanner.s.solid
  if solid then
    love.graphics.setShader(solid)
    love.graphics.rectangle("fill", 0, 0, 8, 8)
  end
  local wave = scanner.s.wave
  if wave then
    love.graphics.setShader(wave)
    love.graphics.rectangle("fill", 8, 8, 8, 8)
  end
  love.graphics.setShader()
end
"#,
    )
    .expect("write LÖVE preload source");
    let build = Command::new(env!("CARGO_BIN_EXE_tua"))
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("build LÖVE preload project");
    assert!(
        build.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let child = Command::new(love)
        .arg(temp.path().join("build"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch LÖVE preload project");
    let output = support::wait_for_child_output(child, Duration::from_secs(10));
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
