use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::source_text::{is_identifier_continue, is_identifier_start};
use crate::vfs::normalize_path;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;
const MAX_ADAPTIVE_MEMBERS: usize = 4096;
const MAX_SIGNAL_AGE: u64 = 30 * 24 * 60 * 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct CompletionAdaptiveKey {
    pub file_scope_hash: u64,
    pub receiver_hash: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompletionAdaptiveMemberStats {
    pub access_count: u32,
    pub call_count: u32,
    pub assignment_rhs_count: u32,
    pub last_seen_epoch: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompletionAdaptiveEntry {
    pub members: BTreeMap<String, CompletionAdaptiveMemberStats>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompletionAdaptiveStats {
    entries: BTreeMap<CompletionAdaptiveKey, CompletionAdaptiveEntry>,
}

impl CompletionAdaptiveKey {
    pub fn new(file_scope_hash: u64, receiver_hash: u64) -> Self {
        Self {
            file_scope_hash: nonzero_hash(file_scope_hash),
            receiver_hash: nonzero_hash(receiver_hash),
        }
    }
}

impl CompletionAdaptiveMemberStats {
    fn score(self) -> usize {
        self.access_count as usize * 400
            + self.call_count as usize * 1_200
            + self.assignment_rhs_count as usize * 800
    }

    fn total(self) -> u32 {
        self.access_count
            .saturating_add(self.call_count)
            .saturating_add(self.assignment_rhs_count)
    }
}

impl CompletionAdaptiveStats {
    pub fn file_scope_hash(path: &Path) -> u64 {
        let mut hash = FNV_OFFSET;
        let path = normalize_path(path);

        for component in path.components() {
            hash = hash_bytes(hash, component.as_os_str().to_string_lossy().as_bytes());
            hash = hash_byte(hash, b'/');
        }

        nonzero_hash(hash)
    }

    pub fn receiver_hash(receiver_parts: &[String]) -> u64 {
        if receiver_parts.is_empty() {
            return 1;
        }

        let mut hash = FNV_OFFSET;

        for part in receiver_parts {
            hash = hash_bytes(hash, part.as_bytes());
            hash = hash_byte(hash, 0);
        }

        nonzero_hash(hash)
    }

    pub fn insert_member_stats(
        &mut self,
        key: CompletionAdaptiveKey,
        member: impl Into<String>,
        stats: CompletionAdaptiveMemberStats,
    ) {
        if stats.total() == 0 {
            return;
        }

        self.entries
            .entry(key)
            .or_default()
            .members
            .insert(member.into(), stats);
    }

    pub fn merge(&mut self, other: Self) {
        for (key, entry) in other.entries {
            for (member, stats) in entry.members {
                let target = self
                    .entries
                    .entry(key)
                    .or_default()
                    .members
                    .entry(member)
                    .or_default();

                target.access_count = target.access_count.saturating_add(stats.access_count);
                target.call_count = target.call_count.saturating_add(stats.call_count);
                target.assignment_rhs_count = target
                    .assignment_rhs_count
                    .saturating_add(stats.assignment_rhs_count);
                target.last_seen_epoch = target.last_seen_epoch.max(stats.last_seen_epoch);
            }
        }
    }

    pub fn replace_file_observations(
        &mut self,
        file_scope_hash: u64,
        source: &str,
        now_epoch: u64,
    ) {
        let file_scope_hash = nonzero_hash(file_scope_hash);

        self.entries
            .retain(|key, _| key.file_scope_hash != file_scope_hash);
        self.observe_source(file_scope_hash, source, now_epoch);
        self.evict(now_epoch);
    }

    pub fn remove_file_observations(&mut self, file_scope_hash: u64) {
        let file_scope_hash = nonzero_hash(file_scope_hash);

        self.entries
            .retain(|key, _| key.file_scope_hash != file_scope_hash);
    }

    pub fn retain_file_scopes(&mut self, file_scope_hashes: &BTreeSet<u64>) {
        self.entries
            .retain(|key, _| file_scope_hashes.contains(&key.file_scope_hash));
    }

    pub fn score(&self, file_scope_hash: u64, receiver_parts: &[String], member: &str) -> usize {
        let key = CompletionAdaptiveKey::new(file_scope_hash, Self::receiver_hash(receiver_parts));

        self.entries
            .get(&key)
            .and_then(|entry| entry.members.get(member).copied())
            .map(CompletionAdaptiveMemberStats::score)
            .unwrap_or_default()
    }

    pub fn iter_members(
        &self,
    ) -> impl Iterator<Item = (CompletionAdaptiveKey, &str, CompletionAdaptiveMemberStats)> {
        self.entries.iter().flat_map(|(key, entry)| {
            entry
                .members
                .iter()
                .map(|(member, stats)| (*key, member.as_str(), *stats))
        })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn evict(&mut self, now_epoch: u64) {
        for entry in self.entries.values_mut() {
            entry.members.retain(|_, stats| {
                stats.total() > 0
                    && now_epoch.saturating_sub(stats.last_seen_epoch) <= MAX_SIGNAL_AGE
            });
        }

        self.entries.retain(|_, entry| !entry.members.is_empty());
        self.evict_to_budget();
    }

    fn observe_source(&mut self, file_scope_hash: u64, source: &str, now_epoch: u64) {
        let bytes = source.as_bytes();
        let mut index = 0;
        let mut quote = None;
        let mut escaped = false;
        let mut line_comment = false;

        while index < bytes.len() {
            let byte = bytes[index];

            if byte == b'\n' {
                line_comment = false;
                index += 1;
                continue;
            }

            if line_comment {
                index += 1;
                continue;
            }

            if let Some(quote_byte) = quote {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == quote_byte {
                    quote = None;
                }
                index += 1;
                continue;
            }

            if byte == b'-' && bytes.get(index + 1) == Some(&b'-') {
                line_comment = true;
                index += 2;
                continue;
            }

            if matches!(byte, b'\'' | b'"') {
                quote = Some(byte);
                index += 1;
                continue;
            }

            if matches!(byte, b'.' | b':') {
                if let (Some(receiver), Some((member, end))) = (
                    identifier_before(source, index),
                    identifier_at(source, index + 1),
                ) {
                    let receiver_hash = receiver_identifier_hash(receiver);
                    let is_call = next_code_byte_after(bytes, end) == Some(b'(');
                    let is_assignment_rhs = member_is_assignment_rhs(bytes, index);

                    self.record_member(
                        CompletionAdaptiveKey::new(file_scope_hash, receiver_hash),
                        member,
                        is_call,
                        is_assignment_rhs,
                        now_epoch,
                    );

                    index = end;
                    continue;
                }
            }

            index += 1;
        }
    }

    fn record_member(
        &mut self,
        key: CompletionAdaptiveKey,
        member: &str,
        is_call: bool,
        is_assignment_rhs: bool,
        now_epoch: u64,
    ) {
        let stats = self
            .entries
            .entry(key)
            .or_default()
            .members
            .entry(member.to_owned())
            .or_default();

        stats.access_count = stats.access_count.saturating_add(1);
        if is_call {
            stats.call_count = stats.call_count.saturating_add(1);
        }
        if is_assignment_rhs {
            stats.assignment_rhs_count = stats.assignment_rhs_count.saturating_add(1);
        }
        stats.last_seen_epoch = now_epoch;
    }

    fn evict_to_budget(&mut self) {
        let member_count = self
            .entries
            .values()
            .map(|entry| entry.members.len())
            .sum::<usize>();

        if member_count <= MAX_ADAPTIVE_MEMBERS {
            return;
        }

        let mut ranked = self
            .iter_members()
            .map(|(key, member, stats)| {
                (stats.last_seen_epoch, stats.total(), key, member.to_owned())
            })
            .collect::<Vec<_>>();

        ranked.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
        });

        let retained = ranked
            .into_iter()
            .take(MAX_ADAPTIVE_MEMBERS)
            .map(|(_, _, key, member)| (key, member))
            .collect::<BTreeSet<_>>();

        for (key, entry) in &mut self.entries {
            entry
                .members
                .retain(|member, _| retained.contains(&(*key, member.clone())));
        }
        self.entries.retain(|_, entry| !entry.members.is_empty());
    }
}

fn member_is_assignment_rhs(bytes: &[u8], member_separator: usize) -> bool {
    let line_start = bytes[..member_separator]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |position| position + 1);
    let mut index = line_start;

    while index < member_separator {
        if bytes[index] == b'='
            && bytes
                .get(index.saturating_sub(1))
                .is_none_or(|byte| !matches!(*byte, b'=' | b'<' | b'>' | b'~'))
            && bytes
                .get(index + 1)
                .is_none_or(|byte| !matches!(*byte, b'='))
        {
            return true;
        }

        index += 1;
    }

    false
}

fn identifier_at(source: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = source.as_bytes();

    if !bytes
        .get(start)
        .is_some_and(|byte| is_identifier_start(*byte))
    {
        return None;
    }

    let mut end = start + 1;

    while end < bytes.len() && is_identifier_continue(bytes[end]) {
        end += 1;
    }

    Some((&source[start..end], end))
}

fn identifier_before(source: &str, end: usize) -> Option<&str> {
    let bytes = source.as_bytes();

    if end == 0 || !is_identifier_continue(*bytes.get(end - 1)?) {
        return None;
    }

    let mut start = end - 1;

    while start > 0 && is_identifier_continue(bytes[start - 1]) {
        start -= 1;
    }

    Some(&source[start..end])
}

fn next_code_byte_after(bytes: &[u8], mut index: usize) -> Option<u8> {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }

    bytes.get(index).copied()
}

