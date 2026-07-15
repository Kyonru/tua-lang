use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tua_core::{CompletionAdaptiveKey, CompletionAdaptiveMemberStats, CompletionAdaptiveStats};

const STATS_DIR: &str = ".tua";
const STATS_FILE: &str = "completion-stats.json";
const STATS_VERSION: u32 = 1;

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

pub(super) fn file_scope_hash(_workspace_root: &Path, path: &Path) -> u64 {
    CompletionAdaptiveStats::file_scope_hash(path)
}

pub(super) fn load_completion_stats(workspace_root: &Path) -> CompletionAdaptiveStats {
    let path = completion_stats_path(workspace_root);
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
    if stats.is_empty() {
        let _ = remove_completion_stats(workspace_root)?;

        return Ok(());
    }

    let path = completion_stats_path(workspace_root);
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
    fs::write(path, body)?;

    Ok(())
}

pub(super) fn remove_completion_stats(workspace_root: &Path) -> std::io::Result<bool> {
    let path = completion_stats_path(workspace_root);

    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn retain_observed_file_scopes(
    stats: &mut CompletionAdaptiveStats,
    file_scope_hashes: &BTreeSet<u64>,
) {
    stats.retain_file_scopes(file_scope_hashes);
}
