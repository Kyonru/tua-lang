use std::ops::Range;

use full_moon::ast::{self, LuaVersion};
use full_moon::node::Node;
use full_moon::tokenizer::{Lexer, Position, Token, TokenKind, TokenReference};

use crate::classes;
use crate::parser::ParserLineProvenance;
use crate::{ParsedSource, SourcePosition, SourceSpan};

mod operation;

pub(crate) use operation::{LoweringOperation, LoweringOperationError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoweringKind {
    Source,
    DefaultParameter,
    BuiltinCall,
    Import,
    RuntimeTypeCheck,
    Class,
    FinalClassCall,
    CompoundAssignment,
    Sprite,
    Annotation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LuaLineProvenance {
    pub generated_line: usize,
    pub source_span: SourceSpan,
    pub lowering: LoweringKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaEmission {
    pub code: String,
    pub provenance: Vec<LuaLineProvenance>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoweredLine {
    text: String,
    source_span: SourceSpan,
    lowering: LoweringKind,
}

#[derive(Clone, Debug)]
pub(crate) struct LoweringDocument {
    lines: Vec<LoweredLine>,
    fallback_span: SourceSpan,
}

impl LoweringDocument {
    fn from_source(source: &str) -> Self {
        let fallback_span = line_span(source, 1);
        let lines = split_lines(source)
            .into_iter()
            .enumerate()
            .map(|(index, text)| LoweredLine {
                text: text.to_owned(),
                source_span: line_span(source, index + 1),
                lowering: LoweringKind::Source,
            })
            .collect();

        Self {
            lines,
            fallback_span,
        }
    }

    pub(crate) fn from_erased_ast(ast: &ast::Ast, parsed: &ParsedSource) -> Self {
        let fallback_span = parsed.span.unwrap_or_else(|| line_span(&parsed.source, 1));
        let output = ast.to_string();
        let mut syntax = surviving_syntax_tokens(ast, parsed).into_iter().peekable();
        let mut document = Self {
            lines: Vec::new(),
            fallback_span,
        };

        let lua_version = match parsed.raw.language {
            crate::ParsedLanguage::Lua => LuaVersion::lua51(),
            crate::ParsedLanguage::Tua => LuaVersion::luau(),
        };
        let output_tokens = Lexer::new(&output, lua_version).collect().unwrap();

        for token in output_tokens {
            let text = token.to_string();
            if text.is_empty() {
                continue;
            }

            let matched_origin = syntax.peek().and_then(|candidate| {
                (candidate.kind == token.token_kind() && candidate.text == text)
                    .then_some((candidate.source_span, candidate.lowering))
            });
            if matched_origin.is_some() {
                syntax.next();
            }
            let origin = (token.token_kind() != TokenKind::Whitespace)
                .then_some(matched_origin)
                .flatten();
            document.push_syntax_text(&text, origin);
        }

        debug_assert_eq!(document.code(), output);
        document
    }

    #[cfg(test)]
    pub(crate) fn from_erased_lua(lua: String, parsed: &ParsedSource) -> Self {
        let fallback_span = parsed.span.unwrap_or_else(|| line_span(&parsed.source, 1));
        let lines = split_lines(&lua)
            .into_iter()
            .enumerate()
            .map(|(index, text)| LoweredLine {
                text: text.to_owned(),
                source_span: line_span(&parsed.source, index + 1),
                lowering: LoweringKind::Source,
            })
            .collect();

        Self {
            lines,
            fallback_span,
        }
    }

    pub(crate) fn code(&self) -> String {
        self.lines.iter().map(|line| line.text.as_str()).collect()
    }

    pub(crate) fn operation(
        &self,
        lowering: LoweringKind,
        range: Range<usize>,
        replacement: impl Into<String>,
    ) -> LoweringOperation {
        let source_span = self.source_span_at(range.start);
        LoweringOperation::replace(range, replacement, source_span, lowering)
    }

    pub(crate) fn source_span_at(&self, offset: usize) -> SourceSpan {
        line_at_offset(&self.lines, offset)
            .map(|line| line.source_span)
            .unwrap_or(self.fallback_span)
    }

    pub(crate) fn apply_operations(
        &mut self,
        operations: Vec<LoweringOperation>,
    ) -> Result<(), LoweringOperationError> {
        if operations.is_empty() {
            return Ok(());
        }

        let code = self.code();
        self.lines = operation::apply_operations(&self.lines, &code, operations)?;
        Ok(())
    }

    fn push_syntax_text(&mut self, text: &str, origin: Option<(SourceSpan, LoweringKind)>) {
        let inherited = self
            .lines
            .last()
            .map(|line| (line.source_span, line.lowering))
            .unwrap_or((self.fallback_span, LoweringKind::Source));
        let (source_span, lowering) = origin.unwrap_or(inherited);

        for piece in split_lines(text) {
            if let Some(line) = self
                .lines
                .last_mut()
                .filter(|line| !line.text.ends_with('\n'))
            {
                line.text.push_str(piece);
                if origin.is_some() {
                    line.source_span = source_span;
                    line.lowering = lowering;
                }
            } else {
                self.lines.push(LoweredLine {
                    text: piece.to_owned(),
                    source_span,
                    lowering,
                });
            }
        }
    }

    pub(crate) fn finish(self) -> LuaEmission {
        let raw = self.code();
        let trimmed = raw.trim_matches(char::is_whitespace);
        if trimmed.is_empty() {
            return LuaEmission {
                code: String::new(),
                provenance: Vec::new(),
            };
        }

        let leading = raw.len() - raw.trim_start_matches(char::is_whitespace).len();
        let code = format!("{trimmed}\n");
        let mut provenance = Vec::new();
        let mut raw_offset = leading;

        for (index, line) in split_lines(&code).into_iter().enumerate() {
            let lowered = line_at_offset(&self.lines, raw_offset).unwrap_or_else(|| {
                self.lines
                    .last()
                    .expect("non-empty lowering output should retain one provenance line")
            });

            provenance.push(LuaLineProvenance {
                generated_line: index + 1,
                source_span: lowered.source_span,
                lowering: lowered.lowering,
            });
            raw_offset += line.len();
        }

        LuaEmission { code, provenance }
    }

    fn parser_provenance(&self) -> Vec<ParserLineProvenance> {
        self.lines
            .iter()
            .map(|line| ParserLineProvenance {
                source_span: line.source_span,
                class_generated: line.lowering == LoweringKind::Class,
            })
            .collect()
    }
}

pub(crate) fn lower_classes_for_parser(source: &str) -> (String, Vec<ParserLineProvenance>) {
    let mut document = LoweringDocument::from_source(source);
    let operations = classes::class_lowering_edits(source)
        .into_iter()
        .map(|edit| {
            LoweringOperation::replace(
                edit.range,
                edit.replacement,
                edit.source_span,
                LoweringKind::Class,
            )
        })
        .collect();

    document
        .apply_operations(operations)
        .expect("class parser lowering edits should be valid and non-overlapping");

    let provenance = document.parser_provenance();
    (document.code(), provenance)
}

#[derive(Clone, Debug)]
struct SyntaxTokenOrigin {
    start: usize,
    end: usize,
    text: String,
    kind: TokenKind,
    source_span: SourceSpan,
    lowering: LoweringKind,
}

fn surviving_syntax_tokens(ast: &ast::Ast, parsed: &ParsedSource) -> Vec<SyntaxTokenOrigin> {
    let mut tokens = Vec::new();

    for reference in ast.tokens().chain(std::iter::once(ast.eof())) {
        collect_reference_origins(reference, parsed, &mut tokens);
    }

    tokens.sort_by_key(|token| (token.start, token.end));
    tokens.dedup_by(|left, right| {
        left.start == right.start
            && left.end == right.end
            && left.kind == right.kind
            && left.text == right.text
    });
    tokens
}

fn collect_reference_origins(
    reference: &TokenReference,
    parsed: &ParsedSource,
    output: &mut Vec<SyntaxTokenOrigin>,
) {
    for token in reference
        .leading_trivia()
        .chain(std::iter::once(reference.token()))
        .chain(reference.trailing_trivia())
    {
        let Some((source_span, lowering)) = positioned_token_origin(token, parsed) else {
            continue;
        };
        let text = token.to_string();
        if text.is_empty() {
            continue;
        }

        output.push(SyntaxTokenOrigin {
            start: token.start_position().bytes(),
            end: token.end_position().bytes(),
            text,
            kind: token.token_kind(),
            source_span,
            lowering,
        });
    }
}

fn positioned_token_origin(
    token: &Token,
    parsed: &ParsedSource,
) -> Option<(SourceSpan, LoweringKind)> {
    let start = token.start_position();
    let end = token.end_position();
    let text = token.to_string();
    let positioned_text = parsed.raw.parser_source.get(start.bytes()..end.bytes())?;

    if positioned_text != text {
        return None;
    }

    parser_range_origin(parsed, start, end)
}

fn parser_range_origin(
    parsed: &ParsedSource,
    start: Position,
    end: Position,
) -> Option<(SourceSpan, LoweringKind)> {
    let start_index = start.line().max(1) - 1;
    let end_index = end.line().max(1) - 1;
    let start_line = parsed.raw.parser_provenance.get(start_index)?;
    let end_line = parsed
        .raw
        .parser_provenance
        .get(end_index)
        .unwrap_or(start_line);
    let class_generated = start_line.class_generated || end_line.class_generated;

    if class_generated {
        return Some((start_line.source_span, LoweringKind::Class));
    }

    let source_span = SourceSpan {
        start: start_line.source_span.start,
        end: end_line.source_span.end,
    };

    Some((source_span, LoweringKind::Source))
}

fn line_at_offset(lines: &[LoweredLine], offset: usize) -> Option<&LoweredLine> {
    let mut cursor = 0usize;

    for line in lines {
        let end = cursor + line.text.len();
        if offset < end || (line.text.is_empty() && offset == cursor) {
            return Some(line);
        }
        cursor = end;
    }

    lines.last()
}

fn split_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        Vec::new()
    } else {
        text.split_inclusive('\n').collect()
    }
}

fn line_span(source: &str, line: usize) -> SourceSpan {
    let text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
    SourceSpan {
        start: SourcePosition { line, column: 1 },
        end: SourcePosition {
            line,
            column: text.chars().count() + 1,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_tua_source;

    #[test]
    fn operation_preserves_unchanged_lines_and_marks_generated_lines() {
        let parsed = parse_tua_source(
            "src/main.tua",
            "local value: number = 1\nvalue += 2\nprint(value)\n",
        )
        .expect("fixture should parse");
        let mut document = LoweringDocument::from_erased_lua(
            "local value = 1\nvalue += 2\nprint(value)\n".to_owned(),
            &parsed,
        );

        let code = document.code();
        let start = code.find("value += 2").expect("assignment should exist");
        let operation = document.operation(
            LoweringKind::CompoundAssignment,
            start..start + "value += 2".len(),
            "value = value + 2",
        );
        document
            .apply_operations(vec![operation])
            .expect("operation should apply");
        let emission = document.finish();

        assert_eq!(emission.provenance[0].lowering, LoweringKind::Source);
        assert_eq!(
            emission.provenance[1].lowering,
            LoweringKind::CompoundAssignment
        );
        assert_eq!(emission.provenance[1].source_span.start.line, 2);
        assert_eq!(emission.provenance[2].lowering, LoweringKind::Source);
    }

    #[test]
    fn operation_maps_inserted_lines_to_explicit_source_span() {
        let parsed = parse_tua_source(
            "src/main.tua",
            "local function greet(name: string = \"Ada\")\n  return name\nend\n",
        )
        .expect("fixture should parse");
        let mut document = LoweringDocument::from_erased_lua(
            "local function greet(name)\n  return name\nend\n".to_owned(),
            &parsed,
        );

        let insert_at = document
            .code()
            .find('\n')
            .map(|offset| offset + 1)
            .expect("function header should end with a newline");
        let source_span = parsed.function_annotations[0].default_parameters[0]
            .span
            .expect("default should have a source span");
        document
            .apply_operations(vec![LoweringOperation::insert(
                insert_at,
                "  if name == nil then\n    name = \"Ada\"\n  end\n",
                source_span,
                LoweringKind::DefaultParameter,
            )])
            .expect("operation should apply");
        let emission = document.finish();

        assert!(emission.provenance[1..4]
            .iter()
            .all(|line| line.lowering == LoweringKind::DefaultParameter));
        assert!(emission.provenance[1..4]
            .iter()
            .all(|line| line.source_span.start.line == 1));
    }

    #[test]
    fn operation_preserves_following_source_after_large_expansion() {
        let source = "local before = 1\nlocal after = 2\n";
        let parsed = parse_tua_source("src/main.tua", source).expect("fixture should parse");
        let mut document = LoweringDocument::from_erased_lua(source.to_owned(), &parsed);
        let mut expanded = String::new();

        for index in 0..80 {
            expanded.push_str(&format!("-- generated {index}\n"));
        }

        let insert_at = "local before = 1\n".len();
        let source_span = document.source_span_at(insert_at);
        document
            .apply_operations(vec![LoweringOperation::insert(
                insert_at,
                expanded,
                source_span,
                LoweringKind::Annotation,
            )])
            .expect("large insertion should apply");
        let emission = document.finish();

        assert_eq!(emission.provenance.len(), 82);
        assert!(emission.provenance[1..81]
            .iter()
            .all(|line| line.lowering == LoweringKind::Annotation));
        assert_eq!(emission.provenance[81].lowering, LoweringKind::Source);
        assert_eq!(emission.provenance[81].source_span.start.line, 2);
    }

    #[test]
    fn operations_preserve_multiline_and_non_ascii_provenance() {
        let source = "local text = \"Olá\"\nvalue += 1\n";
        let parsed = parse_tua_source("src/main.tua", source).expect("fixture should parse");
        let mut document = LoweringDocument::from_erased_lua(source.to_owned(), &parsed);
        let start = source.find("value += 1").expect("assignment should exist");
        let operation = document.operation(
            LoweringKind::CompoundAssignment,
            start..start + "value += 1".len(),
            "value = value + 1\nprint(\"✓\")",
        );

        document
            .apply_operations(vec![operation])
            .expect("multiline operation should apply");
        let emission = document.finish();

        assert_eq!(
            emission.code,
            "local text = \"Olá\"\nvalue = value + 1\nprint(\"✓\")\n"
        );
        assert_eq!(emission.provenance[0].lowering, LoweringKind::Source);
        assert!(emission.provenance[1..]
            .iter()
            .all(|line| line.lowering == LoweringKind::CompoundAssignment));
        assert!(emission.provenance[1..]
            .iter()
            .all(|line| line.source_span.start.line == 2));
    }

    #[test]
    fn operations_reject_overlaps_and_utf8_splits() {
        let source = "local text = \"Olá\"\n";
        let parsed = parse_tua_source("src/main.tua", source).expect("fixture should parse");
        let mut document = LoweringDocument::from_erased_lua(source.to_owned(), &parsed);
        let source_span = document.source_span_at(0);
        let overlap = document
            .apply_operations(vec![
                LoweringOperation::replace(6..12, "first", source_span, LoweringKind::BuiltinCall),
                LoweringOperation::replace(10..15, "second", source_span, LoweringKind::Import),
            ])
            .expect_err("overlapping operations should fail");
        assert!(matches!(overlap, LoweringOperationError::Overlap { .. }));

        let non_ascii = source
            .find('á')
            .expect("fixture should contain non-ASCII text");
        let utf8_split = document
            .apply_operations(vec![LoweringOperation::insert(
                non_ascii + 1,
                "invalid",
                source_span,
                LoweringKind::Annotation,
            )])
            .expect_err("UTF-8 split should fail");
        assert_eq!(
            utf8_split,
            LoweringOperationError::InvalidUtf8Boundary {
                offset: non_ascii + 1
            }
        );
    }

    #[test]
    fn same_offset_insertions_keep_declared_order() {
        let source = "local first = 1\nlocal second = 2\n";
        let parsed = parse_tua_source("src/main.tua", source).expect("fixture should parse");
        let mut document = LoweringDocument::from_erased_lua(source.to_owned(), &parsed);
        let first_span = document.source_span_at(0);
        let second_span = document.source_span_at("local first = 1\n".len());

        document
            .apply_operations(vec![
                LoweringOperation::insert(0, "-- first\n", first_span, LoweringKind::BuiltinCall),
                LoweringOperation::insert(0, "-- second\n", second_span, LoweringKind::Import),
            ])
            .expect("insertions should apply");
        let emission = document.finish();

        assert!(emission.code.starts_with("-- first\n-- second\n"));
        assert_eq!(emission.provenance[0].source_span.start.line, 1);
        assert_eq!(emission.provenance[1].source_span.start.line, 2);
    }
}
