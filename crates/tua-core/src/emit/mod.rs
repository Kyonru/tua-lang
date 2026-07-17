use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

use full_moon::ast::span::ContainedSpan;
use full_moon::ast::{self, Expression, Field, LastStmt, Var};
use full_moon::tokenizer::TokenReference;
use full_moon::visitors::VisitorMut;

use crate::classes;
use crate::runtime_type;
use crate::semantic::SemanticModel;
use crate::types::{build_type_environment_with_diagnostics, TypeEnvironment};
use crate::{
    asset, ecs, infer_module_exports, object_pool, parse_tua_source, signal, vector,
    AnnotationMode, Diagnostic, DiagnosticCode, ParameterDefault, ParsedSource, ProjectWorkspace,
    SourcePosition, SourceSpan, TuaType, Type,
};

pub(crate) mod lowering;

pub use lowering::{LoweringKind, LuaEmission, LuaLineProvenance};

type ImportResolver<'a> = Option<&'a dyn Fn(&str) -> Option<String>>;

pub fn emit_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<String, Vec<Diagnostic>> {
    emit_tua_source_artifact(path, source).map(|emission| emission.code)
}

pub fn emit_tua_source_artifact(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<LuaEmission, Vec<Diagnostic>> {
    emit_tua_source_artifact_with_mode(path, source, AnnotationMode::Off)
}

pub fn emit_tua_source_with_mode(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    annotation_mode: AnnotationMode,
) -> Result<String, Vec<Diagnostic>> {
    emit_tua_source_artifact_with_mode(path, source, annotation_mode).map(|emission| emission.code)
}

pub fn emit_tua_source_artifact_with_mode(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    annotation_mode: AnnotationMode,
) -> Result<LuaEmission, Vec<Diagnostic>> {
    let parsed =
        parse_tua_source(path, source).map_err(reclassify_compound_assignment_diagnostics)?;

    emit_lua_artifact_with_mode(&parsed, annotation_mode)
}

pub fn emit_tua_source_with_workspace(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    annotation_mode: AnnotationMode,
    workspace: &ProjectWorkspace,
) -> Result<String, Vec<Diagnostic>> {
    emit_tua_source_with_workspace_artifact(path, source, annotation_mode, workspace)
        .map(|emission| emission.code)
}

pub fn emit_tua_source_with_workspace_artifact(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    annotation_mode: AnnotationMode,
    workspace: &ProjectWorkspace,
) -> Result<LuaEmission, Vec<Diagnostic>> {
    let parsed =
        parse_tua_source(path, source).map_err(reclassify_compound_assignment_diagnostics)?;

    let resolver = |specifier: &str| workspace.module_name_for_import(&parsed.path, specifier);

    emit_lua_artifact_with_mode_and_import_resolver(
        &parsed,
        annotation_mode,
        Some(&resolver),
        workspace.config().ecs.enabled,
    )
}

pub fn emit_lua(parsed: &ParsedSource) -> Result<String, Vec<Diagnostic>> {
    emit_lua_artifact(parsed).map(|emission| emission.code)
}

pub fn emit_lua_artifact(parsed: &ParsedSource) -> Result<LuaEmission, Vec<Diagnostic>> {
    emit_lua_artifact_with_mode(parsed, AnnotationMode::Off)
}

pub fn emit_lua_with_mode(
    parsed: &ParsedSource,
    annotation_mode: AnnotationMode,
) -> Result<String, Vec<Diagnostic>> {
    emit_lua_artifact_with_mode(parsed, annotation_mode).map(|emission| emission.code)
}

pub fn emit_lua_artifact_with_mode(
    parsed: &ParsedSource,
    annotation_mode: AnnotationMode,
) -> Result<LuaEmission, Vec<Diagnostic>> {
    emit_lua_artifact_with_mode_and_import_resolver(
        parsed,
        annotation_mode,
        None,
        crate::config::EcsConfig::default().enabled,
    )
}

pub(crate) fn emit_lua_with_mode_and_import_resolver(
    parsed: &ParsedSource,
    annotation_mode: AnnotationMode,
    import_resolver: ImportResolver<'_>,
) -> Result<String, Vec<Diagnostic>> {
    emit_lua_artifact_with_mode_and_import_resolver(
        parsed,
        annotation_mode,
        import_resolver,
        crate::config::EcsConfig::default().enabled,
    )
    .map(|emission| emission.code)
}

fn emit_lua_artifact_with_mode_and_import_resolver(
    parsed: &ParsedSource,
    annotation_mode: AnnotationMode,
    import_resolver: ImportResolver<'_>,
    ecs_enabled: bool,
) -> Result<LuaEmission, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let ast = erase_ast(&parsed.path, parsed.ast.clone(), &mut diagnostics);

    if diagnostics.is_empty() {
        let mut document = lowering::LoweringDocument::from_erased_ast(&ast, parsed);

        let operations = lower_default_parameter_operations(&document, parsed);
        apply_lowering_operations(&mut document, operations, parsed)?;

        let operations = lower_builtin_call_operations(&document, parsed, ecs_enabled);
        apply_lowering_operations(&mut document, operations, parsed)?;

        let operations = lower_import_call_operations(&document, parsed, import_resolver)?;
        apply_lowering_operations(&mut document, operations, parsed)?;

        let operations = lower_runtime_type_check_operations(&document, parsed)?;
        apply_lowering_operations(&mut document, operations, parsed)?;

        let operations = lower_final_class_method_call_operations(&document, parsed);
        apply_lowering_operations(&mut document, operations, parsed)?;

        let operations = lower_compound_assignment_operations(&document);
        apply_lowering_operations(&mut document, operations, parsed)?;

        let operations = crate::sprite::lowering_operations(&document, parsed);
        apply_lowering_operations(&mut document, operations, parsed)?;

        if annotation_mode == AnnotationMode::Luacats {
            let operations = luacats_annotation_operations(&document, parsed);
            apply_lowering_operations(&mut document, operations, parsed)?;
        }

        Ok(document.finish())
    } else {
        Err(diagnostics)
    }
}

fn apply_lowering_operations(
    document: &mut lowering::LoweringDocument,
    operations: Vec<lowering::LoweringOperation>,
    parsed: &ParsedSource,
) -> Result<(), Vec<Diagnostic>> {
    document.apply_operations(operations).map_err(|error| {
        let position = parsed
            .span
            .map(|span| span.start)
            .unwrap_or(SourcePosition { line: 1, column: 1 });
        vec![Diagnostic::error(
            parsed.path.clone(),
            position.line,
            position.column,
            error.to_string(),
        )
        .with_code(DiagnosticCode::LOWERING_OPERATION)]
    })
}

fn lower_runtime_type_check_operations(
    document: &lowering::LoweringDocument,
    parsed: &ParsedSource,
) -> Result<Vec<lowering::LoweringOperation>, Vec<Diagnostic>> {
    let output = document.code();
    let (environment, _) = build_type_environment_with_diagnostics(parsed);
    let semantic = SemanticModel::new(parsed, &environment);
    let runtime_check_sites = runtime_type_check_sites(parsed);
    let mut runtime_check_site_index = 0usize;
    let mut operations = Vec::new();
    let mut diagnostics = Vec::new();
    let mut index = 0;

    while index < output.len() {
        let rest = &output[index..];

        if rest.starts_with("--") {
            index = consume_comment(&output, index);
            continue;
        }

        if let Some(end) = consume_string_literal(&output, index) {
            index = end;
            continue;
        }

        if let Some(identifier_end) = consume_identifier(rest) {
            let name = &output[index..index + identifier_end];

            if name == runtime_type::IS_CHECK_HELPER && can_start_builtin_call(&output, index) {
                let argument_start = skip_whitespace(&output, index + identifier_end);

                if let Some((replacement, end)) = lower_runtime_type_check_call(
                    &output,
                    argument_start,
                    parsed,
                    &environment,
                    &semantic,
                    runtime_check_sites.get(runtime_check_site_index).copied(),
                    document.source_span_at(index).start,
                ) {
                    runtime_check_site_index += 1;

                    match replacement {
                        Ok(replacement) => operations.push(document.operation(
                            LoweringKind::RuntimeTypeCheck,
                            index..end,
                            replacement,
                        )),
                        Err(diagnostic) => diagnostics.push(diagnostic),
                    }

                    index = end;
                    continue;
                }
            }

            index += identifier_end;
            continue;
        }

        let character = rest
            .chars()
            .next()
            .expect("loop only runs at valid string boundaries");

        index += character.len_utf8();
    }

    if diagnostics.is_empty() {
        Ok(operations)
    } else {
        Err(diagnostics)
    }
}

