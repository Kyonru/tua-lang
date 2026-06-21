use std::collections::BTreeMap;
use std::path::Path;

use self::context::{completion_context, CompletionContext};
use self::require::complete_require_paths;
use self::symbol::lua_symbols_before_position;
use crate::lua::{self, LuaItem, LuaItemKind};
use crate::module_index::{required_module_exports_for_receiver, ModuleExports};
use crate::semantic::{resolve_aliases, resolve_field_chain_type};
use crate::types::{build_type_environment_with_diagnostics, TableShape, Type, TypeEnvironment};
use crate::{parse_tua_source, Diagnostic, ParsedSource, SourcePosition};

mod context;
mod require;
mod symbol;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Completion {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionKind {
    Function,
    TypeAlias,
    Field,
    Keyword,
    Module,
    Variable,
    Value,
}

pub fn complete_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    position: SourcePosition,
) -> Result<Vec<Completion>, Vec<Diagnostic>> {
    let path = path.as_ref().to_path_buf();
    let source = source.into();
    let Some(context) = completion_context(&source, position) else {
        return Ok(Vec::new());
    };
    let parsed = match parse_completion_source(&path, &source, position) {
        Ok(parsed) => parsed,
        Err(_) => return Ok(complete_lossy_source(&path, &source, context)),
    };

    Ok(complete_parsed_source_with_context(&parsed, context))
}

