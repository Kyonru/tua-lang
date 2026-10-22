//! Local VS Code menu-state replay ingestion for the Phase 1 gate.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::dataset::EvalSet;
use crate::inline_completion::eligibility::cursor_context;
use crate::SourcePosition;

#[derive(Clone, Debug, Default)]
pub struct MenuReplay {
    entries: BTreeMap<PathBuf, Vec<ReplayEntry>>,
    base_report: MenuReplayReport,
}

#[derive(Clone, Debug)]
pub struct ReplayMenuState {
    pub menu_open: bool,
    pub selected_text: Option<String>,
    pub selected_prefix: Option<String>,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct MenuReplayReport {
    pub records: usize,
    pub accepted: usize,
    pub matched: usize,
    pub unmatched: usize,
    /// Earlier records replaced by a later record with the same file, source
    /// hash, and cursor. The last capture wins deterministically.
    pub superseded: usize,
    pub rejected_schema: usize,
    pub rejected_path: usize,
    pub rejected_position: usize,
    pub rejected_ineligible: usize,
}

#[derive(Clone, Debug)]
struct ReplayEntry {
    source_hash: String,
    position: LspPosition,
    selected: Option<SelectedCompletion>,
    outcome: Arc<AtomicU8>,
}

const UNRESOLVED: u8 = 0;
const MATCHED: u8 = 1;
const BAD_POSITION: u8 = 2;
const INELIGIBLE: u8 = 3;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    schema_version: u32,
    dataset_path: PathBuf,
    source_hash: String,
    position: LspPosition,
    #[allow(dead_code)]
    document_version: i64,
    #[allow(dead_code)]
    elapsed_since_edit_ms: Option<f64>,
    menu_open: bool,
    selected_completion: Option<SelectedCompletion>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct LspPosition {
    line: usize,
    character: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectedCompletion {
    text: String,
    range: LspRange,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LspRange {
    start: LspPosition,
    end: LspPosition,
}

impl MenuReplay {
    pub fn load(path: &Path, sets: &[EvalSet]) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("cannot read menu replay {}: {error}", path.display()))?;
        let files = sets
            .iter()
            .flat_map(|set| &set.projects)
            .flat_map(|project| &project.files)
            .collect::<Vec<_>>();
        let mut replay = Self::default();

        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            replay.base_report.records += 1;
            let Ok(record) = serde_json::from_str::<Record>(line) else {
                replay.base_report.rejected_schema += 1;
                continue;
            };
            if record.schema_version != 1
                || record.menu_open != record.selected_completion.is_some()
                || record.source_hash.len() != 64
                || !record
                    .source_hash
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
            {
                replay.base_report.rejected_schema += 1;
                continue;
            }
            let matches = files
                .iter()
                .filter(|file| file.path.ends_with(&record.dataset_path))
                .collect::<Vec<_>>();
            let [file] = matches.as_slice() else {
                replay.base_report.rejected_path += 1;
                continue;
            };
            let source_hash = record.source_hash.to_ascii_lowercase();
            let entries = replay.entries.entry(file.path.clone()).or_default();
            let entry = ReplayEntry {
                source_hash: source_hash.clone(),
                position: record.position,
                selected: record.selected_completion,
                outcome: Arc::new(AtomicU8::new(UNRESOLVED)),
            };
            if let Some(existing) = entries.iter_mut().find(|existing| {
                existing.source_hash == source_hash && existing.position == entry.position
            }) {
                *existing = entry;
                replay.base_report.superseded += 1;
            } else {
                entries.push(entry);
            }
            replay.base_report.accepted += 1;
        }
        Ok(replay)
    }

    pub fn get(
        &self,
        path: &Path,
        document: &str,
        position: SourcePosition,
    ) -> Option<ReplayMenuState> {
        let normalized = document.replace("\r\n", "\n");
        let hash = sha256_hex(normalized.as_bytes());
        for entry in self.entries.get(path)? {
            if entry.source_hash != hash {
                continue;
            }
            let Some((captured, cursor_offset)) =
                source_position_from_lsp(document, entry.position.line, entry.position.character)
            else {
                mark_once(&entry.outcome, BAD_POSITION);
                continue;
            };
            if cursor_context(document, cursor_offset).is_none() {
                mark_once(&entry.outcome, INELIGIBLE);
                continue;
            }
            if captured != position {
                continue;
            }
            let (selected_text, selected_prefix) = if let Some(selected) = &entry.selected {
                let Ok(resolved) = resolve_selection(document, cursor_offset, selected) else {
                    mark_once(&entry.outcome, BAD_POSITION);
                    continue;
                };
                resolved
            } else {
                (None, None)
            };
            entry.outcome.store(MATCHED, Ordering::Relaxed);
            return Some(ReplayMenuState {
                menu_open: entry.selected.is_some(),
                selected_text,
                selected_prefix,
            });
        }
        None
    }

