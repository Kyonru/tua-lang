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
struct RequireModuleLiteral {
    module: String,
    span: SourceSpan,
}

fn require_module_literals(source: &str) -> Vec<RequireModuleLiteral> {
    source
        .split('\n')
        .enumerate()
        .flat_map(|(line_index, line)| {
            let line_number = line_index + 1;
            let line = line.strip_suffix('\r').unwrap_or(line);

            require_module_literals_on_line(line, line_number)
        })
        .collect()
}

fn require_module_literals_on_line(line: &str, line_number: usize) -> Vec<RequireModuleLiteral> {
    let mut literals = Vec::new();
    let mut search_from = 0usize;

    while let Some(offset) = line[search_from..].find("require") {
        let require_start = search_from + offset;
        let Some(literal) = require_module_literal_after(line, line_number, require_start) else {
            search_from = require_start + "require".len();
            continue;
        };

        search_from = byte_for_column(line, literal.span.end.column).max(require_start + 1);
        literals.push(literal);
    }

    literals
}

fn require_module_literal_after(
    line: &str,
    line_number: usize,
    require_start: usize,
) -> Option<RequireModuleLiteral> {
    let after_require = require_start + "require".len();
    let open_paren = line[after_require..].find('(')? + after_require;
    let quote_start = line[open_paren + 1..].find(['"', '\''])? + open_paren + 1;
    let quote = line.as_bytes()[quote_start] as char;
    let module_start = quote_start + 1;
    let quote_end = line[module_start..].find(quote)? + module_start;

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
