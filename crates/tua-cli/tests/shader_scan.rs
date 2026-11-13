use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

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