fn lower_runtime_type_check_call(
    output: &str,
    argument_start: usize,
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    semantic: &SemanticModel<'_>,
    source_position: Option<SourcePosition>,
    fallback_position: SourcePosition,
) -> Option<(Result<String, Diagnostic>, usize)> {
    if !output[argument_start..].starts_with('(') {
        return None;
    }

    let close = find_matching_paren(output, argument_start)?;
    let arguments = split_top_level_arguments(&output[argument_start + 1..close])?;
    let diagnostic_position = source_position
        .map(source_position_tuple)
        .unwrap_or_else(|| source_position_tuple(fallback_position));

    let replacement = match arguments.as_slice() {
        [value, target] if !value.is_empty() => {
            let Some(target) = short_string_literal_value(target) else {
                return Some((
                    Err(runtime_type_emit_diagnostic(
                        parsed,
                        diagnostic_position,
                        "`is` checks require one static type target",
                    )),
                    close + 1,
                ));
            };

            let lookup_position = source_position.unwrap_or(fallback_position);

            if classes::class_by_name(&parsed.class_declarations, &target).is_some() {
                Ok(class_runtime_is_check(value, &target))
            } else if let Some(lua_type) =
                runtime_type_lua_type_name(environment, semantic, &target, lookup_position)
            {
                Ok(format!("type({value}) == {}", lua_string_literal(lua_type)))
            } else if visible_metatable_target(semantic, &target, lookup_position) {
                Ok(format!("getmetatable({value}) == {target}"))
            } else {
                Err(runtime_type_emit_diagnostic(
                    parsed,
                    diagnostic_position,
                    format!(
                        "unsupported `is` target `{target}`: expected a primitive Lua type or visible metatable table"
                    ),
                ))
            }
        }
        _ => Err(runtime_type_emit_diagnostic(
            parsed,
            diagnostic_position,
            "`is` checks require a value and one runtime-checkable type target",
        )),
    };

    Some((replacement, close + 1))
}

fn runtime_type_check_sites(parsed: &ParsedSource) -> Vec<SourcePosition> {
    let source = &parsed.raw.parser_source;
    let mut sites = Vec::new();
    let mut index = 0usize;

    while index < source.len() {
        if source[index..].starts_with("--") {
            index = consume_comment(source, index);
            continue;
        }

        if let Some(end) = consume_string_literal(source, index) {
            index = end;
            continue;
        }

        if let Some(identifier_end) = consume_identifier(&source[index..]) {
            let name = &source[index..index + identifier_end];

            if name == runtime_type::IS_CHECK_HELPER && can_start_builtin_call(source, index) {
                let argument_start = skip_whitespace(source, index + identifier_end);

                if source[argument_start..].starts_with('(')
                    && find_matching_paren(source, argument_start).is_some()
                {
                    sites.push(source_position_from_byte(source, argument_start));
                }
            }

            index += identifier_end;
            continue;
        }

        let character = source[index..]
            .chars()
            .next()
            .expect("loop only runs at valid string boundaries");
        index += character.len_utf8();
    }

    sites
}

fn class_runtime_is_check(value: &str, target: &str) -> String {
    format!(
        "(getmetatable({value}) ~= nil and getmetatable({value}).__is ~= nil and getmetatable({value}).__is[{target}.__type_id] == true)"
    )
}

fn lower_final_class_method_call_operations(
    document: &lowering::LoweringDocument,
    parsed: &ParsedSource,
) -> Vec<lowering::LoweringOperation> {
    if parsed.class_declarations.is_empty() {
        return Vec::new();
    }

    let output = document.code();
    let local_classes = inferred_class_locals(&output, parsed);
    if local_classes.is_empty() {
        return Vec::new();
    }

    let mut operations = Vec::new();
    let mut index = 0usize;

    while index < output.len() {
        if output[index..].starts_with("--") {
            index = consume_comment(&output, index);
            continue;
        }

        if let Some(end) = consume_string_literal(&output, index) {
            index = end;
            continue;
        }

        if let Some(identifier_end) = consume_identifier(&output[index..]) {
            let receiver = &output[index..index + identifier_end];

            if let Some(class_name) = local_classes.get(receiver) {
                if let Some((replacement, end)) = final_class_method_call_replacement(
                    &output, index, receiver, class_name, parsed,
                ) {
                    operations.push(document.operation(
                        LoweringKind::FinalClassCall,
                        index..end,
                        replacement,
                    ));
                    index = end;
                    continue;
                }
            }

            index += identifier_end;
            continue;
        }

        let character = output[index..]
            .chars()
            .next()
            .expect("loop only runs at valid string boundaries");
        index += character.len_utf8();
    }

    operations
}

fn inferred_class_locals(output: &str, parsed: &ParsedSource) -> BTreeMap<String, String> {
    let mut locals = BTreeMap::new();

    for line in output.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("local ") else {
            continue;
        };
        let Some((name, expression)) = rest.split_once('=') else {
            continue;
        };
        let name = name.trim();
        let expression = expression.trim();

        if !is_identifier(name) {
            continue;
        }

        for class in &parsed.class_declarations {
            let prefix = format!("{}.new", class.name);
            if expression.starts_with(&prefix)
                && expression[prefix.len()..].trim_start().starts_with('(')
            {
                locals.insert(name.to_owned(), class.name.clone());
            }
        }
    }

    locals
}

fn final_class_method_call_replacement(
    output: &str,
    receiver_start: usize,
    receiver: &str,
    class_name: &str,
    parsed: &ParsedSource,
) -> Option<(String, usize)> {
    let mut cursor = receiver_start + receiver.len();
    cursor = skip_whitespace(output, cursor);
    let separator = output[cursor..].chars().next()?;

    if separator != '.' && separator != ':' {
        return None;
    }

    cursor += separator.len_utf8();
    cursor = skip_whitespace(output, cursor);
    let method_start = cursor;
    let method_end = consume_identifier(&output[method_start..])?;
    let method = &output[method_start..method_start + method_end];

    if !classes::class_method_is_final(&parsed.class_declarations, class_name, method) {
        return None;
    }

    cursor = skip_whitespace(output, method_start + method_end);
    if !output[cursor..].starts_with('(') {
        return None;
    }

    let close = find_matching_paren(output, cursor)?;
    let args = output[cursor + 1..close].trim();
    let replacement = if args.is_empty() {
        format!("{class_name}_{method}({receiver})")
    } else {
        format!("{class_name}_{method}({receiver}, {args})")
    };

    Some((replacement, close + 1))
}

