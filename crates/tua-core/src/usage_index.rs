use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::module_graph::{ModuleGraph, RequireResolution};
use crate::signal::{
    collect_potential_event_mentions, IndexedSignalEvent, SignalBusIdentity, SignalEventIndex,
    SignalEventMention,
};
use crate::source_text::{is_identifier_continue, is_identifier_start};
use crate::vfs::normalize_path;
use crate::{
    Diagnostic, ParsedSource, ProjectTypeDatabase, ScopeMap, SourcePosition, Type, TypeEnvironment,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct FileUsageIndex {
    member_counts: BTreeMap<String, usize>,
    receiver_member_counts: BTreeMap<String, BTreeMap<String, usize>>,
    call_counts: BTreeMap<String, usize>,
    member_lines: BTreeMap<String, Vec<usize>>,
    signal_events: Vec<SignalEventMention>,
    require_bindings: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ProjectUsageIndex {
    revision: u64,
    files: BTreeMap<PathBuf, Arc<FileUsageIndex>>,
    member_counts: BTreeMap<String, usize>,
    receiver_member_counts: BTreeMap<String, BTreeMap<String, usize>>,
    call_counts: BTreeMap<String, usize>,
    signal_events: SignalEventIndex,
    signal_bus_identities: BTreeMap<(PathBuf, Vec<String>), SignalBusIdentity>,
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

impl FileUsageIndex {
    pub fn build(
        source: &str,
        parsed: Option<&ParsedSource>,
        environment: Option<&TypeEnvironment>,
        scope: Option<&ScopeMap>,
    ) -> Self {
        let mut index = Self::default();

        index.collect_member_usage(source);
        if let (Some(parsed), Some(environment)) = (parsed, environment) {
            index.signal_events = collect_potential_event_mentions(parsed, environment, scope);
            index.require_bindings = crate::require_bindings_for_parsed_source(parsed)
                .into_iter()
                .map(|binding| (binding.alias, binding.module))
                .collect();
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
        module_graph: &ModuleGraph,
        type_database: &ProjectTypeDatabase,
    ) -> Self {
        let mut index = Self {
            revision,
            ..Self::default()
        };
        let mut signal_events = Vec::new();

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

            let path = normalize_path(path);
            for mention in &file.signal_events {
                if !mention.confirmed_bus
                    && !project_receiver_is_signal_bus(&path, mention, module_graph, type_database)
                {
                    continue;
                }

                let identity = signal_bus_identity(&path, file, mention, module_graph);
                index.signal_bus_identities.insert(
                    (path.clone(), mention.receiver_parts.clone()),
                    identity.clone(),
                );

                if mention.name.is_empty() {
                    continue;
                }

                signal_events.push(IndexedSignalEvent {
                    identity,
                    path: path.clone(),
                    mention: mention.clone(),
                });
            }
            index.files.insert(path.to_path_buf(), Arc::clone(file));
        }

        index.signal_events = SignalEventIndex::new(signal_events);
        index
    }

    pub fn from_sources(current_path: &Path, source: &str, project_sources: &[&str]) -> Self {
        let mut files = BTreeMap::new();

        files.insert(
            normalize_path(current_path),
            Arc::new(FileUsageIndex::build(source, None, None, None)),
        );
        for (index, project_source) in project_sources.iter().enumerate() {
            files.insert(
                PathBuf::from(format!("__completion_project_source_{index}.tua")),
                Arc::new(FileUsageIndex::build(project_source, None, None, None)),
            );
        }

        Self::build(
            0,
            files.iter().map(|(path, file)| (path.as_path(), file)),
            &ModuleGraph::default(),
            &ProjectTypeDatabase::default(),
        )
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
        self.signal_events.events()
    }

    pub fn signal_event_index(&self) -> &SignalEventIndex {
        &self.signal_events
    }

    pub fn signal_event_at(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<&IndexedSignalEvent> {
        self.signal_events.event_at(&normalize_path(path), position)
    }

    pub fn signal_event_definition(
        &self,
        event: &IndexedSignalEvent,
    ) -> Option<&IndexedSignalEvent> {
        self.signal_events.definition(event)
    }

    pub fn same_signal_bus(
        &self,
        path: &Path,
        receiver_parts: &[String],
        candidate: &IndexedSignalEvent,
        position: SourcePosition,
    ) -> bool {
        let path = normalize_path(path);

        self.signal_bus_identities
            .get(&(path.clone(), receiver_parts.to_vec()))
            .is_some_and(|identity| identity == &candidate.identity)
            || self
                .signal_events
                .same_bus(&path, receiver_parts, candidate, position)
    }

    pub fn signal_event_diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        self.signal_events
            .diagnostics_for_path(&normalize_path(path))
    }
}

fn signal_bus_identity(
    path: &Path,
    file: &FileUsageIndex,
    mention: &SignalEventMention,
    module_graph: &ModuleGraph,
) -> SignalBusIdentity {
    let Some(receiver) = mention.receiver_parts.first() else {
        return local_signal_bus_identity(path, mention);
    };
    let Some(module) = file.require_bindings.get(receiver) else {
        return local_signal_bus_identity(path, mention);
    };
    let Some(resolution) = module_graph.resolve_require(path, module) else {
        return local_signal_bus_identity(path, mention);
    };
    let origin = match resolution {
        RequireResolution::Resolved { path } => path.clone(),
        RequireResolution::Duplicate { selected, .. } => selected.clone(),
        RequireResolution::Missing { .. } | RequireResolution::Invalid { .. } => {
            return local_signal_bus_identity(path, mention);
        }
    };

    SignalBusIdentity {
        origin: normalize_path(origin),
        root_definition: None,
        receiver_parts: mention.receiver_parts.iter().skip(1).cloned().collect(),
    }
}

fn local_signal_bus_identity(path: &Path, mention: &SignalEventMention) -> SignalBusIdentity {
    SignalBusIdentity {
        origin: normalize_path(path),
        root_definition: mention
            .root_definition
            .map(|span| (span.start.line, span.start.column)),
        receiver_parts: mention.receiver_parts.clone(),
    }
}

fn project_receiver_is_signal_bus(
    path: &Path,
    mention: &SignalEventMention,
    module_graph: &ModuleGraph,
    type_database: &ProjectTypeDatabase,
) -> bool {
    let Some((receiver, fields)) = mention.receiver_parts.split_first() else {
        return false;
    };
    let Some(ty) = type_database.module_export_type(module_graph, path, receiver, fields) else {
        return false;
    };

    matches!(ty, Type::Alias(ref name) if name == crate::signal::TYPE_NAME)
        || ty == crate::signal::signal_bus_type()
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
