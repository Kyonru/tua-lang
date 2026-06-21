use std::path::{Path, PathBuf};

use full_moon::node::Node;
use full_moon::tokenizer::Position;

use crate::{Diagnostic, DiagnosticCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

impl SourcePosition {
    fn from_full_moon(position: Position) -> Self {
        Self {
            line: position.line().max(1),
            column: position.character().max(1),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

impl SourceSpan {
    fn from_range(start: Position, end: Position) -> Self {
        Self {
            start: SourcePosition::from_full_moon(start),
            end: SourcePosition::from_full_moon(end),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParsedSource {
    pub path: PathBuf,
    pub source: String,
    pub ast: full_moon::ast::Ast,
    pub span: Option<SourceSpan>,
}

impl ParsedSource {
    pub fn statement_count(&self) -> usize {
        self.ast.nodes().stmts().count()
    }

    pub fn has_return(&self) -> bool {
        self.ast.nodes().last_stmt().is_some()
    }
}

pub fn parse_lua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    let path = path.as_ref().to_path_buf();
    let source = source.into();

    match full_moon::parse(&source) {
        Ok(ast) => {
            let span = ast
                .range()
                .map(|(start, end)| SourceSpan::from_range(start, end));

            Ok(ParsedSource {
                path,
                source,
                ast,
                span,
            })
        }
        Err(errors) => Err(errors
            .into_iter()
            .map(|error| parser_error_to_diagnostic(&path, error))
            .collect()),
    }
}

fn parser_error_to_diagnostic(path: &Path, error: full_moon::Error) -> Diagnostic {
    let (start, _) = error.range();
    let position = SourcePosition::from_full_moon(start);

    Diagnostic::error(
        path,
        position.line,
        position.column,
        error.error_message().into_owned(),
    )
    .with_code(DiagnosticCode::SYNTAX_PARSE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_assignments() {
        let parsed = parse_lua_source("src/local.tlua", "local x = 1\nlocal y = x + 2\n")
            .expect("locals should parse");

        assert_eq!(parsed.path, PathBuf::from("src/local.tlua"));
        assert_eq!(parsed.statement_count(), 2);
        assert!(parsed.span.is_some());
    }

    #[test]
    fn parses_functions_and_returns() {
        let parsed = parse_lua_source(
            "src/function.tlua",
            r#"
local function add(a, b)
  return a + b
end
"#,
        )
        .expect("function should parse");

        assert_eq!(parsed.statement_count(), 1);
        assert!(parsed.source.contains("return a + b"));
    }

    #[test]
    fn parses_table_constructors() {
        let parsed = parse_lua_source(
            "src/table.tlua",
            r#"
local player = {
  hp = 100,
  alive = true,
}
"#,
        )
        .expect("table should parse");

        assert_eq!(parsed.statement_count(), 1);
        assert!(parsed.source.contains("alive"));
    }

    #[test]
    fn parses_return_statements() {
        let parsed = parse_lua_source("src/return.tlua", "return { ok = true }\n")
            .expect("return should parse");

        assert_eq!(parsed.statement_count(), 0);
        assert!(parsed.has_return());
    }

    #[test]
    fn parses_require_calls() {
        let parsed = parse_lua_source(
            "src/require.tlua",
            r#"
local Player = require("player")
return Player
"#,
        )
        .expect("require should parse");

        assert_eq!(parsed.statement_count(), 1);
        assert!(parsed.has_return());
        assert!(parsed.source.contains("require"));
    }

    #[test]
    fn syntax_errors_produce_tl000x_diagnostics_with_locations() {
        let diagnostics = parse_lua_source("src/broken.tlua", "local x = \n")
            .expect_err("invalid Lua should fail");

        assert!(!diagnostics.is_empty());

        let first = &diagnostics[0];

        assert_eq!(first.file, PathBuf::from("src/broken.tlua"));
        assert_eq!(first.severity, crate::DiagnosticSeverity::Error);
        assert_eq!(first.code, Some(DiagnosticCode::SYNTAX_PARSE));
        assert!(first.line >= 1);
        assert!(first.column >= 1);
        assert!(first.format_cli().contains("error TL0001"));
    }
}