fn runtime_type_lua_type_name<'a>(
    environment: &'a TypeEnvironment,
    semantic: &SemanticModel<'_>,
    target: &'a str,
    position: SourcePosition,
) -> Option<&'static str> {
    if let Some(lua_type) = runtime_type::target_lua_type_name(target) {
        return Some(lua_type);
    }

    if semantic.type_of_name(target, position).is_some() {
        return None;
    }

    alias_lua_type_name(environment, environment.alias(target)?, 0)
}

fn alias_lua_type_name(
    environment: &TypeEnvironment,
    ty: &Type,
    depth: usize,
) -> Option<&'static str> {
    if depth >= 64 {
        return None;
    }

    match ty {
        Type::Primitive(crate::PrimitiveType::Boolean) => Some("boolean"),
        Type::Primitive(crate::PrimitiveType::Nil) => Some("nil"),
        Type::Primitive(crate::PrimitiveType::Number) => Some("number"),
        Type::Primitive(crate::PrimitiveType::String) => Some("string"),
        Type::Alias(name) => alias_lua_type_name(environment, environment.alias(name)?, depth + 1),
        _ => None,
    }
}

fn visible_metatable_target(
    semantic: &SemanticModel<'_>,
    target: &str,
    position: SourcePosition,
) -> bool {
    if !runtime_type::is_identifier(target) {
        return false;
    }

    matches!(
        semantic.type_of_name(target, position),
        Some(Type::Table(_))
    )
}

fn runtime_type_emit_diagnostic(
    parsed: &ParsedSource,
    position: (usize, usize),
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(parsed.path.clone(), position.0, position.1, message)
        .with_code(DiagnosticCode::TYPE_UNSUPPORTED_RUNTIME_CHECK)
}

#[derive(Clone, Debug)]
struct LuacatsComments {
    lines: Vec<String>,
    source_span: SourceSpan,
}

fn luacats_annotation_operations(
    document: &lowering::LoweringDocument,
    parsed: &ParsedSource,
) -> Vec<lowering::LoweringOperation> {
    let runtime_output = document.code();
    let runtime_start =
        runtime_output.len() - runtime_output.trim_start_matches(char::is_whitespace).len();
    let runtime_is_empty = runtime_output.trim().is_empty();
    let fallback_span = parsed
        .span
        .unwrap_or_else(|| document.source_span_at(runtime_start));
    let mut operations = Vec::new();
    let mut local_annotations = BTreeMap::<String, VecDeque<LuacatsComments>>::new();
    let mut local_export_types = BTreeMap::<String, String>::new();
    let mut function_annotations = BTreeMap::<String, VecDeque<LuacatsComments>>::new();
    let mut function_export_types = BTreeMap::<String, String>::new();

    for (index, alias) in parsed.type_aliases.iter().enumerate() {
        let mut text = format!("---@alias {} {}\n", alias.name, luacats_type(&alias.ty));
        if index + 1 == parsed.type_aliases.len() && !runtime_is_empty {
            text.push('\n');
        }
        operations.push(lowering::LoweringOperation::insert(
            runtime_start,
            text,
            alias.span.unwrap_or(fallback_span),
            LoweringKind::Annotation,
        ));
    }

    for local in &parsed.local_annotations {
        let type_label = luacats_type(&local.ty);
        local_annotations
            .entry(local.name.clone())
            .or_default()
            .push_back(LuacatsComments {
                lines: vec![format!("---@type {type_label}")],
                source_span: local.span.unwrap_or(fallback_span),
            });
        local_export_types
            .entry(local.name.clone())
            .or_insert(type_label);
    }

    for function in &parsed.function_annotations {
        if let Some(name) = function.name.as_deref() {
            let mut comments = Vec::new();

            for param in &function.parameters {
                comments.push(format!(
                    "---@param {} {}",
                    param.name,
                    luacats_type(&param.ty)
                ));
            }

            if let Some(return_type) = function.return_type.as_ref() {
                comments.push(format!("---@return {}", luacats_type(return_type)));
            }

            if !comments.is_empty() {
                function_annotations
                    .entry(name.to_owned())
                    .or_default()
                    .push_back(LuacatsComments {
                        lines: comments,
                        source_span: function.span.unwrap_or(fallback_span),
                    });
            }

            function_export_types
                .entry(name.to_owned())
                .or_insert_with(|| luacats_function_type(function));
        }
    }

    let export_annotations =
        module_export_annotations(parsed, &local_export_types, &function_export_types);
    let lines = runtime_output
        .split_inclusive('\n')
        .map(|line| line.trim_end_matches(['\r', '\n']))
        .collect::<Vec<_>>();
    let export_line = if export_annotations.is_empty() {
        None
    } else {
        lines
            .iter()
            .rposition(|line| line.trim_start().starts_with("return "))
    };
    let mut offset = 0usize;

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];

        if Some(index) == export_line {
            operations.push(lowering::LoweringOperation::insert(
                offset,
                indented_comments(indent, &export_annotations),
                document.source_span_at(offset),
                LoweringKind::Annotation,
            ));
        }

        for name in declared_local_names(trimmed) {
            if let Some(comments) = pop_comments(&mut local_annotations, &name) {
                operations.push(lowering::LoweringOperation::insert(
                    offset,
                    indented_comments(indent, &comments.lines),
                    comments.source_span,
                    LoweringKind::Annotation,
                ));
            }
        }

        if let Some(name) = declared_function_name(trimmed) {
            if let Some(comments) = pop_comments(&mut function_annotations, &name) {
                operations.push(lowering::LoweringOperation::insert(
                    offset,
                    indented_comments(indent, &comments.lines),
                    comments.source_span,
                    LoweringKind::Annotation,
                ));
            }
        }

        offset += runtime_output[offset..]
            .find('\n')
            .map(|newline| newline + 1)
            .unwrap_or(line.len());
    }

    operations
}

fn indented_comments(indent: &str, comments: &[String]) -> String {
    let mut output = String::new();
    for comment in comments {
        output.push_str(indent);
        output.push_str(comment);
        output.push('\n');
    }
    output
}

fn pop_comments(
    annotations: &mut BTreeMap<String, VecDeque<LuacatsComments>>,
    name: &str,
) -> Option<LuacatsComments> {
    let comments = annotations.get_mut(name)?.pop_front();

    if annotations.get(name).is_some_and(VecDeque::is_empty) {
        annotations.remove(name);
    }

    comments
}

fn declared_function_name(line: &str) -> Option<String> {
    let rest = if let Some(rest) = line.strip_prefix("local function ") {
        rest.trim_start()
    } else if let Some(rest) = line.strip_prefix("function ") {
        rest.trim_start()
    } else {
        return None;
    };

    let token = rest
        .split([' ', '('])
        .next()
        .map(str::trim)
        .filter(|token| !token.is_empty())?;

    Some(token.to_owned())
}

fn declared_local_names(line: &str) -> Vec<String> {
    if !line.starts_with("local ") || line.starts_with("local function ") {
        return Vec::new();
    }

    let rest = line["local ".len()..].trim_start();
    let lhs = rest.split('=').next().unwrap_or(rest).trim();

    lhs.split(',')
        .filter_map(|name| extract_identifier_name(name.trim()))
        .collect()
}

fn extract_identifier_name(input: &str) -> Option<String> {
    let mut iter = input.chars().peekable();

    if let Some(first) = iter.peek() {
        if !is_identifier_start(*first) {
            return None;
        }
    } else {
        return None;
    }

    let mut len = 0usize;
    for ch in iter.by_ref() {
        if !is_identifier_part(ch) {
            break;
        }

        len += ch.len_utf8();
    }

    Some(input[..len].to_owned())
}