pub fn complete_parsed_source(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Result<Vec<Completion>, Vec<Diagnostic>> {
    let Some(context) = completion_context(&parsed.source, position) else {
        return Ok(Vec::new());
    };

    Ok(complete_parsed_source_with_context(parsed, context))
}

fn complete_parsed_source_with_context(
    parsed: &ParsedSource,
    context: CompletionContext,
) -> Vec<Completion> {
    match context {
        CompletionContext::TypeAnnotation { prefix } => {
            complete_type_names(parsed, prefix.as_str())
        }
        CompletionContext::FieldAccess {
            receiver_parts,
            prefix,
        } => {
            let environment = build_completion_environment(parsed);
            let fields = complete_fields(&environment, &receiver_parts, prefix.as_str());

            if !fields.is_empty() {
                return fields;
            }

            let module_fields =
                complete_required_module_fields(parsed, &receiver_parts, prefix.as_str());

            if !module_fields.is_empty() {
                return module_fields;
            }

            if environment.local(&receiver_parts[0]).is_some() {
                return fields;
            }

            complete_lua_fields(&receiver_parts, prefix.as_str())
        }
        CompletionContext::RequirePath { prefix } => {
            complete_require_paths(&parsed.path, prefix.as_str())
        }
        CompletionContext::Identifier { prefix, position } => {
            complete_identifier_items(parsed, prefix.as_str(), position)
        }
    }
}

fn complete_lossy_source(path: &Path, source: &str, context: CompletionContext) -> Vec<Completion> {
    match context {
        CompletionContext::TypeAnnotation { prefix } => complete_builtin_types(prefix.as_str()),
        CompletionContext::FieldAccess {
            receiver_parts,
            prefix,
        } => complete_lua_fields(&receiver_parts, prefix.as_str()),
        CompletionContext::RequirePath { prefix } => complete_require_paths(path, prefix.as_str()),
        CompletionContext::Identifier { prefix, position } => {
            complete_lossy_identifier_items(source, prefix.as_str(), position)
        }
    }
}

fn complete_type_names(parsed: &ParsedSource, prefix: &str) -> Vec<Completion> {
    let mut completions = BTreeMap::new();

    for completion in complete_builtin_types(prefix) {
        completions.insert(completion.label.clone(), completion);
    }

    for completion in complete_type_aliases(parsed, prefix) {
        completions.insert(completion.label.clone(), completion);
    }

    completions.into_values().collect()
}

fn complete_builtin_types(prefix: &str) -> Vec<Completion> {
    ["any", "boolean", "nil", "number", "string"]
        .into_iter()
        .filter(|primitive| primitive.starts_with(prefix))
        .map(|primitive| Completion {
            label: primitive.to_owned(),
            kind: CompletionKind::TypeAlias,
            detail: Some("built-in type".to_owned()),
        })
        .collect()
}

fn complete_type_aliases(parsed: &ParsedSource, prefix: &str) -> Vec<Completion> {
    parsed
        .type_aliases
        .iter()
        .filter(|alias| alias.name.starts_with(prefix))
        .map(|alias| Completion {
            label: alias.name.clone(),
            kind: CompletionKind::TypeAlias,
            detail: Some("type alias".to_owned()),
        })
        .collect()
}

fn complete_identifier_items(
    parsed: &ParsedSource,
    prefix: &str,
    position: SourcePosition,
) -> Vec<Completion> {
    let mut completions = BTreeMap::new();

    insert_lossy_identifier_items(&mut completions, &parsed.source, prefix, position);

    for local in &parsed.local_annotations {
        if local.name.starts_with(prefix) {
            completions.insert(
                local.name.clone(),
                Completion {
                    label: local.name.clone(),
                    kind: CompletionKind::Variable,
                    detail: Some("local".to_owned()),
                },
            );
        }
    }

    for function in &parsed.function_annotations {
        let Some(name) = &function.name else {
            continue;
        };

        if name.starts_with(prefix) {
            completions.insert(
                name.clone(),
                Completion {
                    label: name.clone(),
                    kind: CompletionKind::Function,
                    detail: Some("function".to_owned()),
                },
            );
        }
    }

    completions.into_values().collect()
}

fn complete_lossy_identifier_items(
    source: &str,
    prefix: &str,
    position: SourcePosition,
) -> Vec<Completion> {
    let mut completions = BTreeMap::new();

    insert_lossy_identifier_items(&mut completions, source, prefix, position);

    completions.into_values().collect()
}

fn insert_lossy_identifier_items(
    completions: &mut BTreeMap<String, Completion>,
    source: &str,
    prefix: &str,
    position: SourcePosition,
) {
    for item in lua::keyword_items(prefix)
        .into_iter()
        .chain(lua::top_level_items(prefix))
    {
        insert_lua_item(completions, item);
    }

    for symbol in lua_symbols_before_position(source, position) {
        if symbol.name.starts_with(prefix) {
            completions.insert(symbol.name.clone(), symbol.into_completion());
        }
    }
}

fn complete_fields(
    environment: &TypeEnvironment,
    receiver_parts: &[String],
    prefix: &str,
) -> Vec<Completion> {
    let Some(receiver_type) = resolve_field_chain_type(environment, receiver_parts) else {
        return Vec::new();
    };
    let mut fields = BTreeMap::new();

    collect_table_fields(environment, &receiver_type, &mut fields);

    fields
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, detail)| Completion {
            label: name,
            kind: CompletionKind::Field,
            detail: Some(detail),
        })
        .collect()
}

fn collect_table_fields(
    environment: &TypeEnvironment,
    ty: &Type,
    fields: &mut BTreeMap<String, String>,
) {
    match resolve_aliases(environment, ty, 0) {
        Type::Table(table) => collect_shape_fields(&table, fields),
        Type::Union(types) => {
            for ty in types {
                collect_table_fields(environment, &ty, fields);
            }
        }
        Type::Any | Type::Alias(_) | Type::Function(_) | Type::Primitive(_) => {}
    }
}

fn collect_shape_fields(table: &TableShape, fields: &mut BTreeMap<String, String>) {
    for (name, field) in table.fields() {
        let suffix = if field.optional { " optional" } else { "" };

        fields.insert(name.to_owned(), format!("field{suffix}: {}", field.ty));
    }
}

fn complete_lua_fields(receiver_parts: &[String], prefix: &str) -> Vec<Completion> {
    lua::field_items(receiver_parts, prefix)
        .into_iter()
        .map(completion_from_lua_item)
        .collect()
}

