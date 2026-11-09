use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use full_moon::ast::span::ContainedSpan;
use full_moon::ast::{self, Expression, Field, LastStmt, LuaVersion, Var};
use full_moon::tokenizer::{Token, TokenKind, TokenReference};
use full_moon::visitors::{Visitor, VisitorMut};

use crate::classes;
use crate::runtime_type;
use crate::semantic::SemanticModel;
use crate::types::{build_type_environment_with_diagnostics, TypeEnvironment};
use crate::{
    audio, ecs, game_math, infer_module_exports, input, object_pool, parse_tua_source, signal,
    state_machine, store, vector, AnnotationMode, Diagnostic, DiagnosticCode, ParameterDefault,
    ParsedSource, ProjectWorkspace, SourcePosition, SourceSpan, TuaType, Type,
};

mod builtin_calls;
pub(crate) mod lowering;
mod vec2_fusion;

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

/// Emits one project source with semantic facts supplied by an analysis
/// snapshot. Build frontends should prefer this over rebuilding a file-local
/// environment, because imported module values may carry types that affect
/// lowering decisions such as Vec2 fusion.
pub fn emit_tua_source_with_workspace_environment_artifact(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    annotation_mode: AnnotationMode,
    workspace: &ProjectWorkspace,
    environment: &TypeEnvironment,
) -> Result<LuaEmission, Vec<Diagnostic>> {
    let parsed =
        parse_tua_source(path, source).map_err(reclassify_compound_assignment_diagnostics)?;
    let resolver = |specifier: &str| workspace.module_name_for_import(&parsed.path, specifier);

    emit_lua_artifact_with_mode_import_resolver_and_environment(
        &parsed,
        annotation_mode,
        Some(&resolver),
        workspace.config().ecs.enabled,
        Some(environment),
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

#[cfg(test)]
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
    emit_lua_artifact_with_mode_import_resolver_and_environment(
        parsed,
        annotation_mode,
        import_resolver,
        ecs_enabled,
        None,
    )
}

pub(crate) fn emit_lua_artifact_with_mode_import_resolver_and_environment(
    parsed: &ParsedSource,
    annotation_mode: AnnotationMode,
    import_resolver: ImportResolver<'_>,
    ecs_enabled: bool,
    environment: Option<&TypeEnvironment>,
) -> Result<LuaEmission, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    // Emission features that need semantic facts share one environment build.
    // In particular, Vec2 fusion must not add another whole-file analysis pass
    // before runtime `is` lowering performs the same work.
    let built_environment = environment
        .is_none()
        .then(|| build_type_environment_with_diagnostics(parsed).0);
    let environment = environment
        .or(built_environment.as_ref())
        .expect("an environment is supplied or built locally");
    let ast = vec2_fusion::fuse_vec2_arithmetic(parsed, environment, parsed.ast.clone());
    let (ast, mut builtin_constructor_spans) = builtin_calls::lower(ast, parsed);
    let ast = erase_ast(&parsed.path, ast, &mut diagnostics);

    if diagnostics.is_empty() {
        let mut document = lowering::LoweringDocument::from_erased_ast(&ast, parsed);

        let operations =
            lower_default_parameter_operations(&document, parsed, &mut builtin_constructor_spans)?;
        apply_lowering_operations(&mut document, operations, parsed)?;

        let operations = lower_builtin_call_operations(
            &document,
            parsed,
            ecs_enabled,
            builtin_constructor_spans,
            environment,
        );
        apply_lowering_operations(&mut document, operations, parsed)?;

        let operations = lower_import_call_operations(&document, parsed, import_resolver)?;
        apply_lowering_operations(&mut document, operations, parsed)?;

        let operations = lower_runtime_type_check_operations(&document, parsed, environment)?;
        apply_lowering_operations(&mut document, operations, parsed)?;

        let operations = lower_class_method_call_operations(&document, parsed);
        apply_lowering_operations(&mut document, operations, parsed)?;

        let operations = lower_compound_assignment_operations(&document);
        apply_lowering_operations(&mut document, operations, parsed)?;

        let operations = crate::sprite::lowering_operations(&document, parsed);
        apply_lowering_operations(&mut document, operations, parsed)?;

        if annotation_mode == AnnotationMode::Luacats {
            let operations = luacats_annotation_operations(&document, parsed, environment);
            apply_lowering_operations(&mut document, operations, parsed)?;
        }

        validate_lua51_emission(parsed, document.finish())
    } else {
        Err(diagnostics)
    }
}

