use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{SourcePosition, SourceSpan};

const DEFAULT_FIXTURE_PATH: &str = "src/main.tua";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fixture {
    files: Vec<FixtureFile>,
    markers: BTreeMap<String, FixtureMarker>,
    expectations: Vec<FixtureExpectation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureFile {
    pub path: PathBuf,
    pub source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FixtureMarker {
    Point {
        path: PathBuf,
        position: SourcePosition,
    },
    Range {
        path: PathBuf,
        span: SourceSpan,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureExpectation {
    pub path: PathBuf,
    pub kind: String,
    pub value: String,
}

#[derive(Debug)]
pub struct FixtureTempDir {
    path: PathBuf,
}

impl Fixture {
    pub fn parse(input: &str) -> Self {
        Self::try_parse(input).expect("fixture should parse")
    }

    pub fn try_parse(input: &str) -> Result<Self, String> {
        let builders = split_file_builders(input);
        let mut files = Vec::new();
        let mut markers = BTreeMap::new();
        let mut expectations = Vec::new();

        for builder in builders {
            let path = builder.path;
            let (source, file_markers) = strip_markers(&path, &builder.source)?;

            for (name, marker) in file_markers {
                if markers.insert(name.clone(), marker).is_some() {
                    return Err(format!("duplicate fixture marker `{name}`"));
                }
            }

            expectations.extend(builder.expectations);
            files.push(FixtureFile { path, source });
        }

        Ok(Self {
            files,
            markers,
            expectations,
        })
    }

    pub fn files(&self) -> &[FixtureFile] {
        &self.files
    }

    pub fn expectations(&self) -> &[FixtureExpectation] {
        &self.expectations
    }

    pub fn file(&self, path: impl AsRef<Path>) -> Option<&FixtureFile> {
        let path = path.as_ref();

        self.files.iter().find(|file| file.path == path)
    }

    pub fn source(&self, path: impl AsRef<Path>) -> &str {
        self.file(path)
            .map(|file| file.source.as_str())
            .expect("fixture file should exist")
    }

    pub fn marker(&self, name: &str) -> &FixtureMarker {
        self.markers
            .get(name)
            .unwrap_or_else(|| panic!("fixture marker `{name}` should exist"))
    }

    pub fn position(&self, name: &str) -> SourcePosition {
        match self.marker(name) {
            FixtureMarker::Point { position, .. } => *position,
            FixtureMarker::Range { span, .. } => span.start,
        }
    }

    pub fn range(&self, name: &str) -> SourceSpan {
        match self.marker(name) {
            FixtureMarker::Point { position, .. } => SourceSpan {
                start: *position,
                end: *position,
            },
            FixtureMarker::Range { span, .. } => *span,
        }
    }

    pub fn marker_path(&self, name: &str) -> &Path {
        match self.marker(name) {
            FixtureMarker::Point { path, .. } | FixtureMarker::Range { path, .. } => path,
        }
    }

    pub fn expected_values(&self, kind: &str) -> Vec<&str> {
        self.expectations
            .iter()
            .filter(|expectation| expectation.kind == kind)
            .map(|expectation| expectation.value.as_str())
            .collect()
    }

    pub fn expected_list(&self, kind: &str) -> Vec<String> {
        self.expected_values(kind)
            .into_iter()
            .flat_map(|value| value.split(','))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }

    pub fn write_to(&self, root: impl AsRef<Path>) -> io::Result<()> {
        let root = root.as_ref();

        for file in &self.files {
            let path = root.join(&file.path);

            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(path, &file.source)?;
        }

        Ok(())
    }

    pub fn write_to_temp(&self, name: &str) -> io::Result<FixtureTempDir> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("tua-fixture-{name}-{}-{unique}", process::id()));

        fs::create_dir_all(&path)?;
        self.write_to(&path)?;

        Ok(FixtureTempDir { path })
    }
}

impl FixtureTempDir {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.path.join(path)
    }
}

impl Drop for FixtureTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug)]
struct FileBuilder {
    path: PathBuf,
    source: String,
    expectations: Vec<FixtureExpectation>,
}

impl FileBuilder {
    fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            source: String::new(),
            expectations: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.source.is_empty() && self.expectations.is_empty()
    }
}

fn split_file_builders(input: &str) -> Vec<FileBuilder> {
    let mut builders = Vec::new();
    let mut current = FileBuilder::new(DEFAULT_FIXTURE_PATH);

    for segment in input.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);

        if let Some(path) = file_header_path(line) {
            if !current.is_empty() {
                builders.push(current);
            }

            current = FileBuilder::new(path);
            continue;
        }

        if let Some((kind, value)) = expectation(line) {
            current.expectations.push(FixtureExpectation {
                path: current.path.clone(),
                kind,
                value,
            });

            if segment.ends_with('\n') {
                current.source.push('\n');
            }

            continue;
        }

        current.source.push_str(segment);
    }

    if !current.is_empty() || builders.is_empty() {
        builders.push(current);
    }

    builders
}

fn file_header_path(line: &str) -> Option<PathBuf> {
    let trimmed = line.trim();
    let path = trimmed
        .strip_prefix("-- file:")
        .or_else(|| trimmed.strip_prefix("# file:"))?
        .trim();

    if path.is_empty() {
        return None;
    }

    Some(PathBuf::from(path))
}

fn expectation(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("-- expect ")?;
    let (kind, value) = rest.split_once(':')?;
    let kind = kind.trim();

    if kind.is_empty() {
        return None;
    }

    Some((kind.to_owned(), value.trim().to_owned()))
}

fn strip_markers(
    path: &Path,
    marked_source: &str,
) -> Result<(String, BTreeMap<String, FixtureMarker>), String> {
    let mut source = String::new();
    let mut markers = BTreeMap::new();
    let mut pending = BTreeMap::new();
    let mut line = 1;
    let mut column = 1;
    let mut index = 0;

    while index < marked_source.len() {
        if marked_source[index..].starts_with("<|") {
            let Some(end) = marked_source[index + 2..].find("|>") else {
                return Err(format!("unterminated fixture marker in {}", path.display()));
            };
            let marker_end = index + 2 + end;
            let name = &marked_source[index + 2..marker_end];

            if let Some(name) = name.strip_prefix('/') {
                let Some(start) = pending.remove(name) else {
                    return Err(format!(
                        "closing fixture marker `{name}` has no opener in {}",
                        path.display()
                    ));
                };

                let span = SourceSpan {
                    start,
                    end: SourcePosition { line, column },
                };

                if markers
                    .insert(
                        name.to_owned(),
                        FixtureMarker::Range {
                            path: path.to_path_buf(),
                            span,
                        },
                    )
                    .is_some()
                {
                    return Err(format!("duplicate fixture marker `{name}`"));
                }
            } else {
                if name.trim().is_empty() {
                    return Err(format!("empty fixture marker in {}", path.display()));
                }

                if pending
                    .insert(name.to_owned(), SourcePosition { line, column })
                    .is_some()
                    || markers.contains_key(name)
                {
                    return Err(format!("duplicate fixture marker `{name}`"));
                }
            }

            index = marker_end + 2;
            continue;
        }

        let character = marked_source[index..]
            .chars()
            .next()
            .expect("character should exist");

        source.push(character);

        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }

        index += character.len_utf8();
    }

    for (name, position) in pending {
        markers.insert(
            name,
            FixtureMarker::Point {
                path: path.to_path_buf(),
                position,
            },
        );
    }

    Ok((source, markers))
}

#[cfg(test)]
mod tests;
