use std::path::Path;

use crate::hir::{lower_parsed_source, HirExportValue, HirModule, HirSymbolId, HirSymbolKind};
use crate::{parse_tua_source, Diagnostic, ParsedSource, SourcePosition, SourceSpan, TuaType};

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

pub fn document_symbols_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<Vec<DocumentSymbol>, Vec<Diagnostic>> {
    let parsed = parse_tua_source(path, source)?;

    Ok(document_symbols_parsed_source(&parsed))
}

pub fn document_symbols_parsed_source(parsed: &ParsedSource) -> Vec<DocumentSymbol> {
    let hir = lower_parsed_source(parsed);

    document_symbols_parsed_source_with_hir(parsed, &hir)
}

pub fn document_symbols_parsed_source_with_hir(
    parsed: &ParsedSource,
    hir: &HirModule,
) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    symbols.extend(type_alias_symbols(parsed, hir));
    symbols.extend(function_symbols(parsed, hir));
    symbols.extend(local_symbols(parsed, hir));

    if let Some(exports) = module_exports_symbol(hir) {
        symbols.push(exports);
    }

    symbols.sort_by_key(|symbol| position_key(symbol.range.start));

    symbols
}

fn type_alias_symbols(parsed: &ParsedSource, hir: &HirModule) -> Vec<DocumentSymbol> {
    hir.symbols()
        .iter()
        .filter(|symbol| symbol.kind == HirSymbolKind::TypeAlias)
        .filter_map(|symbol| {
            let alias = parsed
                .type_aliases
                .iter()
                .find(|alias| alias.name_span == Some(symbol.name_span))?;
            let range = symbol.origin_span;
            let selection_range = symbol.name_span;

            let detail_prefix = match alias.kind {
                crate::TypeAliasKind::Alias => "type",
                crate::TypeAliasKind::Enum => "enum",
            };

            Some(document_symbol(DocumentSymbol {
                name: symbol.name.clone(),
                kind: DocumentSymbolKind::TypeAlias,
                detail: Some(format!("{detail_prefix} {}", format_tua_type(&alias.ty))),
                range,
                selection_range,
                children: table_field_symbols(hir, symbol.id),
            }))
        })
        .collect()
}

fn table_field_symbols(hir: &HirModule, parent: HirSymbolId) -> Vec<DocumentSymbol> {
    hir.symbols()
        .iter()
        .filter(|symbol| symbol.kind == HirSymbolKind::TableField && symbol.parent == Some(parent))
        .map(|symbol| {
            let range = symbol.origin_span;
            let selection_range = symbol.name_span;

            document_symbol(DocumentSymbol {
                name: symbol.name.clone(),
                kind: DocumentSymbolKind::Field,
                detail: symbol.ty.as_ref().map(format_tua_type),
                range,
                selection_range,
                children: table_field_symbols(hir, symbol.id),
            })
        })
        .collect()
}

fn function_symbols(parsed: &ParsedSource, hir: &HirModule) -> Vec<DocumentSymbol> {
    hir.symbols()
        .iter()
        .filter(|symbol| symbol.kind == HirSymbolKind::Function)
        .map(|symbol| {
            let function = parsed
                .function_annotations
                .iter()
                .find(|function| function.name_span == Some(symbol.name_span));
            let range = symbol.origin_span;
            let selection_range = symbol.name_span;

            document_symbol(DocumentSymbol {
                name: symbol.name.clone(),
                kind: DocumentSymbolKind::Function,
                detail: function
                    .and_then(|function| function.return_type.as_ref())
                    .as_ref()
                    .map(|ty| format!("returns {}", format_tua_type(ty))),
                range,
                selection_range,
                children: parameter_symbols(parsed, hir, symbol.id),
            })
        })
        .collect()
}

fn parameter_symbols(
    parsed: &ParsedSource,
    hir: &HirModule,
    parent: HirSymbolId,
) -> Vec<DocumentSymbol> {
    hir.symbols()
        .iter()
        .filter(|symbol| symbol.kind == HirSymbolKind::Parameter && symbol.parent == Some(parent))
        .map(|symbol| {
            let parameter = parsed
                .function_annotations
                .iter()
                .flat_map(|function| function.parameters.iter())
                .find(|parameter| parameter.name_span == Some(symbol.name_span));

            document_symbol(DocumentSymbol {
                name: symbol.name.clone(),
                kind: DocumentSymbolKind::Parameter,
                detail: parameter.map(|parameter| format_tua_type(&parameter.ty)),
                range: symbol.origin_span,
                selection_range: symbol.name_span,
                children: Vec::new(),
            })
        })
        .collect()
}