    pub fn positions_for(&self, path: &Path, source: &str) -> Vec<(SourcePosition, usize)> {
        self.entries
            .get(path)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                source_position_from_lsp(source, entry.position.line, entry.position.character)
            })
            .collect()
    }

    pub fn report(&self) -> MenuReplayReport {
        let mut report = self.base_report.clone();
        for entry in self.entries.values().flatten() {
            match entry.outcome.load(Ordering::Relaxed) {
                MATCHED => report.matched += 1,
                BAD_POSITION => report.rejected_position += 1,
                INELIGIBLE => report.rejected_ineligible += 1,
                _ => report.unmatched += 1,
            }
        }
        report
    }
}

fn mark_once(outcome: &AtomicU8, value: u8) {
    let _ = outcome.compare_exchange(UNRESOLVED, value, Ordering::Relaxed, Ordering::Relaxed);
}

fn resolve_selection(
    document: &str,
    cursor_offset: usize,
    selected: &SelectedCompletion,
) -> Result<(Option<String>, Option<String>), ()> {
    if selected.range.start.line != selected.range.end.line {
        return Err(());
    }
    let (_, start) = source_position_from_lsp(
        document,
        selected.range.start.line,
        selected.range.start.character,
    )
    .ok_or(())?;
    let (_, end) = source_position_from_lsp(
        document,
        selected.range.end.line,
        selected.range.end.character,
    )
    .ok_or(())?;
    if start > cursor_offset || end != cursor_offset {
        return Err(());
    }
    Ok((
        Some(selected.text.clone()),
        Some(document[start..cursor_offset].to_owned()),
    ))
}

