use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempProject(PathBuf);

impl TempProject {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("tua-shader-scan-{}-{nonce}", std::process::id()));
        fs::create_dir_all(path.join("src")).expect("create source directory");
        fs::write(
            path.join("tua.toml"),
            "source = \"src\"\n",
        )
        .expect("write project config");
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

#[test]
fn safe_send_skips_missing_uniforms_and_warns_once() {
    let lua = Command::new("lua")
        .arg("-v")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|_| "lua");
    let Some(lua) = lua else {
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
    let main = fs::read_to_string(temp.path().join("build/main.lua")).expect("read emitted Lua");
    assert!(main.contains("require(\"tuashaderscan\")"));
    assert!(temp.path().join("build/tuashaderscan.lua").is_file());

    let run = Command::new(lua)
        .arg("main.lua")
        .current_dir(temp.path().join("build"))
        .output()
        .expect("run ShaderScan safe send project");
    assert!(
        run.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout)
            .matches("has no active uniform \"optimized_out\"")
            .count(),
        1,
        "missing uniforms should warn once per shader and name"
    );
}