fn complete_required_module_fields(
    parsed: &ParsedSource,
    receiver_parts: &[String],
    prefix: &str,
) -> Vec<Completion> {
    let Some((receiver, fields)) = receiver_parts.split_first() else {
        return Vec::new();
    };
    let Some(exports) = required_module_exports_for_receiver(parsed, receiver) else {
        return Vec::new();
    };
    let ty = exported_type_for_receiver(&exports, fields);
    let mut fields = BTreeMap::new();

    collect_export_fields(&ty, &mut fields);

    fields
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, ty)| Completion {
            kind: completion_kind_for_type(&ty),
            detail: Some(format!("field: {ty}")),
            label: name,
        })
        .collect()
}

fn exported_type_for_receiver(exports: &ModuleExports, fields: &[String]) -> Type {
    let mut current = Type::table(
        exports
            .fields
            .iter()
            .map(|(name, ty)| (name.clone(), crate::TableField::required(ty.clone()))),
    );

    for field in fields {
        let Some(next) = export_field_type(&current, field) else {
            return Type::Any;
        };

        current = next;
    }

    current
}

fn export_field_type(ty: &Type, field: &str) -> Option<Type> {
    match ty {
        Type::Table(table) => table.field(field).map(|field| field.ty.clone()),
        Type::Union(types) => {
            let mut resolved = Vec::new();

            for ty in types {
                if let Some(field_ty) = export_field_type(ty, field) {
                    resolved.push(field_ty);
                }
            }

            match resolved.len() {
                0 => None,
                1 => resolved.into_iter().next(),
                _ => Some(Type::union(resolved)),
            }
        }
        Type::Any | Type::Alias(_) | Type::Function(_) | Type::Primitive(_) => None,
    }
}

fn collect_export_fields(ty: &Type, fields: &mut BTreeMap<String, Type>) {
    match ty {
        Type::Table(table) => {
            fields.extend(
                table
                    .fields()
                    .map(|(name, field)| (name.to_owned(), field.ty.clone())),
            );
        }
        Type::Union(types) => {
            for ty in types {
                collect_export_fields(ty, fields);
            }
        }
        Type::Any | Type::Alias(_) | Type::Function(_) | Type::Primitive(_) => {}
    }
}

fn completion_kind_for_type(ty: &Type) -> CompletionKind {
    match ty {
        Type::Function(_) => CompletionKind::Function,
        _ => CompletionKind::Field,
    }
}

fn completion_from_lua_item(item: LuaItem) -> Completion {
    Completion {
        label: item.label.to_owned(),
        kind: completion_kind_from_lua_item(item.kind),
        detail: Some(item.detail.to_owned()),
    }
}

fn insert_lua_item(completions: &mut BTreeMap<String, Completion>, item: LuaItem) {
    completions
        .entry(item.label.to_owned())
        .or_insert_with(|| completion_from_lua_item(item));
}

fn completion_kind_from_lua_item(kind: LuaItemKind) -> CompletionKind {
    match kind {
        LuaItemKind::Function => CompletionKind::Function,
        LuaItemKind::Keyword => CompletionKind::Keyword,
        LuaItemKind::Module => CompletionKind::Module,
        LuaItemKind::Value => CompletionKind::Value,
    }
}

fn build_completion_environment(parsed: &ParsedSource) -> TypeEnvironment {
    let (environment, _) = build_type_environment_with_diagnostics(parsed);

    environment
}

fn parse_completion_source(
    path: &Path,
    source: &str,
    position: SourcePosition,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    parse_tua_source(path, source.to_owned())
        .or_else(|_| parse_tua_source(path, source_with_line_removed(source, position.line)))
}

fn source_with_line_removed(source: &str, line_to_remove: usize) -> String {
    let mut output = String::with_capacity(source.len());
    let mut current_line = 1;
    let mut line_start = 0;

    for (index, character) in source.char_indices() {
        if character != '\n' {
            continue;
        }

        if current_line == line_to_remove {
            output.push('\n');
        } else {
            output.push_str(&source[line_start..=index]);
        }

        current_line += 1;
        line_start = index + 1;
    }

    if line_start < source.len() {
        if current_line == line_to_remove {
            output.push('\n');
        } else {
            output.push_str(&source[line_start..]);
        }
    }

    output
}

#[cfg(test)]
mod tests;
