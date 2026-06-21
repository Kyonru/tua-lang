use std::path::Path;

use full_moon::ast::{Expression, Field, LastStmt, TableConstructor};
use full_moon::node::Node;

use crate::{parse_tlua_source, Diagnostic, ParsedSource, SourcePosition, SourceSpan, TluaType};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: DocumentSymbolKind,
    pub detail: Option<String>,
    pub range: SourceSpan,
    pub selection_range: SourceSpan,
    pub children: Vec<DocumentSymbol>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentSymbolKind {
    TypeAlias,
    Function,
    Local,
    Parameter,
    Field,
    ModuleExports,
}

pub fn document_symbols_tlua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<Vec<DocumentSymbol>, Vec<Diagnostic>> {
    let parsed = parse_tlua_source(path, source)?;

    Ok(document_symbols_parsed_source(&parsed))
}

pub fn document_symbols_parsed_source(parsed: &ParsedSource) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    symbols.extend(type_alias_symbols(parsed));
    symbols.extend(function_symbols(parsed));
    symbols.extend(local_symbols(parsed));

    if let Some(exports) = module_exports_symbol(parsed) {
        symbols.push(exports);
    }

    symbols.sort_by_key(|symbol| position_key(symbol.range.start));

    symbols
}

fn type_alias_symbols(parsed: &ParsedSource) -> Vec<DocumentSymbol> {
    parsed
        .type_aliases
        .iter()
        .filter_map(|alias| {
            let range = alias.span?;
            let selection_range = alias.name_span.unwrap_or(range);

            Some(DocumentSymbol {
                name: alias.name.clone(),
                kind: DocumentSymbolKind::TypeAlias,
                detail: Some(format!("type {}", format_tlua_type(&alias.ty))),
                range,
                selection_range,
                children: table_field_symbols(&alias.ty),
            })
        })
        .collect()
}

fn table_field_symbols(ty: &TluaType) -> Vec<DocumentSymbol> {
    let TluaType::Table(fields) = ty else {
        return Vec::new();
    };

    fields
        .iter()
        .filter_map(|field| {
            let range = field.span?;
            let selection_range = field.name_span.unwrap_or(range);
            let optional = field.optional.then_some("optional ");

            Some(DocumentSymbol {
                name: field.name.clone(),
                kind: DocumentSymbolKind::Field,
                detail: Some(format!(
                    "{}{}",
                    optional.unwrap_or_default(),
                    format_tlua_type(&field.ty)
                )),
                range,
                selection_range,
                children: Vec::new(),
            })
        })
        .collect()
}

fn function_symbols(parsed: &ParsedSource) -> Vec<DocumentSymbol> {
    parsed
        .function_annotations
        .iter()
        .filter_map(|function| {
            let name = function.name.clone()?;
            let range = function.span?;
            let selection_range = function.name_span.unwrap_or(range);

            Some(DocumentSymbol {
                name,
                kind: DocumentSymbolKind::Function,
                detail: function
                    .return_type
                    .as_ref()
                    .map(|ty| format!("returns {}", format_tlua_type(ty))),
                range,
                selection_range,
                children: function
                    .parameters
                    .iter()
                    .filter_map(|parameter| {
                        let range = parameter.span?;
                        let selection_range = parameter.name_span.unwrap_or(range);

                        Some(DocumentSymbol {
                            name: parameter.name.clone(),
                            kind: DocumentSymbolKind::Parameter,
                            detail: Some(format_tlua_type(&parameter.ty)),
                            range,
                            selection_range,
                            children: Vec::new(),
                        })
                    })
                    .collect(),
            })
        })
        .collect()
}

fn local_symbols(parsed: &ParsedSource) -> Vec<DocumentSymbol> {
    parsed
        .local_annotations
        .iter()
        .filter_map(|local| {
            let selection_range = local.name_span?;
            let range = local.span.unwrap_or(selection_range);

            Some(DocumentSymbol {
                name: local.name.clone(),
                kind: DocumentSymbolKind::Local,
                detail: Some(format_tlua_type(&local.ty)),
                range,
                selection_range,
                children: Vec::new(),
            })
        })
        .collect()
}

fn module_exports_symbol(parsed: &ParsedSource) -> Option<DocumentSymbol> {
    let LastStmt::Return(return_stmt) = parsed.ast.nodes().last_stmt()? else {
        return None;
    };
    let Expression::TableConstructor(table) = return_stmt.returns().iter().next()? else {
        return None;
    };
    let range = node_span(table)?;
    let children = export_field_symbols(table);

    (!children.is_empty()).then(|| DocumentSymbol {
        name: "exports".to_owned(),
        kind: DocumentSymbolKind::ModuleExports,
        detail: Some("module exports".to_owned()),
        range,
        selection_range: range,
        children,
    })
}

fn export_field_symbols(table: &TableConstructor) -> Vec<DocumentSymbol> {
    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NameKey { key, .. } = field else {
                return None;
            };
            let range = node_span(field)?;
            let selection_range = node_span(key).unwrap_or(range);

            Some(DocumentSymbol {
                name: token_text(key),
                kind: DocumentSymbolKind::Field,
                detail: Some("export".to_owned()),
                range,
                selection_range,
                children: Vec::new(),
            })
        })
        .collect()
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn format_tlua_type(ty: &TluaType) -> String {
    match ty {
        TluaType::Named(name) => name.clone(),
        TluaType::Union(types) => types
            .iter()
            .map(format_tlua_type)
            .collect::<Vec<_>>()
            .join(" | "),
        TluaType::Table(fields) => {
            let fields = fields
                .iter()
                .map(|field| {
                    if field.optional {
                        format!("{}?: {}", field.name, format_tlua_type(&field.ty))
                    } else {
                        format!("{}: {}", field.name, format_tlua_type(&field.ty))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            format!("{{ {fields} }}")
        }
    }
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests;