fn token_text(token: &TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

fn module_export_annotations(
    parsed: &ParsedSource,
    local_export_types: &BTreeMap<String, String>,
    function_export_types: &BTreeMap<String, String>,
) -> Vec<String> {
    let Some(return_fields) = returned_module_table_fields(parsed) else {
        return Vec::new();
    };

    let inferred_exports = infer_module_exports(parsed);
    let fields = return_fields
        .into_iter()
        .filter_map(|(field_name, value)| {
            let type_label = match value {
                Some(value_name) => function_export_types
                    .get(&value_name)
                    .or_else(|| local_export_types.get(&value_name))
                    .cloned(),
                None => None,
            }
            .or_else(|| {
                inferred_exports
                    .as_ref()
                    .and_then(|exports| exports.fields.get(&field_name))
                    .and_then(|field| luacats_export_type(&field.ty))
            })?;

            Some(format!("---@field {field_name} {type_label}"))
        })
        .collect::<Vec<_>>();

    if fields.is_empty() {
        return Vec::new();
    }

    let mut comments = vec![format!("---@class {}", luacats_export_class_name(parsed))];
    comments.extend(fields);
    comments
}

fn returned_module_table_fields(parsed: &ParsedSource) -> Option<Vec<(String, Option<String>)>> {
    let LastStmt::Return(return_stmt) = parsed.ast.nodes().last_stmt()? else {
        return None;
    };

    let expression = return_stmt.returns().iter().next()?;
    let table = match expression {
        Expression::TableConstructor(table) => table,
        Expression::Parentheses { expression, .. } => match expression.as_ref() {
            Expression::TableConstructor(table) => table,
            _ => return None,
        },
        _ => return None,
    };

    let mut fields = Vec::new();

    for field in table.fields() {
        let Field::NameKey { key, value, .. } = field else {
            continue;
        };

        let value_name = match value {
            Expression::Var(Var::Name(name)) => Some(token_text(name)),
            _ => None,
        };

        fields.push((token_text(key), value_name));
    }

    Some(fields)
}

fn luacats_export_class_name(parsed: &ParsedSource) -> String {
    let stem = parsed
        .path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("module");

    let mut output = String::new();
    let mut capitalize_next = true;

    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() {
            if capitalize_next {
                output.extend(ch.to_uppercase());
            } else {
                output.push(ch);
            }
            capitalize_next = false;
        } else {
            capitalize_next = true;
        }
    }

    if output.is_empty() {
        output.push_str("Module");
    }

    output.push_str("Exports");
    output
}

