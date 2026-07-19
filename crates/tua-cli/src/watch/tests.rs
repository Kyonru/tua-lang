use super::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn fingerprint(len: u64) -> SourceFingerprint {
    SourceFingerprint {
        len,
        modified: UNIX_EPOCH + Duration::from_secs(len),
        content_hash: len,
    }
}

#[test]
fn snapshot_diff_tracks_changed_and_deleted_files() {
    let main = PathBuf::from("/project/src/main.tua");
    let player = PathBuf::from("/project/src/player.tua");
    let hud = PathBuf::from("/project/src/ui/hud.tua");

    let previous = SourceSnapshot {
        files: BTreeMap::from([
            (main.clone(), fingerprint(10)),
            (player.clone(), fingerprint(20)),
        ]),
    };
    let next = SourceSnapshot {
        files: BTreeMap::from([
            (main.clone(), fingerprint(11)),
            (hud.clone(), fingerprint(30)),
        ]),
    };

    let changes = previous.diff(&next);

    assert_eq!(changes.changed, vec![main, hud]);
    assert_eq!(changes.deleted, vec![player]);
    assert_eq!(changes.len(), 3);
}

#[test]
fn snapshot_diff_is_empty_for_identical_inputs() {
    let path = PathBuf::from("/project/src/main.tua");
    let snapshot = SourceSnapshot {
        files: BTreeMap::from([(path, fingerprint(10))]),
    };

    assert!(snapshot.diff(&snapshot).is_empty());
}

#[test]
fn snapshot_diff_detects_content_changes_with_identical_metadata() {
    let path = PathBuf::from("/project/src/main.tua");
    let previous = SourceSnapshot {
        files: BTreeMap::from([(path.clone(), fingerprint(10))]),
    };
    let mut changed = fingerprint(10);
    changed.content_hash = 99;
    let next = SourceSnapshot {
        files: BTreeMap::from([(path.clone(), changed)]),
    };

    assert_eq!(previous.diff(&next).changed, vec![path]);
}

#[test]
fn event_hint_rehashes_same_metadata_file() {
    let path = temp_file("same-metadata", b"new");
    let metadata = fs::metadata(&path).expect("metadata should load");
    let previous = SourceSnapshot {
        files: BTreeMap::from([(
            path.clone(),
            SourceFingerprint {
                len: metadata.len(),
                modified: metadata.modified().unwrap_or(UNIX_EPOCH),
                content_hash: 1,
            },
        )]),
    };
    let batch = WatchBatch {
        paths: [path.clone()].into_iter().collect(),
        rescan_all: false,
    };

    let fingerprint =
        fingerprint_path(&path, Some(&previous), &batch).expect("fingerprint should load");

    assert_ne!(fingerprint.content_hash, 1);
    assert_eq!(
        fingerprint.content_hash,
        hash_file(&path).expect("hash should load")
    );

    fs::remove_file(path).expect("temp file should be removed");
}

#[test]
fn change_set_extracts_config_from_uniform_batch() {
    let config = PathBuf::from("/project/tua.toml");
    let source = PathBuf::from("/project/src/main.tua");
    let mut changes = ChangeSet {
        changed: vec![config.clone(), source.clone()],
        deleted: Vec::new(),
    };

    assert!(changes.take_config_change(&config));
    assert_eq!(changes.changed, vec![source]);
    assert!(!changes.take_config_change(&config));
}

#[test]
fn deleted_tua_sources_remove_lua_and_source_map_outputs() {
    let root = temp_dir("remove-source-outputs");
    let source = root.join("src/main.tua");
    let output = root.join("build/main.lua");
    let source_map = root.join("build/main.lua.map.json");
    let debugger_source_map = root.join("build/main.lua.map");

    fs::create_dir_all(source.parent().expect("source should have a parent"))
        .expect("source directory should be created");
    fs::create_dir_all(output.parent().expect("output should have a parent"))
        .expect("output directory should be created");
    fs::write(&output, "return 1\n").expect("output should be written");
    fs::write(&source_map, "{}\n").expect("source map should be written");
    fs::write(&debugger_source_map, "{}\n").expect("debugger source map should be written");

    let loaded = LoadedConfig {
        path: root.join(CONFIG_FILE),
        config: tua_core::ProjectConfig::default(),
        found: true,
    };
    let outcome = remove_deleted_outputs(&root, &loaded, &[source]);

    assert_eq!(outcome.removed, 3);
    assert!(outcome.diagnostics.is_empty());
    assert!(!output.exists());
    assert!(!source_map.exists());
    assert!(!debugger_source_map.exists());

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

fn temp_file(name: &str, contents: &[u8]) -> PathBuf {
    let path = temp_path(name);

    fs::write(&path, contents).expect("temp file should be written");
    path
}

fn temp_dir(name: &str) -> PathBuf {
    let path = temp_path(name);

    fs::create_dir_all(&path).expect("temp directory should be created");
    path
}

fn temp_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();

    env::temp_dir().join(format!("tua-watch-{name}-{}-{unique}", process::id()))
}
