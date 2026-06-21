use std::path::{Path, PathBuf};

use crate::source_text::{byte_for_column, is_identifier_continue_char, line_at};
use crate::{
    definition_parsed_source, parse_tlua_source, references_parsed_source, DefinitionKind,
    Diagnostic, ParsedSource, ReferenceKind, SourcePosition, SourceSpan,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenameTarget {
    pub path: PathBuf,
    pub span: SourceSpan,
    pub placeholder: String,
    pub kind: RenameKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenameKind {
    TypeAlias,
    Local,
    Function,
    Parameter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenameEdit {
    pub edits: Vec<TextEdit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextEdit {
    pub path: PathBuf,
    pub span: SourceSpan,
    pub new_text: String,
}

pub fn prepare_rename_tlua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    position: SourcePosition,
) -> Result<Option<RenameTarget>, Vec<Diagnostic>> {
    let parsed = parse_tlua_source(path, source)?;

    prepare_rename_parsed_source(&parsed, position)
}

pub fn prepare_rename_parsed_source(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Result<Option<RenameTarget>, Vec<Diagnostic>> {
    let Some(definition) = definition_parsed_source(parsed, position)? else {
        return Ok(None);
    };
    let Some(kind) = rename_kind(definition.kind) else {
        return Ok(None);
    };
    let Some(placeholder) = text_for_span(&parsed.source, definition.span) else {
        return Ok(None);
    };

    Ok(Some(RenameTarget {
        path: definition.path,
        span: definition.span,
        placeholder: placeholder.to_owned(),
        kind,
    }))
}

pub fn rename_tlua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    position: SourcePosition,
    new_name: impl Into<String>,
) -> Result<Option<RenameEdit>, Vec<Diagnostic>> {
    let parsed = parse_tlua_source(path, source)?;

    rename_parsed_source(&parsed, position, new_name)
}

pub fn rename_parsed_source(
    parsed: &ParsedSource,
    position: SourcePosition,
    new_name: impl Into<String>,
) -> Result<Option<RenameEdit>, Vec<Diagnostic>> {
    let new_name = new_name.into();

    if !is_valid_rename_identifier(&new_name) {
        return Ok(None);
    }

    let Some(target) = prepare_rename_parsed_source(parsed, position)? else {
        return Ok(None);
    };
    let reference_kind = reference_kind(target.kind);
    let edits = references_parsed_source(parsed, position, true)?
        .into_iter()
        .filter(|reference| reference.kind == reference_kind)
        .map(|reference| TextEdit {
            path: reference.path,
            span: reference.span,
            new_text: new_name.clone(),
        })
        .collect::<Vec<_>>();

    Ok((!edits.is_empty()).then_some(RenameEdit { edits }))
}

fn rename_kind(kind: DefinitionKind) -> Option<RenameKind> {
    match kind {
        DefinitionKind::TypeAlias => Some(RenameKind::TypeAlias),
        DefinitionKind::Local => Some(RenameKind::Local),
        DefinitionKind::Function => Some(RenameKind::Function),
        DefinitionKind::Parameter => Some(RenameKind::Parameter),
        DefinitionKind::TableField => None,
    }
}

fn reference_kind(kind: RenameKind) -> ReferenceKind {
    match kind {
        RenameKind::TypeAlias => ReferenceKind::TypeAlias,
        RenameKind::Local => ReferenceKind::Local,
        RenameKind::Function => ReferenceKind::Function,
        RenameKind::Parameter => ReferenceKind::Parameter,
    }
}

fn is_valid_rename_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    is_identifier_start_char(first)
        && chars.all(is_identifier_continue_char)
        && !is_reserved_word(name)
}

fn is_identifier_start_char(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_reserved_word(name: &str) -> bool {
    matches!(
        name,
        "and"
            | "break"
            | "do"
            | "else"
            | "elseif"
            | "end"
            | "false"
            | "for"
            | "function"
            | "if"
            | "in"
            | "local"
            | "nil"
            | "not"
            | "or"
            | "repeat"
            | "return"
            | "then"
            | "true"
            | "type"
            | "until"
            | "while"
    )
}

fn text_for_span(source: &str, span: SourceSpan) -> Option<&str> {
    if span.start.line != span.end.line {
        return None;
    }

    let line = line_at(source, span.start.line)?;
    let start = byte_for_column(line, span.start.column);
    let end = byte_for_column(line, span.end.column);

    line.get(start..end)
}

#[cfg(test)]
mod tests;
