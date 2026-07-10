use std::path::{Path, PathBuf};

use crate::module_index::resolve_required_module;
use crate::source_text::{byte_for_column, column_for_byte};
use crate::{SourcePosition, SourceSpan};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleLink {
    pub span: SourceSpan,
    pub target: PathBuf,
}

pub fn module_links_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Vec<ModuleLink> {
    let path = path.as_ref();
    let source = source.into();

    require_module_literals(&source)
        .into_iter()
        .filter_map(|literal| {
            let target = resolve_required_module(path, &literal.module)?;

            Some(ModuleLink {
                span: literal.span,
                target,
            })
        })
        .collect()
}

pub(crate) fn require_module_at_position(source: &str, position: SourcePosition) -> Option<String> {
    require_module_literals(source)
        .into_iter()
        .find(|literal| contains_position(literal.span, position))
        .map(|literal| literal.module)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RequireModuleLiteral {
    pub module: String,
    pub span: SourceSpan,
}

pub(crate) fn require_module_literals(source: &str) -> Vec<RequireModuleLiteral> {
    let mut state = RequireScanState::Code;

    source
        .split('\n')
        .enumerate()
        .flat_map(|(line_index, line)| {
            let line_number = line_index + 1;
            let line = line.strip_suffix('\r').unwrap_or(line);

            require_module_literals_on_line(line, line_number, &mut state)
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RequireScanState {
    Code,
    LongString { close: Vec<u8> },
    BlockComment { close: Vec<u8> },
}

fn require_module_literals_on_line(
    line: &str,
    line_number: usize,
    state: &mut RequireScanState,
) -> Vec<RequireModuleLiteral> {
    let mut literals = Vec::new();
    let mut index = 0usize;
    let bytes = line.as_bytes();

    while index < line.len() {
        match state {
            RequireScanState::Code => {
                if line[index..].starts_with("--") {
                    if let Some((close, after_open)) = long_bracket_close(line, index + 2) {
                        *state = RequireScanState::BlockComment { close };
                        index = after_open;
                        continue;
                    }

                    break;
                }

                match bytes[index] {
                    b'\'' | b'"' => {
                        index = skip_short_string(line, index, bytes[index]);
                        continue;
                    }
                    b'[' => {
                        if let Some((close, after_open)) = long_bracket_close(line, index) {
                            *state = RequireScanState::LongString { close };
                            index = after_open;
                            continue;
                        }
                    }
                    _ => {}
                }

                if starts_require_at(line, index) {
                    if let Some(literal) = require_module_literal_after(line, line_number, index) {
                        index = (byte_for_column(line, literal.span.end.column) + 1).max(index + 1);
                        literals.push(literal);
                        continue;
                    }
                }

                index += char_len_at(line, index);
            }
            RequireScanState::LongString { close } | RequireScanState::BlockComment { close } => {
                if let Some(offset) = find_bytes(&bytes[index..], close) {
                    index += offset + close.len();
                    *state = RequireScanState::Code;
                } else {
                    break;
                }
            }
        }
    }

    literals
}

fn require_module_literal_after(
    line: &str,
    line_number: usize,
    require_start: usize,
) -> Option<RequireModuleLiteral> {
    let bytes = line.as_bytes();
    let mut index = require_start + "require".len();

    index = skip_spaces(line, index);
    if bytes.get(index) == Some(&b'(') {
        index += 1;
        index = skip_spaces(line, index);
    }

    let quote = *bytes.get(index)?;
    if quote != b'\'' && quote != b'"' {
        return None;
    }

    let module_start = index + 1;
    let quote_end = find_short_string_end(line, index, quote)?;

    Some(RequireModuleLiteral {
        module: line[module_start..quote_end].to_owned(),
        span: SourceSpan {
            start: SourcePosition {
                line: line_number,
                column: column_for_byte(line, module_start),
            },
            end: SourcePosition {
                line: line_number,
                column: column_for_byte(line, quote_end),
            },
        },
    })
}

fn starts_require_at(line: &str, index: usize) -> bool {
    if !line[index..].starts_with("require") {
        return false;
    }

    let before = index
        .checked_sub(1)
        .and_then(|previous| line.as_bytes().get(previous))
        .copied();
    let after = line.as_bytes().get(index + "require".len()).copied();

    !before.is_some_and(is_identifier_byte) && !after.is_some_and(is_identifier_byte)
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn skip_spaces(line: &str, mut index: usize) -> usize {
    while line
        .as_bytes()
        .get(index)
        .is_some_and(u8::is_ascii_whitespace)
    {
        index += 1;
    }

    index
}

fn skip_short_string(line: &str, start: usize, quote: u8) -> usize {
    find_short_string_end(line, start, quote)
        .map(|end| end + 1)
        .unwrap_or(line.len())
}

fn find_short_string_end(line: &str, start: usize, quote: u8) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut index = start + 1;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];

        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == quote {
            return Some(index);
        }

        index += 1;
    }

    None
}

fn long_bracket_close(line: &str, start: usize) -> Option<(Vec<u8>, usize)> {
    let bytes = line.as_bytes();
    if bytes.get(start) != Some(&b'[') {
        return None;
    }

    let mut index = start + 1;
    while bytes.get(index) == Some(&b'=') {
        index += 1;
    }

    if bytes.get(index) != Some(&b'[') {
        return None;
    }

    let mut close = Vec::with_capacity(index - start + 1);
    close.push(b']');
    for _ in 0..index - start - 1 {
        close.push(b'=');
    }
    close.push(b']');

    Some((close, index + 1))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn char_len_at(line: &str, index: usize) -> usize {
    line[index..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or(1)
}

fn contains_position(span: SourceSpan, position: SourcePosition) -> bool {
    position_key(span.start) <= position_key(position)
        && position_key(position) <= position_key(span.end)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_text::position_at;
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn finds_require_module_at_position() {
        let source = r#"local Player = require("player")
local Legacy = require('legacy')
"#;

        assert_eq!(
            require_module_at_position(source, position_at(source, "player")),
            Some("player".to_owned())
        );
        assert_eq!(
            require_module_at_position(source, position_at(source, "legacy")),
            Some("legacy".to_owned())
        );
    }

    #[test]
    fn require_literals_ignore_comments_and_strings() {
        let source = r#"-- require("commented")
local text = "require('stringed')"
--[[
require("block_comment")
]]
local long = [[require("long_string")]]
local Player = require("player")
local Legacy = require "legacy"
local First = require("first"); local Second = require("second")
"#;
        let modules = require_module_literals(source)
            .into_iter()
            .map(|literal| literal.module)
            .collect::<Vec<_>>();

        assert_eq!(modules, vec!["player", "legacy", "first", "second"]);
    }

    #[test]
    fn creates_document_links_for_resolved_require_modules() {
        let temp = TempProject::new("module-links");
        let source = r#"local Player = require("player")
local Legacy = require("legacy")
local Missing = require("missing")
"#;

        temp.write("tua.toml", "source = \"src\"\n");
        temp.write("src/main.tua", source);
        temp.write("src/player.tua", "return {}\n");
        temp.write("src/legacy.lua", "return {}\n");

        let links = module_links_tua_source(temp.path.join("src/main.tua"), source);

        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, temp.path.join("src/player.tua"));
        assert_eq!(links[1].target, temp.path.join("src/legacy.lua"));
    }

    struct TempProject {
        path: PathBuf,
    }

    impl TempProject {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("tua-core-{name}-{}-{unique}", process::id()));

            fs::create_dir_all(&path).expect("temp project should be created");

            Self { path }
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.path.join(relative);

            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent directory should be created");
            }

            fs::write(path, contents).expect("fixture should be written");
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