fn luacats_type(ty: &TuaType) -> String {
    match ty {
        TuaType::Named(name) => name.to_owned(),
        TuaType::StringLiteral(value) => format!("{value:?}"),
        TuaType::Union(types) => types
            .iter()
            .map(luacats_type)
            .collect::<Vec<_>>()
            .join(" | "),
        TuaType::Table(fields) => {
            let rendered_fields = fields
                .iter()
                .map(|field| {
                    format!(
                        "{}{}: {}",
                        field.name,
                        if field.optional { "?" } else { "" },
                        luacats_type(&field.ty)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");

            if rendered_fields.is_empty() {
                "{}".to_owned()
            } else {
                format!("{{ {} }}", rendered_fields)
            }
        }
        TuaType::Array(item) => format!("{}[]", luacats_type(item)),
        TuaType::Tuple(items) => format!(
            "[{}]",
            items
                .iter()
                .map(luacats_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TuaType::Dictionary { key, value } => {
            format!("{{ [{}]: {} }}", luacats_type(key), luacats_type(value))
        }
        TuaType::Function {
            parameters,
            returns,
        } => {
            let parameters = parameters
                .iter()
                .map(|parameter| {
                    if parameter.variadic {
                        return format!("...: {}", luacats_type(&parameter.ty));
                    }
                    let name = parameter
                        .name
                        .as_ref()
                        .map(|name| {
                            format!("{name}{}: ", if parameter.optional { "?" } else { "" })
                        })
                        .unwrap_or_default();
                    format!("{name}{}", luacats_type(&parameter.ty))
                })
                .collect::<Vec<_>>()
                .join(", ");
            let returns = returns
                .iter()
                .map(luacats_type)
                .collect::<Vec<_>>()
                .join(", ");

            if returns.is_empty() {
                format!("fun({parameters})")
            } else {
                format!("fun({parameters}): {returns}")
            }
        }
    }
}

fn luacats_function_type(function: &crate::FunctionAnnotation) -> String {
    let parameters = function
        .parameters
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name, luacats_type(&parameter.ty)))
        .collect::<Vec<_>>()
        .join(", ");

    if let Some(return_type) = &function.return_type {
        format!("fun({parameters}): {}", luacats_type(return_type))
    } else {
        format!("fun({parameters})")
    }
}

fn luacats_export_type(ty: &Type) -> Option<String> {
    match ty {
        Type::Any => None,
        Type::Primitive(_)
        | Type::Alias(_)
        | Type::StringLiteral(_)
        | Type::Union(_)
        | Type::Table(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => Some(luacats_type_from_type(ty)),
        Type::Function(function) => {
            let parameters = function
                .parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    let name = parameter
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("arg{}", index + 1));
                    format!("{name}: {}", luacats_type_from_type(&parameter.ty))
                })
                .collect::<Vec<_>>()
                .join(", ");
            let parameters = append_luacats_vararg(parameters, function.vararg_type.as_deref());
            let returns = function
                .return_types_or_any()
                .iter()
                .map(luacats_type_from_type)
                .collect::<Vec<_>>()
                .join(", ");

            Some(format!("fun({parameters}): {returns}"))
        }
    }
}

fn append_luacats_vararg(parameters: String, vararg_type: Option<&Type>) -> String {
    let Some(vararg_type) = vararg_type else {
        return parameters;
    };
    let vararg = format!("...: {}", luacats_type_from_type(vararg_type));

    if parameters.is_empty() {
        vararg
    } else {
        format!("{parameters}, {vararg}")
    }
}

fn luacats_type_from_type(ty: &Type) -> String {
    match ty {
        Type::Primitive(_) | Type::Any | Type::Alias(_) | Type::StringLiteral(_) => ty.to_string(),
        Type::Union(types) => types
            .iter()
            .map(luacats_type_from_type)
            .collect::<Vec<_>>()
            .join(" | "),
        Type::Function(function) => {
            let parameters = function
                .parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    let name = parameter
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("arg{}", index + 1));
                    format!("{name}: {}", luacats_type_from_type(&parameter.ty))
                })
                .collect::<Vec<_>>()
                .join(", ");
            let parameters = append_luacats_vararg(parameters, function.vararg_type.as_deref());
            let returns = function
                .return_types_or_any()
                .iter()
                .map(luacats_type_from_type)
                .collect::<Vec<_>>()
                .join(", ");

            format!("fun({parameters}): {returns}")
        }
        Type::Table(table) => {
            let fields = table
                .fields()
                .map(|(name, field)| {
                    format!(
                        "{}{}: {}",
                        name,
                        if field.optional { "?" } else { "" },
                        luacats_type_from_type(&field.ty)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");

            if fields.is_empty() {
                "{}".to_owned()
            } else {
                format!("{{ {fields} }}")
            }
        }
        Type::Array(item) => format!("{}[]", luacats_type_from_type(item)),
        Type::Tuple(items) => format!(
            "[{}]",
            items
                .iter()
                .map(luacats_type_from_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Type::Dictionary { key, value } => {
            format!(
                "{{ [{}]: {} }}",
                luacats_type_from_type(key),
                luacats_type_from_type(value)
            )
        }
    }
}

fn erase_ast(path: &Path, ast: ast::Ast, diagnostics: &mut Vec<Diagnostic>) -> ast::Ast {
    let ast = AnonymousFunctionEraser { path, diagnostics }.visit_ast(ast);
    let nodes = erase_block(path, ast.nodes().clone(), diagnostics);

    ast.with_nodes(nodes)
}

struct AnonymousFunctionEraser<'a> {
    path: &'a Path,
    diagnostics: &'a mut Vec<Diagnostic>,
}

impl VisitorMut for AnonymousFunctionEraser<'_> {
    fn visit_anonymous_function_end(
        &mut self,
        function: ast::AnonymousFunction,
    ) -> ast::AnonymousFunction {
        let body = erase_function_body(self.path, function.body().clone(), self.diagnostics);

        function.with_body(body)
    }
}

fn erase_block(path: &Path, block: ast::Block, diagnostics: &mut Vec<Diagnostic>) -> ast::Block {
    let stmts = block
        .stmts_with_semicolon()
        .cloned()
        .filter_map(|(stmt, semicolon)| {
            erase_stmt(path, stmt, diagnostics).map(|stmt| (stmt, semicolon))
        })
        .collect();
    let last_stmt = block.last_stmt_with_semicolon().cloned();

    block.with_stmts(stmts).with_last_stmt(last_stmt)
}

fn erase_stmt(
    path: &Path,
    stmt: ast::Stmt,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ast::Stmt> {
    match stmt {
        ast::Stmt::TypeDeclaration(_) => None,
        ast::Stmt::ExportedTypeDeclaration(_) => None,
        ast::Stmt::LocalAssignment(local) => {
            let had_type_specifier = local.type_specifiers().any(|specifier| specifier.is_some());
            let equal_token = if had_type_specifier && local.equal_token().is_some() {
                Some(spaced_symbol("="))
            } else {
                local.equal_token().cloned()
            };

            Some(ast::Stmt::LocalAssignment(
                local
                    .with_type_specifiers(Vec::new())
                    .with_equal_token(equal_token),
            ))
        }
        ast::Stmt::LocalFunction(function) => {
            let body = erase_function_body(path, function.body().clone(), diagnostics);

            Some(ast::Stmt::LocalFunction(function.with_body(body)))
        }
        ast::Stmt::FunctionDeclaration(function) => {
            let body = erase_function_body(path, function.body().clone(), diagnostics);

            Some(ast::Stmt::FunctionDeclaration(function.with_body(body)))
        }
        ast::Stmt::Do(block) => {
            let erased = erase_block(path, block.block().clone(), diagnostics);

            Some(ast::Stmt::Do(block.with_block(erased)))
        }
        ast::Stmt::If(if_stmt) => {
            let block = erase_block(path, if_stmt.block().clone(), diagnostics);
            let else_if = if_stmt.else_if().map(|items| {
                items
                    .iter()
                    .cloned()
                    .map(|else_if| erase_else_if(path, else_if, diagnostics))
                    .collect()
            });
            let else_block = if_stmt
                .else_block()
                .cloned()
                .map(|block| erase_block(path, block, diagnostics));

            Some(ast::Stmt::If(
                if_stmt
                    .with_block(block)
                    .with_else_if(else_if)
                    .with_else(else_block),
            ))
        }
        ast::Stmt::GenericFor(for_stmt) => {
            let block = erase_block(path, for_stmt.block().clone(), diagnostics);
            let had_type_specifier = for_stmt
                .type_specifiers()
                .any(|specifier| specifier.is_some());
            let for_stmt = if had_type_specifier {
                for_stmt.with_in_token(spaced_symbol("in"))
            } else {
                for_stmt
            };

            Some(ast::Stmt::GenericFor(
                for_stmt.with_type_specifiers(Vec::new()).with_block(block),
            ))
        }
        ast::Stmt::NumericFor(for_stmt) => {
            let block = erase_block(path, for_stmt.block().clone(), diagnostics);
            let had_type_specifier = for_stmt.type_specifier().is_some();
            let for_stmt = if had_type_specifier {
                for_stmt.with_equal_token(spaced_symbol("="))
            } else {
                for_stmt
            };

            Some(ast::Stmt::NumericFor(
                for_stmt.with_type_specifier(None).with_block(block),
            ))
        }
        ast::Stmt::Repeat(repeat) => {
            let block = erase_block(path, repeat.block().clone(), diagnostics);

            Some(ast::Stmt::Repeat(repeat.with_block(block)))
        }
        ast::Stmt::While(while_stmt) => {
            let block = erase_block(path, while_stmt.block().clone(), diagnostics);

            Some(ast::Stmt::While(while_stmt.with_block(block)))
        }
        _ => Some(stmt),
    }
}

fn erase_else_if(
    path: &Path,
    else_if: ast::ElseIf,
    diagnostics: &mut Vec<Diagnostic>,
) -> ast::ElseIf {
    let block = erase_block(path, else_if.block().clone(), diagnostics);

    else_if.with_block(block)
}

fn erase_function_body(
    path: &Path,
    body: ast::FunctionBody,
    diagnostics: &mut Vec<Diagnostic>,
) -> ast::FunctionBody {
    let block = erase_block(path, body.block().clone(), diagnostics);
    let parameters_parentheses = parameters_parentheses_without_return_type(&body);

    body.with_generics(None)
        .with_type_specifiers(Vec::new())
        .with_return_type(None)
        .with_parameters_parentheses(parameters_parentheses)
        .with_block(block)
}

fn parameters_parentheses_without_return_type(body: &ast::FunctionBody) -> ContainedSpan {
    let (open, close) = body.parameters_parentheses().tokens();

    let Some(return_type) = body.return_type() else {
        let close_text = close.to_string();
        let trailing = trailing_whitespace(&close_text);

        return ContainedSpan::new(open.clone(), symbol_with_trailing(close, trailing));
    };

    let return_type_text = return_type.to_string();
    let trailing = trailing_whitespace(&return_type_text);

    ContainedSpan::new(open.clone(), symbol_with_trailing(close, trailing))
}

fn symbol_with_trailing(token: &TokenReference, trailing: &str) -> TokenReference {
    TokenReference::symbol(&format!("{}{trailing}", token.to_string().trim()))
        .expect("existing syntax token should remain a valid symbol")
}

fn spaced_symbol(symbol: &str) -> TokenReference {
    TokenReference::symbol(&format!(" {symbol} ")).expect("spaced syntax token should be valid")
}

fn trailing_whitespace(text: &str) -> &str {
    let index = text.trim_end_matches(char::is_whitespace).len();

    &text[index..]
}

fn lower_compound_assignment_operations(
    document: &lowering::LoweringDocument,
) -> Vec<lowering::LoweringOperation> {
    let output = document.code();
    let mut operations = Vec::new();
    let mut offset = 0usize;

    for line in output.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let replacement = lower_compound_assignment_line(body);

        if replacement != body {
            operations.push(document.operation(
                LoweringKind::CompoundAssignment,
                offset..offset + body.len(),
                replacement,
            ));
        }
        offset += line.len();
    }

    operations
}

fn lower_compound_assignment_line(line: &str) -> String {
    let trimmed = line.trim_start_matches(char::is_whitespace);
    let indent = &line[..line.len() - trimmed.len()];
    let Some((lhs, rest)) = split_plain_field_chain_prefix(trimmed) else {
        return line.to_owned();
    };
    let rest = rest.trim_start();
    let Some((binary_operator, rhs)) = split_compound_operator(rest) else {
        return line.to_owned();
    };

    format!(
        "{indent}{lhs} = {lhs} {binary_operator} {}",
        rhs.trim_start()
    )
}

fn split_plain_field_chain_prefix(input: &str) -> Option<(&str, &str)> {
    let mut end = consume_identifier(input)?;

    loop {
        let rest = &input[end..];

        if !rest.starts_with('.') {
            break;
        }

        let field_end = consume_identifier(&rest[1..])?;
        end += 1 + field_end;
    }

    Some((&input[..end], &input[end..]))
}

fn consume_identifier(input: &str) -> Option<usize> {
    let mut chars = input.char_indices();
    let (_, first) = chars.next()?;

    if !is_identifier_start(first) {
        return None;
    }

    let mut end = first.len_utf8();

    for (index, character) in chars {
        if !is_identifier_part(character) {
            return Some(index);
        }

        end = index + character.len_utf8();
    }

    Some(end)
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_identifier_part(character: char) -> bool {
    is_identifier_start(character) || character.is_ascii_digit()
}

fn split_compound_operator(input: &str) -> Option<(&'static str, &str)> {
    [
        ("+=", "+"),
        ("-=", "-"),
        ("*=", "*"),
        ("/=", "/"),
        ("%=", "%"),
    ]
    .into_iter()
    .find_map(|(sugar, binary)| input.strip_prefix(sugar).map(|rhs| (binary, rhs)))
}

fn lower_builtin_call_operations(
    document: &lowering::LoweringDocument,
    parsed: &ParsedSource,
    ecs_enabled: bool,
) -> Vec<lowering::LoweringOperation> {
    let output = document.code();
    let mut operations = Vec::new();
    let mut index = 0;
    let mut brinevector_span = None;
    let mut tuasignal_span = None;
    let mut tuaobjectpool_span = None;
    let mut tuaecs_span = None;
    let explicit_ecs_binding = explicit_binding_position(parsed, ecs::MODULE_NAME);
    let explicit_object_pool_binding =
        explicit_binding_position(parsed, object_pool::CONSTRUCTOR_NAME);

    while index < output.len() {
        let rest = &output[index..];

        if rest.starts_with("--") {
            index = consume_comment(&output, index);
            continue;
        }

        if let Some(end) = consume_string_literal(&output, index) {
            index = end;
            continue;
        }

        if let Some(identifier_end) = consume_identifier(rest) {
            let name = &output[index..index + identifier_end];

            if can_start_asset_loader(&output, index) {
                if let Some(kind) = asset::loader_kind(name) {
                    let argument_start = skip_whitespace(&output, index + identifier_end);

                    if let Some((replacement, end)) =
                        lower_asset_loader_call(&output, argument_start, kind)
                    {
                        operations.push(document.operation(
                            LoweringKind::BuiltinCall,
                            index..end,
                            replacement,
                        ));
                        index = end;
                        continue;
                    }
                }
            }

            if can_start_builtin_call(&output, index) && vector::is_constructor(name) {
                let argument_start = skip_whitespace(&output, index + identifier_end);

                if let Some((replacement, end)) =
                    lower_vec2_constructor_call(&output, argument_start)
                {
                    let source_span = document.source_span_at(index);
                    brinevector_span.get_or_insert(source_span);
                    operations.push(lowering::LoweringOperation::replace(
                        index..end,
                        replacement,
                        source_span,
                        LoweringKind::BuiltinCall,
                    ));
                    index = end;
                    continue;
                }
            }

            if can_start_builtin_call(&output, index) && signal::is_constructor(name) {
                let argument_start = skip_whitespace(&output, index + identifier_end);

                if let Some((replacement, end)) = lower_runtime_constructor_call(
                    &output,
                    argument_start,
                    signal::EMITTED_CONSTRUCTOR_LOCAL,
                ) {
                    let source_span = document.source_span_at(index);
                    tuasignal_span.get_or_insert(source_span);
                    operations.push(lowering::LoweringOperation::replace(
                        index..end,
                        replacement,
                        source_span,
                        LoweringKind::BuiltinCall,
                    ));
                    index = end;
                    continue;
                }
            }

            if can_start_builtin_call(&output, index) && object_pool::is_constructor(name) {
                let argument_start = skip_whitespace(&output, index + identifier_end);
                let source_span = document.source_span_at(index);
                let binding_is_active = explicit_object_pool_binding.is_some_and(|position| {
                    (position.line, position.column)
                        <= (source_span.start.line, source_span.start.column)
                });

                if !binding_is_active {
                    if let Some((replacement, end)) = lower_runtime_constructor_call(
                        &output,
                        argument_start,
                        object_pool::EMITTED_CONSTRUCTOR_LOCAL,
                    ) {
                        tuaobjectpool_span.get_or_insert(source_span);
                        operations.push(lowering::LoweringOperation::replace(
                            index..end,
                            replacement,
                            source_span,
                            LoweringKind::BuiltinCall,
                        ));
                        index = end;
                        continue;
                    }
                }
            }

            if ecs_enabled && name == ecs::MODULE_NAME {
                let field_start = skip_whitespace(&output, index + identifier_end);

                if output[field_start..].starts_with('.') {
                    let source_span = document.source_span_at(index);
                    let binding_is_active = explicit_ecs_binding.is_some_and(|position| {
                        (position.line, position.column)
                            <= (source_span.start.line, source_span.start.column)
                    });

                    if !binding_is_active {
                        tuaecs_span.get_or_insert(source_span);
                    }
                }
            }

            index += identifier_end;
            continue;
        }

        let character = rest
            .chars()
            .next()
            .expect("loop only runs at valid string boundaries");

        index += character.len_utf8();
    }

    if brinevector_span.is_some()
        || tuasignal_span.is_some()
        || tuaobjectpool_span.is_some()
        || tuaecs_span.is_some()
    {
        let leading_whitespace =
            output.len() - output.trim_start_matches(char::is_whitespace).len();
        if leading_whitespace > 0 {
            operations.push(document.operation(
                LoweringKind::BuiltinCall,
                0..leading_whitespace,
                "",
            ));
        }
    }

    if let Some(source_span) = tuasignal_span {
        operations.push(lowering::LoweringOperation::insert(
            0,
            format!(
                "local {} = require(\"{}\")\n",
                signal::EMITTED_CONSTRUCTOR_LOCAL,
                signal::RUNTIME_MODULE,
            ),
            source_span,
            LoweringKind::BuiltinCall,
        ));
    }
    if let Some(source_span) = brinevector_span {
        operations.push(lowering::LoweringOperation::insert(
            0,
            format!(
                "local {} = require(\"{}\")\n",
                vector::EMITTED_CONSTRUCTOR_LOCAL,
                vector::RUNTIME_MODULE,
            ),
            source_span,
            LoweringKind::BuiltinCall,
        ));
    }
    if let Some(source_span) = tuaobjectpool_span {
        operations.push(lowering::LoweringOperation::insert(
            0,
            format!(
                "local {} = require(\"{}\")\n",
                object_pool::EMITTED_CONSTRUCTOR_LOCAL,
                object_pool::RUNTIME_MODULE,
            ),
            source_span,
            LoweringKind::BuiltinCall,
        ));
    }
    if let Some(source_span) = tuaecs_span {
        operations.push(lowering::LoweringOperation::insert(
            0,
            format!(
                "local {} = require(\"{}\")\n",
                ecs::MODULE_NAME,
                ecs::RUNTIME_MODULE
            ),
            source_span,
            LoweringKind::BuiltinCall,
        ));
    }

    operations
}

fn explicit_binding_position(parsed: &ParsedSource, binding: &str) -> Option<SourcePosition> {
    parsed
        .ast
        .nodes()
        .stmts()
        .filter_map(|stmt| match stmt {
            ast::Stmt::LocalAssignment(local) => local
                .names()
                .iter()
                .find(|name| token_text(name) == binding)
                .map(|name| SourcePosition::from_full_moon(name.start_position())),
            ast::Stmt::LocalFunction(function) if token_text(function.name()) == binding => Some(
                SourcePosition::from_full_moon(function.name().start_position()),
            ),
            ast::Stmt::Assignment(assignment) => {
                assignment
                    .variables()
                    .iter()
                    .find_map(|variable| match variable {
                        Var::Name(name) if token_text(name) == binding => {
                            Some(SourcePosition::from_full_moon(name.start_position()))
                        }
                        _ => None,
                    })
            }
            _ => None,
        })
        .min_by_key(|position| (position.line, position.column))
}

fn lower_import_call_operations(
    document: &lowering::LoweringDocument,
    parsed: &ParsedSource,
    import_resolver: ImportResolver<'_>,
) -> Result<Vec<lowering::LoweringOperation>, Vec<Diagnostic>> {
    let output = document.code();
    let mut operations = Vec::new();
    let mut diagnostics = Vec::new();
    let mut index = 0;

    while index < output.len() {
        let rest = &output[index..];

        if rest.starts_with("--") {
            index = consume_comment(&output, index);
            continue;
        }

        if let Some(end) = consume_string_literal(&output, index) {
            index = end;
            continue;
        }

        if let Some(identifier_end) = consume_identifier(rest) {
            let name = &output[index..index + identifier_end];

            if name == "import"
                && can_start_builtin_call(&output, index)
                && looks_like_local_import_binding(&output, index)
            {
                let argument_start = skip_whitespace(&output, index + identifier_end);
                let diagnostic_position =
                    source_position_tuple(document.source_span_at(index).start);

                if let Some((replacement, end)) = lower_import_call(
                    &output,
                    argument_start,
                    diagnostic_position,
                    parsed,
                    import_resolver,
                ) {
                    match replacement {
                        Ok(replacement) => operations.push(document.operation(
                            LoweringKind::Import,
                            index..end,
                            replacement,
                        )),
                        Err(diagnostic) => diagnostics.push(diagnostic),
                    }

                    index = end;
                    continue;
                }
            }

            index += identifier_end;
            continue;
        }

        let character = rest
            .chars()
            .next()
            .expect("loop only runs at valid string boundaries");

        index += character.len_utf8();
    }

    if diagnostics.is_empty() {
        Ok(operations)
    } else {
        Err(diagnostics)
    }
}

fn lower_import_call(
    output: &str,
    argument_start: usize,
    diagnostic_position: (usize, usize),
    parsed: &ParsedSource,
    import_resolver: ImportResolver<'_>,
) -> Option<(Result<String, Diagnostic>, usize)> {
    if !output[argument_start..].starts_with('(') {
        return None;
    }

    let close = find_matching_paren(output, argument_start)?;
    let arguments = split_top_level_arguments(&output[argument_start + 1..close])?;

    let replacement = match arguments.as_slice() {
        [argument] => {
            let Some(specifier) = short_string_literal_value(argument) else {
                return Some((
                    Err(import_emit_diagnostic(
                        parsed,
                        diagnostic_position,
                        "import(...) expects one static string literal",
                        DiagnosticCode::LINT_BAD_REQUIRE,
                    )),
                    close + 1,
                ));
            };

            if let Some(reason) = invalid_import_specifier_reason(&specifier) {
                return Some((
                    Err(import_emit_diagnostic(
                        parsed,
                        diagnostic_position,
                        reason,
                        DiagnosticCode::LINT_BAD_REQUIRE,
                    )),
                    close + 1,
                ));
            }

            let Some(resolver) = import_resolver else {
                if is_project_import_specifier(&specifier) {
                    return Some((
                        Ok(format!("require({})", lua_string_literal(&specifier))),
                        close + 1,
                    ));
                }

                return Some((
                    Err(import_emit_diagnostic(
                        parsed,
                        diagnostic_position,
                        "child-relative import cannot be emitted without project module resolution",
                        DiagnosticCode::LINT_BAD_REQUIRE,
                    )),
                    close + 1,
                ));
            };

            match resolver(&specifier) {
                Some(module) => Ok(format!("require({})", lua_string_literal(&module))),
                None => Err(import_emit_diagnostic(
                    parsed,
                    diagnostic_position,
                    format!("import not found: `{specifier}`"),
                    DiagnosticCode::LINT_MISSING_REQUIRE,
                )),
            }
        }
        _ => Err(import_emit_diagnostic(
            parsed,
            diagnostic_position,
            "import(...) expects exactly one module path",
            DiagnosticCode::LINT_BAD_REQUIRE,
        )),
    };

    Some((replacement, close + 1))
}

fn short_string_literal_value(input: &str) -> Option<String> {
    let mut chars = input.chars();
    let quote = match chars.next()? {
        '"' => '"',
        '\'' => '\'',
        _ => return None,
    };

    if !input.ends_with(quote) || input.len() < 2 {
        return None;
    }

    let inner = &input[quote.len_utf8()..input.len() - quote.len_utf8()];
    let mut output = String::new();
    let mut escaped = false;

    for character in inner.chars() {
        if escaped {
            output.push(match character {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                _ => character,
            });
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        output.push(character);
    }

    if escaped {
        output.push('\\');
    }

    Some(output)
}

fn lua_string_literal(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);

    output.push('"');

    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            _ => output.push(character),
        }
    }

    output.push('"');
    output
}

