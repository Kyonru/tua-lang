//! Cross-process cache regressions. Compare behavior and writes, never timings.
use std::fs::{self, File, FileTimes};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct Project(PathBuf);

impl Project {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let project = Self(std::env::temp_dir().join(format!(
            "tua-build-cache-{name}-{}-{nonce}",
            std::process::id()
        )));
        project.write("tua.toml", "source = \"src\"\nout = \"build\"\n");
        project.write("src/main.tua", "print(42)\n");
        project
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn build(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tua"))
            .arg("build")
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }

    fn success(&self, args: &[&str]) -> String {
        let output = self.build(args);
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn warm(&self) -> String {
        // A fresh output tree is itself a module/asset-resolution input. Check
        // it once before expecting an unchanged-build hit.
        assert!(self.success(&[]).contains("built "));
        assert!(self.success(&[]).contains("built "));
        let cached = self.success(&[]);
        assert!(cached.contains("up to date"), "{cached}");
        cached
    }

    fn cache(&self) -> PathBuf {
        self.0.join(".tua/cache/build/last-success.json")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

#[test]
fn unchanged_process_reuses_outputs_and_replays_warnings() {
    let p = Project::new("warnings");
    p.write("src/main.tua", "local unused = 1\nprint(42)\n");
    p.write(
        "tua.toml",
        "source = \"src\"\n[diagnostics]\nunused_locals = true\n",
    );
    let cached = p.warm();
    assert!(cached.contains("unused"), "{cached}");
    let path = p.0.join("build/main.lua");
    let timestamp = modified(&path);
    let contents = fs::read(&path).unwrap();
    let result = p.success(&[]);
    assert_eq!(result, cached);
    assert_eq!(modified(&path), timestamp);
    assert_eq!(fs::read(&path).unwrap(), contents);
    let forced = p.success(&["--no-cache"]);
    assert!(forced.contains("built "));
    assert_eq!(forced.lines().next(), cached.lines().next());
    assert_eq!(fs::read(path).unwrap(), contents);
    assert!(p.success(&["--emit-on-error"]).contains("built "));
}

#[test]
fn dependency_content_change_with_preserved_size_and_mtime_is_rechecked() {
    let p = Project::new("dependency");
    p.write("src/value.tua", "return { count = 123 }\n");
    p.write(
        "src/main.tua",
        "local value = require(\"value\")\nlocal count: number = value.count\nprint(count)\n",
    );
    p.warm();
    let path = p.0.join("src/value.tua");
    let time = modified(&path);
    p.write("src/value.tua", "return { count = 'x' }\n");
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(time))
        .unwrap();
    let failed = p.build(&[]);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("main.tua"));
    assert!(
        !p.build(&[]).status.success(),
        "failed builds must not become hits"
    );
}

#[test]
fn deleted_modified_and_new_outputs_invalidate_cached_success() {
    let p = Project::new("outputs");
    p.write("src/data.txt", "original");
    p.warm();
    for name in ["build/main.lua", "build/main.lua.map", "build/data.txt"] {
        let contents = fs::read(p.0.join(name)).unwrap();
        fs::remove_file(p.0.join(name)).unwrap();
        assert!(p.success(&[]).contains("built "));
        assert_eq!(fs::read(p.0.join(name)).unwrap(), contents);
        p.success(&[]);
    }
    p.write("build/main.lua", "manual corruption");
    assert!(p.success(&[]).contains("built "));
    assert!(!fs::read_to_string(p.0.join("build/main.lua"))
        .unwrap()
        .contains("corruption"));
    // Reintroducing exactly the corrupt bytes must not match a snapshot saved
    // before the repair and bypass the next repair.
    p.write("build/main.lua", "manual corruption");
    assert!(p.success(&[]).contains("built "));
    assert!(!fs::read_to_string(p.0.join("build/main.lua"))
        .unwrap()
        .contains("corruption"));
    p.success(&[]);
    p.write("build/user.lua", "return 1\n");
    assert!(p.success(&[]).contains("built "));
    assert_eq!(
        fs::read_to_string(p.0.join("build/user.lua")).unwrap(),
        "return 1\n"
    );
}

#[test]
fn source_inventory_configuration_assets_and_cache_corruption_invalidate() {
    let p = Project::new("invalidation");
    p.warm();
    p.write("src/extra.tua", "return 1\n");
    assert!(p.success(&[]).contains("built "));
    assert!(p.0.join("build/extra.lua").exists());
    p.success(&[]);
    fs::remove_file(p.0.join("src/extra.tua")).unwrap();
    assert!(p.success(&[]).contains("built "));
    assert!(!p.0.join("build/extra.lua").exists());
    p.success(&[]);
    p.write("src/data.txt", "new asset");
    assert!(p.success(&[]).contains("built "));
    assert_eq!(
        fs::read_to_string(p.0.join("build/data.txt")).unwrap(),
        "new asset"
    );
    p.write("tua.toml", "source = \"src\"\nemit_maps = false\n");
    assert!(p.success(&[]).contains("built "));
    assert!(!p.0.join("build/main.lua.map").exists());
    p.success(&[]);
    fs::write(p.cache(), "not JSON").unwrap();
    assert!(p.success(&[]).contains("built "));
    let mut record: serde_json::Value =
        serde_json::from_slice(&fs::read(p.cache()).unwrap()).unwrap();
    record["identity"] = "different compiler".into();
    fs::write(p.cache(), serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(p.success(&[]).contains("built "));
}

#[test]
fn removing_empty_source_directory_cannot_reuse_success() {
    let p = Project::new("empty-source");
    fs::remove_file(p.0.join("src/main.tua")).unwrap();
    p.warm();
    fs::remove_dir(p.0.join("src")).unwrap();
    assert!(!p.build(&[]).status.success());
}

#[cfg(unix)]
#[test]
fn symlinked_outputs_and_cache_do_not_bypass_guards() {
    use std::os::unix::fs::symlink;
    let p = Project::new("symlinks");
    p.warm();
    p.write("outside.txt", "keep");
    let target = p.0.join("build/main.lua");
    fs::remove_file(&target).unwrap();
    symlink(p.0.join("outside.txt"), &target).unwrap();
    assert!(!p.build(&[]).status.success());
    assert_eq!(fs::read_to_string(p.0.join("outside.txt")).unwrap(), "keep");
    fs::remove_file(target).unwrap();
    fs::remove_file(p.cache()).unwrap();
    symlink(p.0.join("outside.txt"), p.cache()).unwrap();
    assert!(p.success(&[]).contains("built "));
    assert_eq!(fs::read_to_string(p.0.join("outside.txt")).unwrap(), "keep");
}

#[test]
fn external_type_libraries_and_output_fallback_modules_are_inputs() {
    let p = Project::new("external-library");
    let library = Project::new("library-root");
    library.write("types/api.lua", "---@meta api\nreturn { count = 123 }\n");
    p.write("tua.toml", &format!(
        "source = \"src\"\n[[type_libraries]]\npath = {:?}\ndeclaration_prefix = \"api\"\nruntime_prefix = \"api\"\n",
        library.0.join("types").to_str().unwrap()
    ));
    p.write(
        "src/main.tua",
        "local api = require(\"api\")\nlocal count: number = api.count\nprint(count)\n",
    );
    p.warm();
    library.write("types/api.lua", "---@meta api\nreturn { count = 'x' }\n");
    assert!(!p.build(&[]).status.success());

    let p = Project::new("output-fallback");
    p.write("build/external.lua", "return { count = 123 }\n");
    p.write(
        "src/main.tua",
        "local api = require(\"external\")\nlocal count: number = api.count\nprint(count)\n",
    );
    p.warm();
    p.write("build/external.lua", "return { count = 'x' }\n");
    assert!(!p.build(&[]).status.success());
}

#[test]
fn inaccessible_cache_and_in_place_builds_keep_normal_build_behavior() {
    let p = Project::new("unavailable-cache");
    p.write(".tua", "not a directory");
    assert!(p.success(&[]).contains("built "));
    assert!(p.success(&[]).contains("built "));

    let p = Project::new("in-place");
    p.write("tua.toml", "source = \"src\"\nout = \"src\"\n");
    assert!(p.success(&[]).contains("built "));
    assert!(p.success(&[]).contains("built "));
    assert!(!p.cache().exists());
}

#[cfg(unix)]
#[test]
fn asset_pipeline_runs_before_cache_lookup_and_new_errors_are_not_hidden() {
    let p = Project::new("pipeline");
    p.write("assets/input.txt", "first");
    p.write(
        "tools/import.sh",
        r#"set -eu
mkdir -p "$TUA_ASSET_PIPELINE_OUTPUT_DIR"
cat "$TUA_ASSET_PIPELINE_PROJECT_ROOT/assets/input.txt" > "$TUA_ASSET_PIPELINE_OUTPUT_DIR/data.txt"
printf '{"version":1,"outputs":["data.txt"],"diagnostics":[]}\n' > "$TUA_ASSET_PIPELINE_MANIFEST"
"#,
    );
    p.write(
        "tua.toml",
        r#"source = "src"
[[asset_pipelines]]
name = "data"
inputs = ["assets/*.txt", "tools/import.sh"]
command = ["sh", "tools/import.sh"]
output = "src/generated"
version = "1"
"#,
    );
    p.warm();
    p.write("assets/input.txt", "second");
    assert!(p.success(&[]).contains("built "));
    assert_eq!(
        fs::read_to_string(p.0.join("build/generated/data.txt")).unwrap(),
        "second"
    );
    p.success(&[]);
    assert!(p.success(&[]).contains("up to date"));
    p.write("tools/import.sh", "exit 1\n");
    assert!(!p.build(&[]).status.success());
}

#[test]
fn cache_neighbors_and_nested_output_remain_fingerprinted() {
    let p = Project::new("cache-neighbors");
    p.write(
        "tua.toml",
        r#"source = "src"
out = ".tua/cache/build/output"
"#,
    );
    p.warm();
    p.write(".tua/cache/build/input.lua", "return 1\n");
    assert!(p.success(&[]).contains("built "));
    assert!(p.success(&[]).contains("up to date"));
    p.write(".tua/cache/build/input.lua", "return 2\n");
    assert!(p.success(&[]).contains("built "));
    p.write(".tua/cache/build/output/main.lua", "broken");
    assert!(p.success(&[]).contains("built "));
    assert_ne!(
        fs::read_to_string(p.0.join(".tua/cache/build/output/main.lua")).unwrap(),
        "broken"
    );
}

#[test]
fn edited_bundled_runtime_keeps_existing_ownership_semantics() {
    let p = Project::new("custom-runtime");
    p.write(
        "src/main.tua",
        "local world = ECS.world()\nworld:create()\n",
    );
    p.warm();
    let custom = "return { custom = true }\n";
    p.write("build/tuaecs.lua", custom);
    assert!(p.success(&[]).contains("built "));
    assert_eq!(
        fs::read_to_string(p.0.join("build/tuaecs.lua")).unwrap(),
        custom
    );
    p.success(&[]);
    assert!(p.success(&[]).contains("up to date"));
    p.write("src/main.tua", "print(1)\n");
    p.success(&[]);
    assert_eq!(
        fs::read_to_string(p.0.join("build/tuaecs.lua")).unwrap(),
        custom
    );
}

#[cfg(unix)]
#[test]
fn source_asset_permissions_are_copied_after_a_warm_build() {
    use std::os::unix::fs::PermissionsExt;
    let p = Project::new("permissions");
    p.write("src/tool.sh", "#!/bin/sh\nexit 0\n");
    fs::set_permissions(p.0.join("src/tool.sh"), fs::Permissions::from_mode(0o644)).unwrap();
    p.warm();
    fs::set_permissions(p.0.join("src/tool.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(p.success(&[]).contains("built "));
    assert_eq!(
        fs::metadata(p.0.join("build/tool.sh"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
}

#[test]
fn live_editor_completion_statistics_do_not_invalidate_builds() {
    let p = Project::new("completion-statistics");
    p.warm();
    let output = p.0.join("build/main.lua");
    let timestamp = modified(&output);
    p.write(
        ".tua/completion-stats.json",
        r#"{"version":1,"updated_at_epoch":1,"entries":[]}"#,
    );
    assert!(p.success(&[]).contains("up to date"));
    p.write(
        ".tua/completion-stats.json",
        r#"{"version":1,"updated_at_epoch":2,"entries":[]}"#,
    );
    assert!(p.success(&[]).contains("up to date"));
    fs::remove_file(p.0.join(".tua/completion-stats.json")).unwrap();
    assert!(p.success(&[]).contains("up to date"));
    assert_eq!(modified(&output), timestamp);
}

#[test]
fn reports_cache_decisions_without_changing_build_semantics() {
    let p = Project::new("report");
    let output = p.build(&["--build-report"]);
    assert!(output.status.success());
    let report = String::from_utf8(output.stderr).unwrap();
    assert!(
        report.contains("cache miss: record unavailable"),
        "{report}"
    );
    assert!(
        report.contains("not saved: inputs/outputs changed"),
        "{report}"
    );
    assert!(report.contains("total:") && report.contains("ms wall"));
    p.success(&[]);
    let hit = p.build(&["--build-report"]);
    assert!(String::from_utf8(hit.stderr).unwrap().contains("cache hit"));
    p.write("src/main.tua", "print(43)\n");
    let miss = p.build(&["--build-report"]);
    let report = String::from_utf8(miss.stderr).unwrap();
    assert!(
        report.contains("changed") && report.contains("main.tua"),
        "{report}"
    );
    let bypass = p.build(&["--no-cache", "--build-report"]);
    assert!(String::from_utf8(bypass.stderr)
        .unwrap()
        .contains("cache bypassed"));
    assert!(p.build(&[]).stderr.is_empty());
}