fn source_position_from_lsp(
    source: &str,
    line_index: usize,
    utf16_column: usize,
) -> Option<(SourcePosition, usize)> {
    let mut offset = 0usize;
    let line = source.split_inclusive('\n').nth(line_index)?;
    let line = line.strip_suffix('\n').unwrap_or(line);
    let line = line.strip_suffix('\r').unwrap_or(line);
    for previous in source.split_inclusive('\n').take(line_index) {
        offset += previous.len();
    }
    let mut units = 0usize;
    let mut byte = 0usize;
    for character in line.chars() {
        if units == utf16_column {
            break;
        }
        let width = character.len_utf16();
        if units + width > utf16_column {
            return None;
        }
        units += width;
        byte += character.len_utf8();
    }
    if units != utf16_column {
        return None;
    }
    Some((
        SourcePosition {
            line: line_index + 1,
            column: line[..byte].chars().count() + 1,
        },
        offset + byte,
    ))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inline_completion::eval::dataset::{EvalFile, EvalProject, EvalSet, SetRole};
    use crate::{AnalysisSourceKind, ProjectConfig};

    #[test]
    fn converts_utf16_positions_without_splitting_surrogates() {
        let source = "local emoji = '😀'\nprint(emoji)";
        let (position, offset) = source_position_from_lsp(source, 0, 17).expect("after emoji");
        assert_eq!(
            position,
            SourcePosition {
                line: 1,
                column: 17
            }
        );
        assert_eq!(&source[offset..offset + 1], "'");
        assert!(source_position_from_lsp(source, 0, 16).is_none());
    }

    #[test]
    fn computes_standard_sha256() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn scenario_hashes_normalize_crlf_like_the_extension() {
        assert_eq!(
            sha256_hex("a\r\nb\r\n".replace("\r\n", "\n").as_bytes()),
            sha256_hex(b"a\nb\n")
        );
    }

    #[test]
    fn loads_the_source_free_extension_schema_and_reports_rejections() {
        let source = "local value = 1\nprint(value)";
        let file = EvalFile {
            path: PathBuf::from("/workspace/project/main.lua"),
            kind: AnalysisSourceKind::Lua,
            source: source.to_owned(),
        };
        let sets = vec![EvalSet {
            name: "tune".to_owned(),
            role: SetRole::Tune,
            projects: vec![EvalProject {
                name: "project".to_owned(),
                model: crate::ProjectModel::default(),
                config: ProjectConfig::default(),
                source_roots: Vec::new(),
                output_roots: Vec::new(),
                asset_roots: Vec::new(),
                files: vec![file],
            }],
        }];
        let edited = "local value = 1\nprint(val";
        let hash = sha256_hex(edited.as_bytes());
        let selected = serde_json::json!({
            "text": "value",
            "range": {
                "start": { "line": 1, "character": 6 },
                "end": { "line": 1, "character": 9 }
            }
        });
        let selected_last = serde_json::json!({
            "text": "planet",
            "range": {
                "start": { "line": 1, "character": 6 },
                "end": { "line": 1, "character": 9 }
            }
        });
        let record =
            |path: &str, hash: &str, menu_open: bool, selection: Option<&serde_json::Value>| {
                serde_json::json!({
                    "schemaVersion": 1,
                    "datasetPath": path,
                    "sourceHash": hash,
                "position": { "line": 1, "character": 9 },
                    "documentVersion": 3,
                    "elapsedSinceEditMs": null,
                    "menuOpen": menu_open,
                    "selectedCompletion": selection,
                })
                .to_string()
            };
        let contents = [
            record("project/main.lua", &hash, true, Some(&selected)),
            record("project/main.lua", &hash, false, Some(&selected)),
            record("project/main.lua", &"0".repeat(64), true, Some(&selected)),
            record("other/main.lua", &hash, true, Some(&selected)),
            record("project/main.lua", &hash, true, Some(&selected_last)),
        ]
        .join("\n");
        let path = std::env::temp_dir().join(format!(
            "tua-inline-menu-replay-{}-{}.jsonl",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::write(&path, contents).expect("write replay fixture");
        let replay = MenuReplay::load(&path, &sets).expect("load replay fixture");
        let _ = fs::remove_file(path);

        assert!(replay
            .get(
                Path::new("/workspace/project/main.lua"),
                source,
                SourcePosition {
                    line: 2,
                    column: 10
                },
            )
            .is_none());
        let state = replay
            .get(
                Path::new("/workspace/project/main.lua"),
                edited,
                SourcePosition {
                    line: 2,
                    column: 10,
                },
            )
            .expect("accepted menu state");
        assert!(state.menu_open);
        assert_eq!(state.selected_text.as_deref(), Some("planet"));
        assert_eq!(state.selected_prefix.as_deref(), Some("val"));

        let report = replay.report();
        assert_eq!(report.records, 5);
        assert_eq!(report.accepted, 3);
        assert_eq!(report.matched, 1);
        assert_eq!(report.unmatched, 1);
        assert_eq!(report.superseded, 1);
        assert_eq!(report.rejected_schema, 1);
        assert_eq!(report.rejected_path, 1);
    }

    #[test]
    fn evaluator_matches_a_real_partial_edit_and_forces_its_position_into_the_sample() {
        let source = "print(player.name)\nprint(player.name)\nreturn player";
        let edited = "print(player.name)\nprint(pla\nreturn player";
        let file_path = PathBuf::from("/workspace/project/main.lua");
        let sets = vec![EvalSet {
            name: "tune".to_owned(),
            role: SetRole::Tune,
            projects: vec![EvalProject {
                name: "project".to_owned(),
                model: crate::ProjectModel::default(),
                config: ProjectConfig::default(),
                source_roots: Vec::new(),
                output_roots: Vec::new(),
                asset_roots: Vec::new(),
                files: vec![EvalFile {
                    path: file_path,
                    kind: AnalysisSourceKind::Lua,
                    source: source.to_owned(),
                }],
            }],
        }];
        let record = serde_json::json!({
            "schemaVersion": 1,
            "datasetPath": "project/main.lua",
            "sourceHash": sha256_hex(edited.as_bytes()),
            "position": { "line": 1, "character": 9 },
            "documentVersion": 7,
            "elapsedSinceEditMs": 72.0,
            "menuOpen": true,
            "selectedCompletion": {
                "text": "player",
                "range": {
                    "start": { "line": 1, "character": 6 },
                    "end": { "line": 1, "character": 9 }
                }
            }
        });
        let path = std::env::temp_dir().join(format!(
            "tua-inline-menu-replay-e2e-{}.jsonl",
            std::process::id()
        ));
        fs::write(&path, record.to_string()).expect("write replay fixture");
        let replay = MenuReplay::load(&path, &sets).expect("load replay fixture");
        let _ = fs::remove_file(path);
        let options = super::super::EvalOptions {
            // The normal cap would not retain this middle-of-file position.
            max_positions_per_file: 1,
            simulate_menu: false,
            measure_warmup: false,
            menu_replay: Some(replay),
            ..super::super::EvalOptions::default()
        };
        let report = super::super::evaluate(
            &sets,
            super::super::dataset::DuplicateReport::default(),
            &options,
        );

        let replay = report.menu_replay.expect("replay report");
        assert_eq!(replay.matched, 1);
        assert_eq!(replay.unmatched, 0);
        let phase1 = &report.sets[0].backends["phase1"];
        assert!(phase1.by_menu["replay-open"].eligible > 0);
    }
}
