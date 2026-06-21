use super::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

fn fingerprint(len: u64) -> SourceFingerprint {
    SourceFingerprint {
        len,
        modified: UNIX_EPOCH + Duration::from_secs(len),
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