fn local_symbols(parsed: &ParsedSource, hir: &HirModule) -> Vec<DocumentSymbol> {
    hir.symbols()
        .iter()
        .filter(|symbol| symbol.kind == HirSymbolKind::Local)
        .filter_map(|symbol| {
            let local = parsed
                .local_annotations
                .iter()
                .find(|local| local.name_span == Some(symbol.name_span))?;

            Some(document_symbol(DocumentSymbol {
                name: symbol.name.clone(),
                kind: DocumentSymbolKind::Local,
                detail: Some(format_tua_type(&local.ty)),
                range: symbol.origin_span,
                selection_range: symbol.name_span,
                children: Vec::new(),
            }))
        })
        .collect()
}

fn module_exports_symbol(hir: &HirModule) -> Option<DocumentSymbol> {
    let children = hir
        .exports
        .iter()
        .filter(|export| matches!(export.value, HirExportValue::Symbol(_)))
        .map(|export| {
            document_symbol(DocumentSymbol {
                name: export.name.clone(),
                kind: DocumentSymbolKind::Field,
                detail: Some("export".to_owned()),
                range: export.origin_span,
                selection_range: export.name_span,
                children: Vec::new(),
            })
        })
        .collect::<Vec<_>>();
    let range = children
        .iter()
        .map(|child| child.range)
        .reduce(enclosing_span)?;

    (!children.is_empty()).then(|| {
        document_symbol(DocumentSymbol {
            name: "exports".to_owned(),
            kind: DocumentSymbolKind::ModuleExports,
            detail: Some("module exports".to_owned()),
            range,
            selection_range: range,
            children,
        })
    })
}

fn document_symbol(mut symbol: DocumentSymbol) -> DocumentSymbol {
    symbol.range = enclosing_span(symbol.range, symbol.selection_range);
    symbol
}

fn enclosing_span(range: SourceSpan, selection_range: SourceSpan) -> SourceSpan {
    SourceSpan {
        start: min_position(range.start, selection_range.start),
        end: max_position(range.end, selection_range.end),
    }
}

fn min_position(left: SourcePosition, right: SourcePosition) -> SourcePosition {
    if position_key(left) <= position_key(right) {
        left
    } else {
        right
    }
}

fn max_position(left: SourcePosition, right: SourcePosition) -> SourcePosition {
    if position_key(left) >= position_key(right) {
        left
    } else {
        right
    }
}

fn format_tua_type(ty: &TuaType) -> String {
    match ty {
        TuaType::Named(name) => name.clone(),
        TuaType::StringLiteral(value) => format!("{value:?}"),
        TuaType::Union(types) => types
            .iter()
            .map(format_tua_type)
            .collect::<Vec<_>>()
            .join(" | "),
        TuaType::Table(fields) => {
            let fields = fields
                .iter()
                .map(|field| {
                    if field.optional {
                        format!("{}?: {}", field.name, format_tua_type(&field.ty))
                    } else {
                        format!("{}: {}", field.name, format_tua_type(&field.ty))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            format!("{{ {fields} }}")
        }
        TuaType::Array(item) => format!("{}[]", format_tua_type(item)),
        TuaType::Tuple(items) => format!(
            "[{}]",
            items
                .iter()
                .map(format_tua_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TuaType::Dictionary { key, value } => {
            format!(
                "{{ [{}]: {} }}",
                format_tua_type(key),
                format_tua_type(value)
            )
        }
        TuaType::Function {
            parameters,
            returns,
        } => {
            let params = parameters
                .iter()
                .map(|parameter| {
                    let label = parameter
                        .name
                        .as_ref()
                        .map(|name| {
                            format!("{name}{}: ", if parameter.optional { "?" } else { "" })
                        })
                        .unwrap_or_default();
                    format!("{label}{}", format_tua_type(&parameter.ty))
                })
                .collect::<Vec<_>>()
                .join(", ");
            let returns = returns
                .iter()
                .map(format_tua_type)
                .collect::<Vec<_>>()
                .join(", ");

            if returns.is_empty() {
                format!("fun({params})")
            } else {
                format!("fun({params}): {returns}")
            }
        }
    }
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests;