fn validate_lua51_emission(
    parsed: &ParsedSource,
    emission: LuaEmission,
) -> Result<LuaEmission, Vec<Diagnostic>> {
    let Err(diagnostics) = crate::parse_raw_lua_source(&parsed.path, emission.code.clone()) else {
        return Ok(emission);
    };

    let mut seen_ranges = BTreeSet::new();
    let diagnostics = diagnostics
        .into_iter()
        .map(|diagnostic| lua51_compatibility_diagnostic(parsed, &emission, diagnostic))
        .filter(|diagnostic| {
            seen_ranges.insert((
                diagnostic.line,
                diagnostic.column,
                diagnostic.end_line,
                diagnostic.end_column,
            ))
        })
        .collect();

    Err(diagnostics)
}

fn lua51_compatibility_diagnostic(
    parsed: &ParsedSource,
    emission: &LuaEmission,
    generated: Diagnostic,
) -> Diagnostic {
    let source_span = emission
        .provenance
        .iter()
        .find(|line| line.generated_line == generated.line)
        .map(|line| line.source_span)
        .or(parsed.span)
        .unwrap_or(SourceSpan {
            start: SourcePosition { line: 1, column: 1 },
            end: SourcePosition { line: 1, column: 2 },
        });

    Diagnostic::error(
        parsed.path.clone(),
        source_span.start.line,
        source_span.start.column,
        format!(
            "compiler output is not valid Lua 5.1 after lowering: {}",
            generated.message
        ),
    )
    .with_range(
        source_span.start.line,
        source_span.start.column,
        source_span.end.line,
        source_span.end.column,
    )
    .with_code(DiagnosticCode::EMIT_UNSUPPORTED_SYNTAX)
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
    environment: &TypeEnvironment,
) -> Result<Vec<lowering::LoweringOperation>, Vec<Diagnostic>> {
    let output = document.code();
    let semantic = SemanticModel::new(parsed, environment);
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
                    environment,
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

fn lower_class_method_call_operations(
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
                if let Some((replacement, end)) =
                    class_method_call_replacement(&output, index, receiver, class_name, parsed)
                {
                    operations.push(document.operation(
                        LoweringKind::ClassCall,
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

fn class_method_call_replacement(
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

    if !classes::class_method_exists(&parsed.class_declarations, class_name, method) {
        return None;
    }

    cursor = skip_whitespace(output, method_start + method_end);
    if !output[cursor..].starts_with('(') {
        return None;
    }

    let close = find_matching_paren(output, cursor)?;
    let args = output[cursor + 1..close].trim();
    let is_final = classes::class_method_is_final(&parsed.class_declarations, class_name, method);
    if !is_final && separator == ':' {
        return None;
    }

    let target = if is_final {
        let owner = classes::class_method_owner(&parsed.class_declarations, class_name, method)?;
        format!("{owner}_{method}")
    } else {
        format!("{receiver}.{method}")
    };
    let replacement = if args.is_empty() {
        format!("{target}({receiver})")
    } else {
        format!("{target}({receiver}, {args})")
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
    environment: &TypeEnvironment,
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
    let type_renderer = LuacatsTypeRenderer::new(parsed, environment);

    for (index, alias) in parsed.type_aliases.iter().enumerate() {
        let mut text = format!(
            "---@alias {} {}\n",
            alias.name,
            type_renderer.tua_type(&alias.ty, 0)
        );
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
        let type_label = type_renderer.tua_type(&local.ty, 0);
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
                    type_renderer.tua_type(&param.ty, 0)
                ));
            }

            if let Some(return_type) = function.return_type.as_ref() {
                comments.push(format!(
                    "---@return {}",
                    luacats_return_list(return_type, |ty| type_renderer.tua_type(ty, 0))
                ));
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
                .or_insert_with(|| type_renderer.function_annotation(function));
        }
    }

    let export_annotations = module_export_annotations(
        parsed,
        &local_export_types,
        &function_export_types,
        &type_renderer,
    );
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
    } else {
        line.strip_prefix("function ")?.trim_start()
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
    let first = iter.peek()?;

    if !is_identifier_start(*first) {
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
    type_renderer: &LuacatsTypeRenderer<'_>,
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
                    .and_then(|field| type_renderer.export_type(&field.ty))
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

struct LuacatsTypeRenderer<'a> {
    environment: &'a TypeEnvironment,
    imported_names: BTreeSet<String>,
    imported_namespaces: Vec<String>,
}

impl<'a> LuacatsTypeRenderer<'a> {
    fn new(parsed: &ParsedSource, environment: &'a TypeEnvironment) -> Self {
        let mut imported_names = BTreeSet::new();
        let mut imported_namespaces = Vec::new();
        for import in &parsed.type_imports {
            if import.reexport {
                continue;
            }
            if !import.namespace.is_empty() {
                imported_namespaces.push(format!("{}.", import.namespace));
            }
            imported_names.extend(import.bindings.iter().map(|binding| binding.local.clone()));
        }
        Self {
            environment,
            imported_names,
            imported_namespaces,
        }
    }

    fn imported(&self, name: &str) -> bool {
        self.imported_names.contains(name)
            || self
                .imported_namespaces
                .iter()
                .any(|namespace| name.starts_with(namespace))
    }

    fn resolved_import(&self, name: &str, depth: usize) -> Option<Type> {
        if depth >= 32 || !self.imported(name) {
            return None;
        }
        let resolved = self.environment.resolve_aliases(&Type::alias(name));
        (!matches!(&resolved, Type::Alias(resolved_name) if resolved_name == name))
            .then_some(resolved)
    }

    fn tua_type(&self, ty: &TuaType, depth: usize) -> String {
        if self.imported_names.is_empty() && self.imported_namespaces.is_empty() {
            return luacats_type(ty);
        }
        match ty {
            TuaType::Named(name) => self
                .resolved_import(name.trim(), depth)
                .map(|resolved| self.inferred_type(&resolved, depth + 1))
                .unwrap_or_else(|| name.trim().to_owned()),
            TuaType::Parameter { name, .. } => name.clone(),
            TuaType::Applied {
                name, arguments, ..
            } => format!(
                "{}<{}>",
                name.trim(),
                arguments
                    .iter()
                    .map(|argument| self.tua_type(argument, depth + 1))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            TuaType::StringLiteral(value) => format!("{value:?}"),
            TuaType::Union(types) => types
                .iter()
                .map(|ty| self.tua_type(ty, depth + 1))
                .collect::<Vec<_>>()
                .join(" | "),
            TuaType::Table(fields) => {
                let fields = fields
                    .iter()
                    .map(|field| {
                        format!(
                            "{}{}: {}",
                            field.name,
                            if field.optional { "?" } else { "" },
                            self.tua_type(&field.ty, depth + 1)
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
            TuaType::Array(item) => {
                let item_text = self.tua_type(item, depth + 1);
                if matches!(item.as_ref(), TuaType::Union(_) | TuaType::Function { .. }) {
                    format!("({item_text})[]")
                } else {
                    format!("{item_text}[]")
                }
            }
            TuaType::Tuple(items) => format!(
                "[{}]",
                items
                    .iter()
                    .map(|ty| self.tua_type(ty, depth + 1))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            TuaType::Dictionary { key, value } => format!(
                "{{ [{}]: {} }}",
                self.tua_type(key, depth + 1),
                self.tua_type(value, depth + 1)
            ),
            TuaType::Function {
                parameters,
                returns,
            } => {
                let parameters = parameters
                    .iter()
                    .map(|parameter| {
                        if parameter.variadic {
                            return format!("...: {}", self.tua_type(&parameter.ty, depth + 1));
                        }
                        let name = parameter
                            .name
                            .as_ref()
                            .map(|name| {
                                format!("{name}{}: ", if parameter.optional { "?" } else { "" })
                            })
                            .unwrap_or_default();
                        format!("{name}{}", self.tua_type(&parameter.ty, depth + 1))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let returns = returns
                    .iter()
                    .map(|ty| self.tua_type(ty, depth + 1))
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

    fn inferred_type(&self, ty: &Type, depth: usize) -> String {
        if let Type::Alias(name) = ty {
            if let Some(resolved) = self.resolved_import(name, depth) {
                return luacats_type_from_type(&resolved);
            }
        }
        luacats_type_from_type(ty)
    }

    fn function_annotation(&self, function: &crate::FunctionAnnotation) -> String {
        if self.imported_names.is_empty() && self.imported_namespaces.is_empty() {
            return luacats_function_type(function);
        }
        let parameters = function
            .parameters
            .iter()
            .map(|parameter| format!("{}: {}", parameter.name, self.tua_type(&parameter.ty, 0)))
            .collect::<Vec<_>>()
            .join(", ");
        function.return_type.as_ref().map_or_else(
            || format!("fun({parameters})"),
            |return_type| {
                format!(
                    "fun({parameters}): {}",
                    luacats_return_list(return_type, |ty| self.tua_type(ty, 0))
                )
            },
        )
    }

    fn export_type(&self, ty: &Type) -> Option<String> {
        if self.imported_names.is_empty() && self.imported_namespaces.is_empty() {
            return luacats_export_type(ty);
        }
        (!matches!(ty, Type::Any)).then(|| self.inferred_type(ty, 0))
    }
}

fn luacats_type(ty: &TuaType) -> String {
    match ty {
        TuaType::Named(name) => name.to_owned(),
        TuaType::Parameter { name, .. } => name.clone(),
        TuaType::Applied {
            name, arguments, ..
        } => format!(
            "{}<{}>",
            name,
            arguments
                .iter()
                .map(luacats_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
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
        TuaType::Array(item) => format!("{}[]", luacats_array_item(item)),
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

fn luacats_array_item(ty: &TuaType) -> String {
    let rendered = luacats_type(ty);

    if matches!(ty, TuaType::Union(_) | TuaType::Function { .. }) {
        format!("({rendered})")
    } else {
        rendered
    }
}

fn luacats_return_list(ty: &TuaType, render: impl Fn(&TuaType) -> String) -> String {
    // Tuples in native return position are multiple values. Elsewhere a
    // LuaCATS tuple remains a single positional table type.
    match ty {
        TuaType::Tuple(items) => items.iter().map(render).collect::<Vec<_>>().join(", "),
        ty => render(ty),
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
        format!(
            "fun({parameters}): {}",
            luacats_return_list(return_type, luacats_type)
        )
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
        | Type::Dictionary { .. }
        | Type::GenericParameter { .. }
        | Type::GenericAliasApplication { .. }
        | Type::GenericClassApplication { .. } => Some(luacats_type_from_type(ty)),
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
        Type::Primitive(_)
        | Type::Any
        | Type::Alias(_)
        | Type::StringLiteral(_)
        | Type::GenericParameter { .. }
        | Type::GenericAliasApplication { .. }
        | Type::GenericClassApplication { .. } => ty.to_string(),
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
        Type::Array(item) => format!("{}[]", luacats_array_item_from_type(item)),
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

fn luacats_array_item_from_type(ty: &Type) -> String {
    let rendered = luacats_type_from_type(ty);

    if matches!(ty, Type::Union(_) | Type::Function(_)) {
        format!("({rendered})")
    } else {
        rendered
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

            // Without `=`, the erased annotation's trailing trivia (the line
            // break or comment after `local x: T`) must stay after the name.
            let names = match local.type_specifiers().last().flatten() {
                Some(specifier) if equal_token.is_none() => {
                    let trailing: Vec<_> = full_moon::node::Node::tokens(specifier)
                        .last()
                        .map(|token| token.trailing_trivia().cloned().collect())
                        .unwrap_or_default();
                    let mut names = local.names().clone();
                    if let Some(name) = names.iter_mut().last() {
                        let mut name_trailing: Vec<_> = name.trailing_trivia().cloned().collect();
                        name_trailing.extend(trailing);
                        *name = TokenReference::new(
                            name.leading_trivia().cloned().collect(),
                            name.token().clone(),
                            name_trailing,
                        );
                    }
                    names
                }
                _ => local.names().clone(),
            };

            Some(ast::Stmt::LocalAssignment(
                local
                    .with_names(names)
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
    let mut open_trailing = open.trailing_trivia().cloned().collect::<Vec<_>>();
    if let Some(generics) = body.generics() {
        open_trailing.extend(generic_comment_trivia(generics));
    }
    let mut close_trailing = close.trailing_trivia().cloned().collect::<Vec<_>>();
    if let Some(return_type) = body.return_type() {
        // Trivia belongs to tokens, not their rendered symbol text. In particular,
        // a trailing header comment must survive erasing the return annotation.
        if let Some(last) = trailing_type_token(return_type.type_info()) {
            close_trailing.extend(last.trailing_trivia().cloned());
        }
    }

    ContainedSpan::new(
        TokenReference::new(
            open.leading_trivia().cloned().collect(),
            open.token().clone(),
            open_trailing,
        ),
        TokenReference::new(
            close.leading_trivia().cloned().collect(),
            close.token().clone(),
            close_trailing,
        ),
    )
}

fn generic_comment_trivia(generics: &ast::luau::GenericDeclaration) -> Vec<Token> {
    let mut trivia = full_moon::node::Node::tokens(generics)
        .flat_map(|reference| {
            reference
                .leading_trivia()
                .chain(reference.trailing_trivia())
        })
        .cloned()
        .collect::<Vec<_>>();
    trivia.sort_by_key(|token| token.start_position().bytes());
    let comments = trivia
        .iter()
        .filter(|token| {
            matches!(
                token.token_kind(),
                TokenKind::SingleLineComment | TokenKind::MultiLineComment
            )
        })
        .collect::<Vec<_>>();
    let (Some(first), Some(last)) = (comments.first(), comments.last()) else {
        return Vec::new();
    };
    let first_start = first.start_position().bytes();
    let last_end = last.end_position().bytes();
    let mut preserved = trivia
        .iter()
        .filter(|token| {
            token.start_position().bytes() >= first_start
                && token.end_position().bytes() <= last_end
        })
        .cloned()
        .collect::<Vec<_>>();

    if last.token_kind() == TokenKind::SingleLineComment {
        if let Some(newline) = trivia.iter().find(|token| {
            token.start_position().bytes() >= last_end
                && token.token_kind() == TokenKind::Whitespace
                && token.to_string().contains('\n')
        }) {
            preserved.push(newline.clone());
        }
    }

    preserved
}

// Full Moon's token iterator visits contained delimiters before their contents,
// so `.tokens().last()` is not the lexical last token for records or tuples.
fn trailing_type_token(ty: &ast::luau::TypeInfo) -> Option<&TokenReference> {
    use ast::luau::TypeInfo;
    match ty {
        TypeInfo::Array { braces, .. } | TypeInfo::Table { braces, .. } => Some(braces.tokens().1),
        TypeInfo::Tuple { parentheses, .. } | TypeInfo::Typeof { parentheses, .. } => {
            Some(parentheses.tokens().1)
        }
        TypeInfo::Generic { arrows, .. } => Some(arrows.tokens().1),
        TypeInfo::Callback { return_type, .. }
        | TypeInfo::Variadic {
            type_info: return_type,
            ..
        } => trailing_type_token(return_type),
        TypeInfo::Union(union) => union.types().iter().last().and_then(trailing_type_token),
        TypeInfo::Intersection(intersection) => intersection
            .types()
            .iter()
            .last()
            .and_then(trailing_type_token),
        _ => full_moon::node::Node::tokens(ty).last(),
    }
}

fn spaced_symbol(symbol: &str) -> TokenReference {
    TokenReference::symbol(&format!(" {symbol} ")).expect("spaced syntax token should be valid")
}

fn lower_compound_assignment_operations(
    document: &lowering::LoweringDocument,
) -> Vec<lowering::LoweringOperation> {
    let output = document.code();
    let Ok(ast) = full_moon::parse_fallible(&output, LuaVersion::luau()).into_result() else {
        // The final Lua 5.1 validation reports malformed intermediate output
        // with source provenance. Avoid turning an internal lowering failure
        // into a compiler panic here.
        return Vec::new();
    };
    let mut collector = CompoundAssignmentCollector::default();
    collector.visit_ast(&ast);

    let byte_range = |node: &dyn full_moon::node::Node| {
        let (start, end) = node.range()?;
        let start =
            crate::source_text::byte_for_position(&output, SourcePosition::from_full_moon(start))?;
        let end =
            crate::source_text::byte_for_position(&output, SourcePosition::from_full_moon(end))?;
        Some(start..end)
    };

    collector
        .assignments
        .into_iter()
        .filter_map(|assignment| {
            let binary_operator = compound_binary_operator(assignment.compound_operator())?;
            let operator = byte_range(assignment.compound_operator().token())?;
            // A comment on the line above the statement is the target's leading
            // trivia; it stays where it is instead of being copied into the
            // right-hand side.
            let lhs = assignment.lhs().to_string();
            let leading = full_moon::node::Node::tokens(assignment.lhs())
                .min_by_key(|token| token.token().start_position().bytes())
                .map(|first| first.leading_trivia().map(ToString::to_string).collect())
                .unwrap_or_else(String::new);
            let lhs = lhs.strip_prefix(leading.as_str()).unwrap_or(&lhs);
            let mut operations = vec![document.operation(
                LoweringKind::CompoundAssignment,
                operator,
                format!("= {} {binary_operator}", lhs.trim()),
            )];

            // `x *= a + b` means `x = x * (a + b)`, so a binary right-hand
            // side keeps its grouping.
            if matches!(assignment.rhs(), Expression::BinaryOperator { .. }) {
                let rhs = byte_range(assignment.rhs())?;
                operations.push(document.operation(
                    LoweringKind::CompoundAssignment,
                    rhs.start..rhs.start,
                    "(",
                ));
                operations.push(document.operation(
                    LoweringKind::CompoundAssignment,
                    rhs.end..rhs.end,
                    ")",
                ));
            }

            Some(operations)
        })
        .flatten()
        .collect()
}

#[derive(Default)]
struct CompoundAssignmentCollector {
    assignments: Vec<ast::CompoundAssignment>,
}

impl Visitor for CompoundAssignmentCollector {
    fn visit_compound_assignment(&mut self, assignment: &ast::CompoundAssignment) {
        self.assignments.push(assignment.clone());
    }
}

fn compound_binary_operator(operator: &ast::CompoundOp) -> Option<&'static str> {
    match operator {
        ast::CompoundOp::PlusEqual(_) => Some("+"),
        ast::CompoundOp::MinusEqual(_) => Some("-"),
        ast::CompoundOp::StarEqual(_) => Some("*"),
        ast::CompoundOp::SlashEqual(_) => Some("/"),
        ast::CompoundOp::PercentEqual(_) => Some("%"),
        _ => None,
    }
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

fn lower_builtin_call_operations(
    document: &lowering::LoweringDocument,
    parsed: &ParsedSource,
    ecs_enabled: bool,
    builtin_constructor_spans: BTreeMap<String, SourceSpan>,
    environment: &TypeEnvironment,
) -> Vec<lowering::LoweringOperation> {
    let output = document.code();
    let mut operations = Vec::new();
    let mut index = 0;
    let brinevector_span = builtin_constructor_spans
        .get(vector::CONSTRUCTOR_NAME)
        .copied();
    let tuasignal_span = builtin_constructor_spans
        .get(signal::CONSTRUCTOR_NAME)
        .copied();
    let tuaobjectpool_span = builtin_constructor_spans
        .get(object_pool::CONSTRUCTOR_NAME)
        .copied();
    let tuastatemachine_span = builtin_constructor_spans
        .get(state_machine::CONSTRUCTOR_NAME)
        .copied();
    let tuastore_span = builtin_constructor_spans
        .get(store::CONSTRUCTOR_NAME)
        .copied();
    let tuaaudio_span = builtin_constructor_spans
        .get(audio::CONSTRUCTOR_NAME)
        .copied();
    let baton_span = builtin_constructor_spans
        .get(input::CONSTRUCTOR_NAME)
        .copied();
    let mut tuaecs_span = None;
    let game_math_helpers = game_math::used_helpers(parsed);
    let explicit_ecs_binding = explicit_binding_position(parsed, ecs::MODULE_NAME);
    let scope = crate::scope::scope_map_parsed_source(parsed);
    let builtin_module_spans = crate::builtin_module::MODULES
        .iter()
        .map(|module| {
            if environment.project_owns_module(module.name) {
                return None;
            }
            scope.references().iter().find_map(|reference| {
                (reference.name == module.name
                    && matches!(&reference.resolution,
                crate::scope::ScopeResolution::Global(global)
                    if global.kind == crate::scope::ScopeGlobalKind::LuaBuiltin))
                .then_some(reference.span)
            })
        })
        .collect::<Vec<_>>();
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
        || tuastatemachine_span.is_some()
        || tuastore_span.is_some()
        || baton_span.is_some()
        || tuaaudio_span.is_some()
        || tuaecs_span.is_some()
        || builtin_module_spans.iter().any(Option::is_some)
        || !game_math_helpers.is_empty()
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
    if let Some(source_span) = tuastatemachine_span {
        operations.push(lowering::LoweringOperation::insert(
            0,
            format!(
                "local {} = require(\"{}\").create\n",
                state_machine::EMITTED_CONSTRUCTOR_LOCAL,
                state_machine::RUNTIME_MODULE,
            ),
            source_span,
            LoweringKind::BuiltinCall,
        ));
    }
    if let Some(source_span) = tuastore_span {
        operations.push(lowering::LoweringOperation::insert(
            0,
            format!(
                "local {} = require(\"{}\")\n",
                store::EMITTED_CONSTRUCTOR_LOCAL,
                store::RUNTIME_MODULE,
            ),
            source_span,
            LoweringKind::BuiltinCall,
        ));
    }
    if let Some(source_span) = baton_span {
        operations.push(lowering::LoweringOperation::insert(
            0,
            format!(
                "local {} = require(\"{}\").new\n",
                input::EMITTED_CONSTRUCTOR_LOCAL,
                input::RUNTIME_MODULE,
            ),
            source_span,
            LoweringKind::BuiltinCall,
        ));
    }
    if let Some(source_span) = tuaaudio_span {
        operations.push(lowering::LoweringOperation::insert(
            0,
            format!(
                "local {} = require(\"{}\")\n",
                audio::EMITTED_CONSTRUCTOR_LOCAL,
                audio::RUNTIME_MODULE,
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
    for (module, source_span) in crate::builtin_module::MODULES
        .iter()
        .zip(builtin_module_spans)
    {
        if let Some(source_span) = source_span {
            operations.push(lowering::LoweringOperation::insert(
                0,
                format!("local {} = require(\"{}\")\n", module.name, module.runtime),
                source_span,
                LoweringKind::BuiltinCall,
            ));
        }
    }
    for used in game_math_helpers {
        operations.push(lowering::LoweringOperation::insert(
            0,
            used.helper.definition(used.namespace),
            used.source_span,
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
    builtin_constructor_spans: &mut BTreeMap<String, SourceSpan>,
) -> Result<Vec<lowering::LoweringOperation>, Vec<Diagnostic>> {
    lower_default_parameter_operations_with_ancestors(
        document,
        parsed,
        builtin_constructor_spans,
        &[],
    )
}

fn lower_default_parameter_operations_with_ancestors(
    document: &lowering::LoweringDocument,
    parsed: &ParsedSource,
    builtin_constructor_spans: &mut BTreeMap<String, SourceSpan>,
    ancestors: &[(&crate::scope::ScopeMap, SourcePosition)],
) -> Result<Vec<lowering::LoweringOperation>, Vec<Diagnostic>> {
    if !parsed
        .function_annotations
        .iter()
        .any(|function| !function.default_parameters.is_empty())
    {
        return Ok(Vec::new());
    }
    let output = document.code();
    // Match bodies in syntax order, including anonymous callbacks. Names are
    // neither unique (nested scopes) nor present for anonymous functions.
    struct BodyOffsets(Vec<(usize, usize)>);
    impl Visitor for BodyOffsets {
        fn visit_function_body(&mut self, body: &ast::FunctionBody) {
            let (open, close) = body.parameters_parentheses().tokens();
            self.0.push((
                open.start_position().bytes(),
                close.start_position().bytes(),
            ));
        }
    }
    let output_ast = full_moon::parse(&output).map_err(|errors| {
        vec![Diagnostic::error(
            &parsed.path,
            1,
            1,
            format!("cannot locate function bodies for default lowering: {errors:?}"),
        )
        .with_code(DiagnosticCode::LOWERING_OPERATION)]
    })?;
    let mut bodies = BodyOffsets(Vec::new());
    bodies.visit_ast(&output_ast);
    bodies.0.sort_unstable();
    if bodies.0.len() != parsed.function_annotations.len() {
        return Err(vec![Diagnostic::error(
            &parsed.path,
            1,
            1,
            "function body provenance changed during default lowering",
        )
        .with_code(DiagnosticCode::LOWERING_OPERATION)]);
    }
    let mut operations = Vec::new();
    let mut outer_scope = None;
    for (function, (header_start, close)) in parsed.function_annotations.iter().zip(bodies.0) {
        if function.default_parameters.is_empty() {
            continue;
        }
        let trimmed_close = output[..close].trim_end_matches([' ', '\t']).len();

        if trimmed_close < close {
            operations.push(document.operation(
                LoweringKind::DefaultParameter,
                trimmed_close..close,
                "",
            ));
        }

        let after_header = &output[close + 1..];
        let whitespace = &after_header[..after_header.len() - after_header.trim_start().len()];
        let newline = whitespace.find('\n');
        let insert_at = newline.map_or(close + 1, |offset| close + 2 + offset);
        let body_indent = format!("{}  ", line_indent_before(&output, header_start));
        let mut first = true;

        for default in &function.default_parameters {
            let source_span = default
                .span
                .or(function.span)
                .unwrap_or_else(|| document.source_span_at(header_start));
            let guard = default_parameter_guard_text(std::slice::from_ref(default), &body_indent);
            let guard = lower_generated_default_helpers_with_ancestors(
                &guard,
                parsed,
                outer_scope.get_or_insert_with(|| crate::scope::scope_map_parsed_source(parsed)),
                source_span,
                builtin_constructor_spans,
                ancestors,
            )?;
            let guard = if first && newline.is_none() {
                format!("\n{guard}")
            } else {
                guard
            };
            first = false;
            operations.push(lowering::LoweringOperation::insert(
                insert_at,
                guard,
                source_span,
                LoweringKind::DefaultParameter,
            ));
        }
    }

    Ok(operations)
}

// Default expressions are absent from the normalized source AST. Parse each
// generated guard once, then reuse lexical helper lowering with inherited outer
// bindings. This also resolves callback-local shadows and nested helpers.
#[cfg(test)]
fn lower_generated_default_helpers(
    text: &str,
    parsed: &ParsedSource,
    outer_scope: &crate::scope::ScopeMap,
    span: SourceSpan,
    used: &mut BTreeMap<String, SourceSpan>,
) -> Result<String, Vec<Diagnostic>> {
    lower_generated_default_helpers_with_ancestors(text, parsed, outer_scope, span, used, &[])
}

fn lower_generated_default_helpers_with_ancestors(
    text: &str,
    parsed: &ParsedSource,
    outer_scope: &crate::scope::ScopeMap,
    span: SourceSpan,
    used: &mut BTreeMap<String, SourceSpan>,
    ancestors: &[(&crate::scope::ScopeMap, SourcePosition)],
) -> Result<String, Vec<Diagnostic>> {
    if ancestors.len() >= 64 {
        return Err(vec![Diagnostic::error(
            &parsed.path,
            span.start.line,
            span.start.column,
            "nested parameter defaults exceed lowering depth limit",
        )
        .with_code(DiagnosticCode::LOWERING_OPERATION)]);
    }
    let remap_diagnostics = |mut diagnostics: Vec<Diagnostic>| {
        let position = parsed
            .raw
            .parser_position_origin(span.start)
            .map(|origin| origin.source_position)
            .unwrap_or(span.start);
        for diagnostic in &mut diagnostics {
            diagnostic.line = position.line;
            diagnostic.column = position.column;
            diagnostic.end_line = position.line;
            diagnostic.end_column = position.column;
        }
        diagnostics
    };
    let guard = parse_tua_source(&parsed.path, text).map_err(remap_diagnostics)?;
    let mut scopes = vec![(outer_scope, span.start)];
    scopes.extend_from_slice(ancestors);
    let (ast, helpers) = builtin_calls::lower_with_outer_scopes(guard.ast.clone(), &guard, &scopes);
    for helper in helpers.into_keys() {
        used.entry(helper).or_insert(span);
    }
    let mut diagnostics = Vec::new();
    let ast = erase_ast(&parsed.path, ast, &mut diagnostics);
    if diagnostics.is_empty() {
        let mut document = lowering::LoweringDocument::from_erased_ast(&ast, &guard);
        let operations =
            lower_default_parameter_operations_with_ancestors(&document, &guard, used, &scopes)
                .map_err(remap_diagnostics)?;
        apply_lowering_operations(&mut document, operations, &guard).map_err(remap_diagnostics)?;
        Ok(document.code())
    } else {
        Err(remap_diagnostics(diagnostics))
    }
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
        return Some(
            "import specifier cannot traverse parent directories; use a project module name instead",
        );
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