fn lower_default_parameter_operations(
    document: &lowering::LoweringDocument,
    parsed: &ParsedSource,
) -> Vec<lowering::LoweringOperation> {
    let output = document.code();
    let mut operations = Vec::new();
    let mut search_start = 0usize;

    for function in parsed
        .function_annotations
        .iter()
        .filter(|function| !function.default_parameters.is_empty())
    {
        let Some(name) = function.name.as_deref() else {
            continue;
        };
        let needle = format!("function {name}(");
        let Some(relative_start) = output[search_start..].find(&needle) else {
            continue;
        };
        let header_start = search_start + relative_start;
        let open = header_start + needle.len() - 1;
        let Some(close) = find_matching_paren(&output, open) else {
            continue;
        };
        let trimmed_close = output[..close].trim_end_matches([' ', '\t']).len();

        if trimmed_close < close {
            operations.push(document.operation(
                LoweringKind::DefaultParameter,
                trimmed_close..close,
                "",
            ));
        }

        let insert_at = output[close + 1..]
            .find('\n')
            .map(|offset| close + 1 + offset + 1)
            .unwrap_or(close + 1);
        let body_indent = format!("{}  ", line_indent_before(&output, header_start));

        for default in &function.default_parameters {
            let source_span = default
                .span
                .or(function.span)
                .unwrap_or_else(|| document.source_span_at(header_start));
            operations.push(lowering::LoweringOperation::insert(
                insert_at,
                default_parameter_guard_text(std::slice::from_ref(default), &body_indent),
                source_span,
                LoweringKind::DefaultParameter,
            ));
        }
        search_start = close + 1;
    }

    operations
}

