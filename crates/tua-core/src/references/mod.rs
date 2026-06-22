use std::path::{Path, PathBuf};

use full_moon::ast;
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};

use crate::{
    definition_parsed_source, parse_tua_source, Definition, DefinitionKind, Diagnostic,
    ParsedSource, SourcePosition, SourceSpan,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reference {
    pub path: PathBuf,
    pub span: SourceSpan,
    pub kind: ReferenceKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceKind {
    Module,
    TypeAlias,
    Local,
    Function,
    Parameter,
    TableField,
}

pub fn references_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    position: SourcePosition,
    include_declaration: bool,
) -> Result<Vec<Reference>, Vec<Diagnostic>> {
    let parsed = parse_tua_source(path, source)?;

    references_parsed_source(&parsed, position, include_declaration)
}

pub fn references_parsed_source(
    parsed: &ParsedSource,
    position: SourcePosition,
    include_declaration: bool,
) -> Result<Vec<Reference>, Vec<Diagnostic>> {
    let Some(target) = definition_parsed_source(parsed, position)? else {
        return Ok(Vec::new());
    };

    references_to_definition_parsed_source(parsed, &target, include_declaration)
}

pub fn references_to_definition_parsed_source(
    parsed: &ParsedSource,
    target: &Definition,
    include_declaration: bool,
) -> Result<Vec<Reference>, Vec<Diagnostic>> {
    let mut references = parsed
        .ast
        .tokens()
        .filter(|token| is_identifier(token))
        .filter_map(|token| reference_for_token(parsed, token, target).transpose())
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;

    if include_declaration
        && target.kind == DefinitionKind::TableField
        && parsed.path == target.path
    {
        references.push(reference_from_definition(target));
    }

    if !include_declaration {
        references.retain(|reference| {
            reference.span != target.span || reference.kind != reference_kind(target.kind)
        });
    }

    references.sort_by_key(|reference| position_key(reference.span.start));
    references.dedup_by_key(|reference| {
        (
            reference.path.clone(),
            reference.span.start.line,
            reference.span.start.column,
            reference.span.end.line,
            reference.span.end.column,
            reference.kind,
        )
    });

    Ok(references)
}

pub fn global_function_name_for_definition(
    parsed: &ParsedSource,
    target: &Definition,
) -> Option<String> {
    if target.path != parsed.path || target.kind != DefinitionKind::Function {
        return None;
    }

    parsed.ast.nodes().stmts().find_map(|stmt| {
        let ast::Stmt::FunctionDeclaration(function) = stmt else {
            return None;
        };
        let span = node_span(function.name())?;

        if span != target.span {
            return None;
        }

        let name = function.name().to_string().trim().to_owned();

        (!name.contains('.') && !name.contains(':')).then_some(name)
    })
}

pub fn global_references_to_definition_parsed_source(
    parsed: &ParsedSource,
    target: &Definition,
    name: &str,
    include_declaration: bool,
) -> Result<Vec<Reference>, Vec<Diagnostic>> {
    let mut references = parsed
        .ast
        .tokens()
        .filter(|token| is_identifier(token))
        .filter(|token| token_text(token) == name)
        .filter_map(|token| global_reference_for_token(parsed, token, target).transpose())
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;

    if !include_declaration {
        references.retain(|reference| reference.span != target.span);
    }

    references.sort_by_key(|reference| position_key(reference.span.start));
    references.dedup_by_key(|reference| {
        (
            reference.path.clone(),
            reference.span.start.line,
            reference.span.start.column,
            reference.span.end.line,
            reference.span.end.column,
            reference.kind,
        )
    });

    Ok(references)
}

fn reference_for_token(
    parsed: &ParsedSource,
    token: &TokenReference,
    target: &Definition,
) -> Result<Option<Reference>, Vec<Diagnostic>> {
    let Some(span) = node_span(token) else {
        return Ok(None);
    };
    let Some(definition) = definition_parsed_source(parsed, span.start)? else {
        return Ok(None);
    };

    Ok(same_definition(&definition, target).then(|| Reference {
        path: parsed.path.clone(),
        span,
        kind: reference_kind(definition.kind),
    }))
}

fn global_reference_for_token(
    parsed: &ParsedSource,
    token: &TokenReference,
    target: &Definition,
) -> Result<Option<Reference>, Vec<Diagnostic>> {
    let Some(span) = node_span(token) else {
        return Ok(None);
    };
    let definition = definition_parsed_source(parsed, span.start)?;

    if definition
        .as_ref()
        .is_some_and(|definition| !same_definition(definition, target))
    {
        return Ok(None);
    }

    Ok(Some(Reference {
        path: parsed.path.clone(),
        span,
        kind: ReferenceKind::Function,
    }))
}

fn reference_from_definition(definition: &Definition) -> Reference {
    Reference {
        path: definition.path.clone(),
        span: definition.span,
        kind: reference_kind(definition.kind),
    }
}

fn same_definition(left: &Definition, right: &Definition) -> bool {
    left.path == right.path && left.span == right.span && left.kind == right.kind
}

fn reference_kind(kind: DefinitionKind) -> ReferenceKind {
    match kind {
        DefinitionKind::Module => ReferenceKind::Module,
        DefinitionKind::TypeAlias => ReferenceKind::TypeAlias,
        DefinitionKind::Local => ReferenceKind::Local,
        DefinitionKind::Function => ReferenceKind::Function,
        DefinitionKind::Parameter => ReferenceKind::Parameter,
        DefinitionKind::TableField => ReferenceKind::TableField,
    }
}

fn is_identifier(token: &TokenReference) -> bool {
    matches!(token.token().token_type(), TokenType::Identifier { .. })
}

fn token_text(token: &TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests;
