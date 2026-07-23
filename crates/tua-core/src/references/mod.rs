use std::path::{Path, PathBuf};

use full_moon::ast;
use full_moon::node::Node;

use crate::hir::{HirExportValue, HirModule};
use crate::source_text::{byte_for_column, line_at, source_identifiers, SourceIdentifier};
use crate::{
    definition_parsed_source, definition_parsed_source_with_hir, lower_parsed_source,
    parse_tua_source, scope_map_parsed_source, Definition, DefinitionKind, Diagnostic,
    ParsedSource, ScopeDeclarationKind, ScopeMap, SemanticIndex, SourcePosition, SourceSpan,
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
    Asset,
    SignalEvent,
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
    references_to_definition_parsed_source_with_artifacts(
        parsed,
        target,
        include_declaration,
        None,
        None,
        None,
    )
}

pub fn references_to_definition_parsed_source_with_artifacts(
    parsed: &ParsedSource,
    target: &Definition,
    include_declaration: bool,
    hir: Option<&HirModule>,
    scope: Option<&ScopeMap>,
    semantic_index: Option<&SemanticIndex>,
) -> Result<Vec<Reference>, Vec<Diagnostic>> {
    if target.kind == DefinitionKind::Builtin {
        return Ok(Vec::new());
    }

    if let Some(references) =
        scope_references_to_definition(parsed, target, include_declaration, hir, scope)
    {
        return Ok(references);
    }

    let mut references = source_identifiers(&parsed.source)
        .into_iter()
        .filter_map(|identifier| {
            reference_for_identifier(parsed, &identifier, target, hir, semantic_index).transpose()
        })
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

fn scope_references_to_definition(
    parsed: &ParsedSource,
    target: &Definition,
    include_declaration: bool,
    hir: Option<&HirModule>,
    scope: Option<&ScopeMap>,
) -> Option<Vec<Reference>> {
    if target.path != parsed.path {
        return None;
    }

    let expected_kind = scope_kind_for_definition(target.kind)?;
    if let Some(scope) = scope {
        return scope_references_to_definition_with_scope(
            parsed,
            target,
            include_declaration,
            hir,
            scope,
            expected_kind,
        );
    }

    let scope = scope_map_parsed_source(parsed);

    scope_references_to_definition_with_scope(
        parsed,
        target,
        include_declaration,
        hir,
        &scope,
        expected_kind,
    )
}

fn scope_references_to_definition_with_scope(
    parsed: &ParsedSource,
    target: &Definition,
    include_declaration: bool,
    hir: Option<&HirModule>,
    scope: &ScopeMap,
    expected_kind: ScopeDeclarationKind,
) -> Option<Vec<Reference>> {
    let declaration = scope.declarations().iter().find(|declaration| {
        declaration.name_span == target.span && declaration.kind == expected_kind
    })?;
    let mut references = scope
        .references_to_symbol(declaration.id, include_declaration)
        .into_iter()
        .map(|reference| Reference {
            path: parsed.path.clone(),
            span: reference.span,
            kind: reference_kind(target.kind),
        })
        .collect::<Vec<_>>();

    references.extend(export_key_references(parsed, target, hir));
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

    Some(references)
}

fn export_key_references(
    parsed: &ParsedSource,
    target: &Definition,
    hir: Option<&HirModule>,
) -> Vec<Reference> {
    if let Some(hir) = hir {
        return export_key_references_with_hir(parsed, target, hir);
    }

    let hir = lower_parsed_source(parsed);

    export_key_references_with_hir(parsed, target, &hir)
}

fn export_key_references_with_hir(
    parsed: &ParsedSource,
    target: &Definition,
    hir: &HirModule,
) -> Vec<Reference> {
    let Some(hir_symbol) = hir
        .symbols()
        .iter()
        .find(|symbol| symbol.name_span == target.span)
    else {
        return Vec::new();
    };

    hir.exports
        .iter()
        .filter(|export| export.value == HirExportValue::Symbol(hir_symbol.id))
        .map(|export| Reference {
            path: parsed.path.clone(),
            span: export.name_span,
            kind: reference_kind(target.kind),
        })
        .collect()
}

fn scope_kind_for_definition(kind: DefinitionKind) -> Option<ScopeDeclarationKind> {
    match kind {
        DefinitionKind::Local => Some(ScopeDeclarationKind::Local),
        DefinitionKind::Function => Some(ScopeDeclarationKind::Function),
        DefinitionKind::Parameter => Some(ScopeDeclarationKind::Parameter),
        DefinitionKind::Module
        | DefinitionKind::Builtin
        | DefinitionKind::Asset
        | DefinitionKind::SignalEvent
        | DefinitionKind::TypeAlias
        | DefinitionKind::TableField => None,
    }
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

pub fn global_type_alias_name_for_definition(
    parsed: &ParsedSource,
    target: &Definition,
) -> Option<String> {
    if target.path != parsed.path || target.kind != DefinitionKind::TypeAlias {
        return None;
    }

    parsed.type_aliases.iter().find_map(|alias| {
        let span = alias.name_span.or(alias.span)?;

        (span == target.span).then(|| alias.name.clone())
    })
}

pub fn global_function_definition_count(parsed: &ParsedSource, name: &str) -> usize {
    parsed
        .ast
        .nodes()
        .stmts()
        .filter_map(|stmt| {
            let ast::Stmt::FunctionDeclaration(function) = stmt else {
                return None;
            };

            Some(function.name().to_string().trim().to_owned())
        })
        .filter(|function_name| {
            function_name == name && !function_name.contains('.') && !function_name.contains(':')
        })
        .count()
}

pub fn type_alias_definition_count(parsed: &ParsedSource, name: &str) -> usize {
    parsed
        .type_aliases
        .iter()
        .filter(|alias| alias.name == name)
        .count()
}

pub fn global_references_to_definition_parsed_source(
    parsed: &ParsedSource,
    target: &Definition,
    name: &str,
    include_declaration: bool,
) -> Result<Vec<Reference>, Vec<Diagnostic>> {
    global_references_to_definition_parsed_source_with_hir(
        parsed,
        target,
        name,
        include_declaration,
        None,
    )
}

pub fn global_references_to_definition_parsed_source_with_hir(
    parsed: &ParsedSource,
    target: &Definition,
    name: &str,
    include_declaration: bool,
    hir: Option<&HirModule>,
) -> Result<Vec<Reference>, Vec<Diagnostic>> {
    let mut references = source_identifiers(&parsed.source)
        .into_iter()
        .filter(|identifier| identifier.text == name)
        .filter_map(|identifier| {
            global_reference_for_identifier(parsed, &identifier, target, hir).transpose()
        })
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

pub fn global_type_alias_references_to_definition_parsed_source(
    parsed: &ParsedSource,
    target: &Definition,
    name: &str,
    include_declaration: bool,
) -> Result<Vec<Reference>, Vec<Diagnostic>> {
    global_type_alias_references_to_definition_parsed_source_with_hir(
        parsed,
        target,
        name,
        include_declaration,
        None,
    )
}

pub fn global_type_alias_references_to_definition_parsed_source_with_hir(
    parsed: &ParsedSource,
    target: &Definition,
    name: &str,
    include_declaration: bool,
    hir: Option<&HirModule>,
) -> Result<Vec<Reference>, Vec<Diagnostic>> {
    let mut references = source_identifiers(&parsed.source)
        .into_iter()
        .filter(|identifier| identifier.text == name)
        .filter_map(|identifier| {
            global_type_alias_reference_for_identifier(parsed, &identifier, target, hir).transpose()
        })
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

fn reference_for_identifier(
    parsed: &ParsedSource,
    identifier: &SourceIdentifier,
    target: &Definition,
    hir: Option<&HirModule>,
    semantic_index: Option<&SemanticIndex>,
) -> Result<Option<Reference>, Vec<Diagnostic>> {
    let span = identifier.span;
    let definition = semantic_index.and_then(|index| index.definition_at(span.start));
    let definition = if definition.is_some() {
        definition
    } else {
        definition_parsed_source_with_hir(parsed, hir, span.start)?
    };
    let Some(definition) = definition else {
        return Ok(None);
    };

    Ok(same_definition(&definition, target).then(|| Reference {
        path: parsed.path.clone(),
        span,
        kind: reference_kind(definition.kind),
    }))
}

fn global_reference_for_identifier(
    parsed: &ParsedSource,
    identifier: &SourceIdentifier,
    target: &Definition,
    hir: Option<&HirModule>,
) -> Result<Option<Reference>, Vec<Diagnostic>> {
    let span = identifier.span;
    let definition = definition_parsed_source_with_hir(parsed, hir, span.start)?;

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

fn global_type_alias_reference_for_identifier(
    parsed: &ParsedSource,
    identifier: &SourceIdentifier,
    target: &Definition,
    hir: Option<&HirModule>,
) -> Result<Option<Reference>, Vec<Diagnostic>> {
    let span = identifier.span;
    let definition = definition_parsed_source_with_hir(parsed, hir, span.start)?;

    if definition.as_ref().is_some_and(|definition| {
        definition.kind == DefinitionKind::TypeAlias && same_definition(definition, target)
    }) {
        return Ok(Some(Reference {
            path: parsed.path.clone(),
            span,
            kind: ReferenceKind::TypeAlias,
        }));
    }

    if definition.is_some() || !is_type_reference(parsed, span.start) {
        return Ok(None);
    }

    Ok(Some(Reference {
        path: parsed.path.clone(),
        span,
        kind: ReferenceKind::TypeAlias,
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
        DefinitionKind::Builtin => ReferenceKind::Function,
        DefinitionKind::Asset => ReferenceKind::Asset,
        DefinitionKind::SignalEvent => ReferenceKind::SignalEvent,
        DefinitionKind::TypeAlias => ReferenceKind::TypeAlias,
        DefinitionKind::Local => ReferenceKind::Local,
        DefinitionKind::Function => ReferenceKind::Function,
        DefinitionKind::Parameter => ReferenceKind::Parameter,
        DefinitionKind::TableField => ReferenceKind::TableField,
    }
}

fn is_type_reference(parsed: &ParsedSource, position: SourcePosition) -> bool {
    let Some(line) = line_at(&parsed.source, position.line) else {
        return false;
    };
    let cursor = byte_for_column(line, position.column).min(line.len());
    let prefix = &line[..cursor];
    let Some(marker_index) = prefix.rfind([':', '|', '=']) else {
        return false;
    };
    let marker = prefix.as_bytes()[marker_index];
    let before_marker = &prefix[..marker_index];

    match marker {
        b':' => true,
        b'|' => before_marker.contains(':') || before_marker.contains("type "),
        b'=' => before_marker.contains("type "),
        _ => false,
    }
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