fn default_parameter_guard_text(defaults: &[ParameterDefault], body_indent: &str) -> String {
    let nested_indent = format!("{body_indent}  ");
    let mut output = String::new();

    for default in defaults {
        output.push_str(body_indent);
        output.push_str("if ");
        output.push_str(&default.name);
        output.push_str(" == nil then\n");
        output.push_str(&nested_indent);
        output.push_str(&default.name);
        output.push_str(" = ");
        output.push_str(&default.expression);
        output.push('\n');
        output.push_str(body_indent);
        output.push_str("end\n");
    }

    output
}

fn line_indent_before(input: &str, index: usize) -> &str {
    let line_start = input[..index]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let line = &input[line_start..index];
    let indent_len = line
        .bytes()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count();

    &line[..indent_len]
}

fn invalid_import_specifier_reason(specifier: &str) -> Option<&'static str> {
    if is_child_relative_import_specifier(specifier) || is_project_import_specifier(specifier) {
        return None;
    }

    if is_parent_relative_import_specifier(specifier) {
        return Some("import specifier cannot traverse parent directories; use a project module name instead");
    }

    Some("import specifiers must be child-relative (`./foo`) or project modules (`engine.foo`)")
}

fn is_child_relative_import_specifier(specifier: &str) -> bool {
    specifier.starts_with("./") && !has_parent_path_component(specifier)
}