fn receiver_identifier_hash(receiver: &str) -> u64 {
    nonzero_hash(hash_byte(hash_bytes(FNV_OFFSET, receiver.as_bytes()), 0))
}

fn hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = hash_byte(hash, *byte);
    }

    hash
}

fn hash_byte(mut hash: u64, byte: u8) -> u64 {
    hash ^= u64::from(byte);
    hash = hash.wrapping_mul(FNV_PRIME);

    hash
}

fn nonzero_hash(hash: u64) -> u64 {
    if hash == 0 {
        1
    } else {
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observes_member_calls_and_assignment_rhs_without_source_content() {
        let mut stats = CompletionAdaptiveStats::default();
        let file_hash = CompletionAdaptiveStats::file_scope_hash(Path::new("src/player.tua"));
        let receiver = vec!["sprite".to_owned()];

        stats.replace_file_observations(
            file_hash,
            r#"
sprite.play()
local current = sprite.pause
sprite.stop()
sprite.stop()
"#,
            100,
        );

        assert!(
            stats.score(file_hash, &receiver, "stop") > stats.score(file_hash, &receiver, "play")
        );
        assert!(stats.score(file_hash, &receiver, "pause") > 0);
        assert_eq!(stats.score(file_hash, &receiver, "missing"), 0);
        assert!(stats
            .iter_members()
            .all(|(_, member, _)| { matches!(member, "play" | "pause" | "stop") }));
    }

    #[test]
    fn replaces_file_observations_and_expires_stale_members() {
        let mut stats = CompletionAdaptiveStats::default();
        let file_hash = CompletionAdaptiveStats::file_scope_hash(Path::new("src/player.tua"));
        let receiver = vec!["sprite".to_owned()];

        stats.replace_file_observations(file_hash, "sprite.old()\n", 1);
        stats.replace_file_observations(file_hash, "sprite.new()\n", 2);

        assert_eq!(stats.score(file_hash, &receiver, "old"), 0);
        assert!(stats.score(file_hash, &receiver, "new") > 0);

        stats.evict(2 + MAX_SIGNAL_AGE + 1);

        assert_eq!(stats.score(file_hash, &receiver, "new"), 0);
    }
}
