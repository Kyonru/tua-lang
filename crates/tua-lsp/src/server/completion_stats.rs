use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tua_core::{
    path_security::reject_symlink_path, CompletionAdaptiveKey, CompletionAdaptiveMemberStats,
    CompletionAdaptiveStats,
};

const STATS_DIR: &str = ".tua";
const STATS_FILE: &str = "completion-stats.json";
const STATS_VERSION: u32 = 1;
const STATS_ENV: &str = "TUA_COMPLETION_STATS";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct StoredCompletionStats {
    version: u32,
    updated_at_epoch: u64,
    entries: Vec<StoredCompletionStatsEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredCompletionStatsEntry {
    file_scope_hash: u64,
    receiver_hash: u64,
    member: String,
    access_count: u32,
    call_count: u32,
    assignment_rhs_count: u32,
    last_seen_epoch: u64,
}

pub(super) fn current_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

pub(super) fn completion_stats_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(STATS_DIR).join(STATS_FILE)
}

pub(super) fn completion_stats_enabled() -> bool {
    std::env::var(STATS_ENV)
        .map(|value| {
            let value = value.trim();

            !(value == "0"
                || value.eq_ignore_ascii_case("false")
                || value.eq_ignore_ascii_case("off"))
        })
        .unwrap_or(true)
}

pub(super) fn file_scope_hash(_workspace_root: &Path, path: &Path) -> u64 {
    CompletionAdaptiveStats::file_scope_hash(path)
}

pub(super) fn load_completion_stats(workspace_root: &Path) -> CompletionAdaptiveStats {
    if !completion_stats_enabled() {
        return CompletionAdaptiveStats::default();
    }

    let path = completion_stats_path(workspace_root);
    if reject_symlink_path(workspace_root, &path).is_err() {
        return CompletionAdaptiveStats::default();
    }
    let Ok(contents) = fs::read_to_string(path) else {
        return CompletionAdaptiveStats::default();
    };
    let Ok(stored) = serde_json::from_str::<StoredCompletionStats>(&contents) else {
        return CompletionAdaptiveStats::default();
    };

    if stored.version != STATS_VERSION {
        return CompletionAdaptiveStats::default();
    }

    let mut stats = CompletionAdaptiveStats::default();

    for entry in stored.entries {
        stats.insert_member_stats(
            CompletionAdaptiveKey::new(entry.file_scope_hash, entry.receiver_hash),
            entry.member,
            CompletionAdaptiveMemberStats {
                access_count: entry.access_count,
                call_count: entry.call_count,
                assignment_rhs_count: entry.assignment_rhs_count,
                last_seen_epoch: entry.last_seen_epoch,
            },
        );
    }

    stats.evict(current_epoch());
    stats
}

pub(super) fn save_completion_stats(
    workspace_root: &Path,
    stats: &CompletionAdaptiveStats,
) -> std::io::Result<()> {
    if !completion_stats_enabled() {
        return Ok(());
    }

    let path = completion_stats_path(workspace_root);
    check_stats_path(workspace_root, &path)?;

    if stats.is_empty() {
        let _ = remove_completion_stats(workspace_root)?;

        return Ok(());
    }

    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let stored = StoredCompletionStats {
        version: STATS_VERSION,
        updated_at_epoch: current_epoch(),
        entries: stats
            .iter_members()
            .map(|(key, member, stats)| StoredCompletionStatsEntry {
                file_scope_hash: key.file_scope_hash,
                receiver_hash: key.receiver_hash,
                member: member.to_owned(),
                access_count: stats.access_count,
                call_count: stats.call_count,
                assignment_rhs_count: stats.assignment_rhs_count,
                last_seen_epoch: stats.last_seen_epoch,
            })
            .collect(),
    };

    let body = serde_json::to_vec_pretty(&stored).map_err(io::Error::other)?;

    fs::create_dir_all(parent)?;
    check_stats_path(workspace_root, &path)?;
    fs::write(path, body)?;

    Ok(())
}

pub(super) fn remove_completion_stats(workspace_root: &Path) -> std::io::Result<bool> {
    if !completion_stats_enabled() {
        return Ok(false);
    }

    let path = completion_stats_path(workspace_root);
    check_stats_path(workspace_root, &path)?;

    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn check_stats_path(workspace_root: &Path, path: &Path) -> io::Result<()> {
    reject_symlink_path(workspace_root, path)
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))
}

pub(super) fn retain_observed_file_scopes(
    stats: &mut CompletionAdaptiveStats,
    file_scope_hashes: &BTreeSet<u64>,
) {
    stats.retain_file_scopes(file_scope_hashes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tua-completion-stats-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[cfg(unix)]
    #[test]
    fn save_rejects_symlinked_stats_file() {
        use std::os::unix::fs::symlink;

        let root = temp_dir("file-symlink");
        fs::create_dir(root.join(STATS_DIR)).unwrap();
        let victim = root.join("victim.json");
        fs::write(&victim, "keep me\n").unwrap();
        symlink(&victim, completion_stats_path(&root)).unwrap();
        let mut stats = CompletionAdaptiveStats::default();
        stats.insert_member_stats(
            CompletionAdaptiveKey::new(1, 2),
            "field".to_owned(),
            CompletionAdaptiveMemberStats {
                access_count: 1,
                ..CompletionAdaptiveMemberStats::default()
            },
        );

        let error = save_completion_stats(&root, &stats).unwrap_err();

        assert!(error.to_string().contains("symbolic link"), "{error}");
        assert_eq!(fs::read_to_string(&victim).unwrap(), "keep me\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn remove_rejects_symlinked_stats_directory() {
        use std::os::unix::fs::symlink;

        let root = temp_dir("directory-symlink");
        let outside = temp_dir("outside");
        let victim = outside.join(STATS_FILE);
        fs::write(&victim, "keep me\n").unwrap();
        symlink(&outside, root.join(STATS_DIR)).unwrap();

        let error = remove_completion_stats(&root).unwrap_err();

        assert!(error.to_string().contains("symbolic link"), "{error}");
        assert_eq!(fs::read_to_string(&victim).unwrap(), "keep me\n");
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }
}