fn is_project_import_specifier(specifier: &str) -> bool {
    !specifier.is_empty() && !specifier.starts_with('.') && !has_parent_path_component(specifier)
}

fn is_parent_relative_import_specifier(specifier: &str) -> bool {
    specifier.starts_with("../")
        || (specifier.starts_with("./") && has_parent_path_component(specifier))
}

fn has_parent_path_component(specifier: &str) -> bool {
    Path::new(specifier)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
}

fn looks_like_local_import_binding(output: &str, import_start: usize) -> bool {
    let line_start = output[..import_start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let prefix = output[line_start..import_start].trim_start();
    let Some(before_import) = prefix.strip_prefix("local ") else {
        return false;
    };
    let before_import = before_import.trim_end();
    let Some(lhs) = before_import.strip_suffix('=') else {
        return false;
    };
    let lhs = lhs.trim();

    if lhs.contains(',') {
        return false;
    }

    let alias = lhs.split(':').next().map(str::trim).unwrap_or(lhs);

    !alias.is_empty() && is_identifier(alias)
}

fn is_identifier(input: &str) -> bool {
    let mut chars = input.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn import_emit_diagnostic(
    parsed: &ParsedSource,
    position: (usize, usize),
    message: impl Into<String>,
    code: DiagnosticCode,
) -> Diagnostic {
    Diagnostic::error(parsed.path.clone(), position.0, position.1, message).with_code(code)
}

fn source_position_from_byte(source: &str, byte: usize) -> SourcePosition {
    let mut line = 1usize;
    let mut column = 1usize;

    for (index, character) in source.char_indices() {
        if index >= byte {
            break;
        }

        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    SourcePosition { line, column }
}

fn source_position_tuple(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn lower_vec2_constructor_call(output: &str, argument_start: usize) -> Option<(String, usize)> {
    if !output[argument_start..].starts_with('(') {
        return None;
    }

    let close = find_matching_paren(output, argument_start)?;
    let arguments = split_top_level_arguments(&output[argument_start + 1..close])?;

    let [x, y] = arguments.as_slice() else {
        return None;
    };

    if x.is_empty() || y.is_empty() {
        return None;
    }

    Some((
        format!("{}({x}, {y})", vector::EMITTED_CONSTRUCTOR_LOCAL),
        close + 1,
    ))
}

fn lower_runtime_constructor_call(
    output: &str,
    argument_start: usize,
    emitted_constructor: &str,
) -> Option<(String, usize)> {
    if !output[argument_start..].starts_with('(') {
        return None;
    }

    let close = find_matching_paren(output, argument_start)?;
    let arguments = output[argument_start + 1..close].trim();

    Some((format!("{emitted_constructor}({arguments})"), close + 1))
}

fn lower_asset_loader_call(
    output: &str,
    argument_start: usize,
    kind: asset::AssetLoaderKind,
) -> Option<(String, usize)> {
    if kind.supports_string_sugar() {
        if let Some(path_end) = consume_string_literal(output, argument_start) {
            let path_literal = output[argument_start..path_end].to_owned();

            return Some((kind.emitted_call(&[path_literal])?, path_end));
        }
    }

    if !output[argument_start..].starts_with('(') {
        return None;
    }

    let close = find_matching_paren(output, argument_start)?;
    let arguments = split_top_level_arguments(&output[argument_start + 1..close])?;
    let arguments = arguments.into_iter().map(str::to_owned).collect::<Vec<_>>();

    if arguments
        .iter()
        .take(kind.required_argument_count())
        .any(|argument| argument.is_empty())
    {
        return None;
    }

    Some((kind.emitted_call(&arguments)?, close + 1))
}

fn can_start_asset_loader(output: &str, index: usize) -> bool {
    can_start_builtin_call(output, index)
}

fn can_start_builtin_call(output: &str, index: usize) -> bool {
    let Some(previous) = previous_char(output, index) else {
        return true;
    };

    !is_identifier_part(previous) && previous != '.' && previous != ':'
}

fn find_matching_paren(output: &str, open: usize) -> Option<usize> {
    if !output[open..].starts_with('(') {
        return None;
    }

    let mut depth = 0usize;
    let mut index = open;

    while index < output.len() {
        if let Some(end) = consume_string_literal(output, index) {
            index = end;
            continue;
        }

        let character = output[index..]
            .chars()
            .next()
            .expect("index should stay on a valid string boundary");

        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;

                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }

        index += character.len_utf8();
    }

    None
}

fn split_top_level_arguments(input: &str) -> Option<Vec<&str>> {
    let mut arguments = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut index = 0;

    while index < input.len() {
        if let Some(end) = consume_string_literal(input, index) {
            index = end;
            continue;
        }

        let character = input[index..]
            .chars()
            .next()
            .expect("index should stay on a valid string boundary");

        match character {
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                arguments.push(input[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }

        index += character.len_utf8();
    }

    arguments.push(input[start..].trim());
    Some(arguments)
}

fn previous_char(output: &str, index: usize) -> Option<char> {
    output[..index].chars().next_back()
}

fn skip_whitespace(output: &str, mut index: usize) -> usize {
    while index < output.len() {
        let character = output[index..]
            .chars()
            .next()
            .expect("index should stay on a valid string boundary");

        if !character.is_whitespace() {
            break;
        }

        index += character.len_utf8();
    }

    index
}

fn consume_comment(output: &str, index: usize) -> usize {
    if let Some(end) = consume_long_bracket(output, index + 2) {
        return end;
    }

    output[index..]
        .find('\n')
        .map_or(output.len(), |offset| index + offset)
}

fn consume_string_literal(output: &str, index: usize) -> Option<usize> {
    consume_short_string_literal(output, index).or_else(|| consume_long_bracket(output, index))
}

fn consume_short_string_literal(output: &str, index: usize) -> Option<usize> {
    let quote = match output[index..].chars().next()? {
        '"' => '"',
        '\'' => '\'',
        _ => return None,
    };
    let mut escaped = false;
    let mut cursor = index + quote.len_utf8();

    while cursor < output.len() {
        let character = output[cursor..]
            .chars()
            .next()
            .expect("cursor should stay on a valid string boundary");

        cursor += character.len_utf8();

        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        if character == quote {
            return Some(cursor);
        }
    }

    None
}

fn consume_long_bracket(output: &str, index: usize) -> Option<usize> {
    let rest = &output[index..];

    if !rest.starts_with('[') {
        return None;
    }

    let mut equals_count = 0;
    let mut cursor = index + 1;

    while output[cursor..].starts_with('=') {
        equals_count += 1;
        cursor += 1;
    }

    if !output[cursor..].starts_with('[') {
        return None;
    }

    cursor += 1;

    let closing = format!("]{}]", "=".repeat(equals_count));
    output[cursor..]
        .find(&closing)
        .map(|offset| cursor + offset + closing.len())
}

fn reclassify_compound_assignment_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|mut diagnostic| {
            if diagnostic.code == Some(DiagnosticCode::SYNTAX_COMPOUND_ASSIGNMENT) {
                diagnostic.code = Some(DiagnosticCode::LOWERING_COMPOUND_ASSIGNMENT);
            }

            diagnostic
        })
        .collect()
}

#[cfg(test)]
mod tests;
