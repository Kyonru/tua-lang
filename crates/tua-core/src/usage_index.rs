use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::signal::{collect_event_mentions, SignalEventMention};
use crate::source_text::{is_identifier_continue, is_identifier_start};
use crate::vfs::normalize_path;
use crate::{ParsedSource, SourcePosition, TypeEnvironment};

#[derive(Clone, Debug, Default)]
pub(crate) struct FileUsageIndex {
    member_counts: BTreeMap<String, usize>,
    receiver_member_counts: BTreeMap<String, BTreeMap<String, usize>>,
    call_counts: BTreeMap<String, usize>,
    member_lines: BTreeMap<String, Vec<usize>>,
    signal_events: Vec<SignalEventMention>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ProjectUsageIndex {
    revision: u64,
    files: BTreeMap<PathBuf, Arc<FileUsageIndex>>,
    member_counts: BTreeMap<String, usize>,
    receiver_member_counts: BTreeMap<String, BTreeMap<String, usize>>,
    call_counts: BTreeMap<String, usize>,
    signal_events: Vec<IndexedSignalEvent>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MemberUsageSummary {
    pub member_counts: BTreeMap<String, usize>,
    pub receiver_member_counts: BTreeMap<String, usize>,
    pub call_counts: BTreeMap<String, usize>,
    pub proximity_scores: BTreeMap<String, usize>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MemberUsageCounts {
    pub member: usize,
    pub receiver_member: usize,
    pub call: usize,
    pub proximity: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct IndexedSignalEvent {
    pub path: PathBuf,
    pub mention: SignalEventMention,
}

impl FileUsageIndex {
    pub fn build(
        source: &str,
        parsed: Option<&ParsedSource>,
        environment: Option<&TypeEnvironment>,
    ) -> Self {
        let mut index = Self::default();

        index.collect_member_usage(source);
        if let (Some(parsed), Some(environment)) = (parsed, environment) {
            index.signal_events = collect_event_mentions(parsed, environment, None);
        }

        index
    }

    fn collect_member_usage(&mut self, source: &str) {
        let bytes = source.as_bytes();
        let mut index = 0;
        let mut line = 1;
        let mut quote = None;
        let mut escaped = false;
        let mut line_comment = false;

        while index < bytes.len() {
            let byte = bytes[index];

            if byte == b'\n' {
                line += 1;
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
                if let Some((member, end)) = identifier_at(source, index + 1) {
                    *self.member_counts.entry(member.to_owned()).or_default() += 1;
                    self.member_lines
                        .entry(member.to_owned())
                        .or_default()
                        .push(line);

                    if next_code_byte_after(bytes, end) == Some(b'(') {
                        *self.call_counts.entry(member.to_owned()).or_default() += 1;
                    }

                    if let Some(receiver) = identifier_before(source, index) {
                        *self
                            .receiver_member_counts
                            .entry(receiver)
                            .or_default()
                            .entry(member.to_owned())
                            .or_default() += 1;
                    }

                    index = end;
                    continue;
                }
            }

            index += 1;
        }
    }
}

impl ProjectUsageIndex {
    pub fn build<'a>(
        revision: u64,
        files: impl IntoIterator<Item = (&'a Path, &'a Arc<FileUsageIndex>)>,
    ) -> Self {
        let mut index = Self {
            revision,
            ..Self::default()
        };

        for (path, file) in files {
            merge_counts(&mut index.member_counts, &file.member_counts);
            merge_counts(&mut index.call_counts, &file.call_counts);

            for (receiver, members) in &file.receiver_member_counts {
                merge_counts(
                    index
                        .receiver_member_counts
                        .entry(receiver.clone())
                        .or_default(),
                    members,
                );
            }

            index
                .signal_events
                .extend(
                    file.signal_events
                        .iter()
                        .cloned()
                        .map(|mention| IndexedSignalEvent {
                            path: path.to_path_buf(),
                            mention,
                        }),
                );
            index.files.insert(path.to_path_buf(), Arc::clone(file));
        }

        index
    }

    pub fn from_sources(current_path: &Path, source: &str, project_sources: &[&str]) -> Self {
        let mut files = BTreeMap::new();

        files.insert(
            normalize_path(current_path),
            Arc::new(FileUsageIndex::build(source, None, None)),
        );
        for (index, project_source) in project_sources.iter().enumerate() {
            files.insert(
                PathBuf::from(format!("__completion_project_source_{index}.tua")),
                Arc::new(FileUsageIndex::build(project_source, None, None)),
            );
        }

        Self::build(0, files.iter().map(|(path, file)| (path.as_path(), file)))
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn member_usage(
        &self,
        current_path: &Path,
        receiver: Option<&str>,
        position: SourcePosition,
    ) -> MemberUsageSummary {
        let current_path = normalize_path(current_path);
        let proximity_scores = self
            .files
            .get(&current_path)
            .map(|file| {
                file.member_lines
                    .iter()
                    .filter_map(|(member, lines)| {
                        let score = lines
                            .iter()
                            .map(|line| proximity_score(*line, position.line))
                            .sum::<usize>();

                        (score > 0).then(|| (member.clone(), score))
                    })
                    .collect()
            })
            .unwrap_or_default();

        MemberUsageSummary {
            member_counts: self.member_counts.clone(),
            receiver_member_counts: receiver
                .and_then(|receiver| self.receiver_member_counts.get(receiver))
                .cloned()
                .unwrap_or_default(),
            call_counts: self.call_counts.clone(),
            proximity_scores,
        }
    }

    pub fn member_usage_counts(
        &self,
        current_path: &Path,
        receiver: Option<&str>,
        position: SourcePosition,
        member: &str,
    ) -> MemberUsageCounts {
        let current_path = normalize_path(current_path);
        let proximity = self
            .files
            .get(&current_path)
            .and_then(|file| file.member_lines.get(member))
            .map(|lines| {
                lines
                    .iter()
                    .map(|line| proximity_score(*line, position.line))
                    .sum()
            })
            .unwrap_or_default();

        MemberUsageCounts {
            member: self.member_counts.get(member).copied().unwrap_or_default(),
            receiver_member: receiver
                .and_then(|receiver| self.receiver_member_counts.get(receiver))
                .and_then(|members| members.get(member))
                .copied()
                .unwrap_or_default(),
            call: self.call_counts.get(member).copied().unwrap_or_default(),
            proximity,
        }
    }

    pub fn signal_events(&self) -> &[IndexedSignalEvent] {
        &self.signal_events
    }
}

fn merge_counts(target: &mut BTreeMap<String, usize>, source: &BTreeMap<String, usize>) {
    for (name, count) in source {
        *target.entry(name.clone()).or_default() += count;
    }
}

fn proximity_score(line: usize, cursor_line: usize) -> usize {
    let distance = line.abs_diff(cursor_line);

    if distance > 50 {
        0
    } else {
        50 - distance + usize::from(line <= cursor_line) * 10
    }
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

fn identifier_before(source: &str, end: usize) -> Option<String> {
    let bytes = source.as_bytes();

    if end == 0 || !is_identifier_continue(*bytes.get(end - 1)?) {
        return None;
    }

    let mut start = end - 1;

    while start > 0 && is_identifier_continue(bytes[start - 1]) {
        start -= 1;
    }

    Some(source[start..end].to_owned())
}

fn next_code_byte_after(bytes: &[u8], mut index: usize) -> Option<u8> {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }

    bytes.get(index).copied()
}
