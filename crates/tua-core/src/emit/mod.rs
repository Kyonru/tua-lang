use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

use full_moon::ast::span::ContainedSpan;
use full_moon::ast::{self, Expression, Field, LastStmt, Var};
use full_moon::tokenizer::TokenReference;

use crate::{
    asset, infer_module_exports, parse_tua_source, vector, AnnotationMode, Diagnostic,
    DiagnosticCode, ParsedSource, TuaType, Type,
};

pub fn emit_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<String, Vec<Diagnostic>> {
    emit_tua_source_with_mode(path, source, AnnotationMode::Off)
}

pub fn emit_tua_source_with_mode(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    annotation_mode: AnnotationMode,
) -> Result<String, Vec<Diagnostic>> {
    let parsed =
        parse_tua_source(path, source).map_err(reclassify_compound_assignment_diagnostics)?;

    emit_lua_with_mode(&parsed, annotation_mode)
}

pub fn emit_lua(parsed: &ParsedSource) -> Result<String, Vec<Diagnostic>> {
    emit_lua_with_mode(parsed, AnnotationMode::Off)
}

pub fn emit_lua_with_mode(
    parsed: &ParsedSource,
    annotation_mode: AnnotationMode,
) -> Result<String, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let ast = erase_ast(&parsed.path, parsed.ast.clone(), &mut diagnostics);

    if diagnostics.is_empty() {
        let output = normalize_emitted_lua(ast.to_string());
        let output = lower_builtin_calls_text(&output);
        let output = lower_compound_assignments_text(&output);

        Ok(match annotation_mode {
            AnnotationMode::Off => output,
            AnnotationMode::Luacats => apply_luacats_annotations(&output, parsed),
        })
    } else {
        Err(diagnostics)
    }
}

fn apply_luacats_annotations(runtime_output: &str, parsed: &ParsedSource) -> String {
    let mut aliases = Vec::new();
    let mut local_annotations = BTreeMap::<String, VecDeque<Vec<String>>>::new();
    let mut local_export_types = BTreeMap::<String, String>::new();
    let mut function_annotations = BTreeMap::<String, VecDeque<Vec<String>>>::new();
    let mut function_export_types = BTreeMap::<String, String>::new();

    for alias in &parsed.type_aliases {
        aliases.push(format!(
            "---@alias {} {}",
            alias.name,
            luacats_type(&alias.ty)
        ));
    }

    for local in &parsed.local_annotations {
        let type_label = luacats_type(&local.ty);
        local_annotations
            .entry(local.name.clone())
            .or_default()
            .push_back(vec![format!("---@type {type_label}")]);
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
                    .push_back(comments);
            }

            function_export_types
                .entry(name.to_owned())
                .or_insert_with(|| luacats_function_type(function));
        }
    }

    let export_annotations =
        module_export_annotations(parsed, &local_export_types, &function_export_types);
    let mut output = String::new();

    if !aliases.is_empty() {
        for alias in aliases {
            output.push_str(&alias);
            output.push('\n');
        }

        if !runtime_output.trim().is_empty() {
            output.push('\n');
        }
    }

    for line in output_lines(
        runtime_output,
        &mut local_annotations,
        &mut function_annotations,
        &export_annotations,
    ) {
        output.push_str(&line);
        output.push('\n');
    }

    output
}

fn output_lines(
    output: &str,
    local_annotations: &mut BTreeMap<String, VecDeque<Vec<String>>>,
    function_annotations: &mut BTreeMap<String, VecDeque<Vec<String>>>,
    export_annotations: &[String],
) -> Vec<String> {
    let lines = output.lines().collect::<Vec<_>>();
    let export_line = if export_annotations.is_empty() {
        None
    } else {
        lines
            .iter()
            .rposition(|line| line.trim_start().starts_with("return "))
    };

    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let mut line_output = String::new();
            let trimmed = line.trim_start();
            let indent = &line[..line.len() - trimmed.len()];

            if Some(index) == export_line {
                push_indented_comments(&mut line_output, indent, export_annotations);
            }

            for name in declared_local_names(trimmed) {
                if let Some(comments) = pop_comments(local_annotations, &name) {
                    push_indented_comments(&mut line_output, indent, &comments);
                }
            }

            if let Some(name) = declared_function_name(trimmed) {
                if let Some(comments) = pop_comments(function_annotations, &name) {
                    push_indented_comments(&mut line_output, indent, &comments);
                }
            }

            line_output.push_str(line);
            line_output
        })
        .collect()
}

fn push_indented_comments(output: &mut String, indent: &str, comments: &[String]) {
    for comment in comments {
        output.push_str(indent);
        output.push_str(comment);
        output.push('\n');
    }
}

fn pop_comments(
    annotations: &mut BTreeMap<String, VecDeque<Vec<String>>>,
    name: &str,
) -> Option<Vec<String>> {
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
                .map(|(index, ty)| format!("arg{}: {}", index + 1, luacats_type_from_type(ty)))
                .collect::<Vec<_>>()
                .join(", ");
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
                .map(|(index, ty)| format!("arg{}: {}", index + 1, luacats_type_from_type(ty)))
                .collect::<Vec<_>>()
                .join(", ");
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
    let nodes = erase_block(path, ast.nodes().clone(), diagnostics);

    ast.with_nodes(nodes)
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
    let Some(return_type) = body.return_type() else {
        return body.parameters_parentheses().clone();
    };

    let (open, close) = body.parameters_parentheses().tokens();
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

fn lower_compound_assignments_text(output: &str) -> String {
    let mut lowered = output
        .lines()
        .map(lower_compound_assignment_line)
        .collect::<Vec<_>>()
        .join("\n");

    if output.ends_with('\n') {
        lowered.push('\n');
    }

    lowered
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

fn lower_builtin_calls_text(output: &str) -> String {
    let mut lowered = String::with_capacity(output.len());
    let mut index = 0;

    while index < output.len() {
        let rest = &output[index..];

        if rest.starts_with("--") {
            let end = consume_comment(output, index);

            lowered.push_str(&output[index..end]);
            index = end;
            continue;
        }

        if let Some(end) = consume_string_literal(output, index) {
            lowered.push_str(&output[index..end]);
            index = end;
            continue;
        }

        if let Some(identifier_end) = consume_identifier(rest) {
            let name = &output[index..index + identifier_end];

            if can_start_asset_loader(output, index) {
                if let Some(kind) = asset::loader_kind(name) {
                    let argument_start = skip_whitespace(output, index + identifier_end);

                    if let Some((replacement, end)) =
                        lower_asset_loader_call(output, argument_start, kind)
                    {
                        lowered.push_str(&replacement);
                        index = end;
                        continue;
                    }
                }
            }

            if can_start_builtin_call(output, index) && vector::is_constructor(name) {
                let argument_start = skip_whitespace(output, index + identifier_end);

                if let Some((replacement, end)) =
                    lower_vec2_constructor_call(output, argument_start)
                {
                    lowered.push_str(&replacement);
                    index = end;
                    continue;
                }
            }

            lowered.push_str(name);
            index += identifier_end;
            continue;
        }

        let character = rest
            .chars()
            .next()
            .expect("loop only runs at valid string boundaries");

        lowered.push(character);
        index += character.len_utf8();
    }

    lowered
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

    Some((format!("{{ x = {x}, y = {y} }}"), close + 1))
}

fn lower_asset_loader_call(
    output: &str,
    argument_start: usize,
    kind: asset::AssetLoaderKind,
) -> Option<(String, usize)> {
    if let Some(path_end) = consume_string_literal(output, argument_start) {
        let path_literal = &output[argument_start..path_end];

        return Some((kind.emitted_call(path_literal), path_end));
    }

    if !output[argument_start..].starts_with('(') {
        return None;
    }

    let path_start = skip_whitespace(output, argument_start + 1);
    let path_end = consume_string_literal(output, path_start)?;
    let close = skip_whitespace(output, path_end);

    if !output[close..].starts_with(')') {
        return None;
    }

    let path_literal = &output[path_start..path_end];

    Some((kind.emitted_call(path_literal), close + 1))
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

fn normalize_emitted_lua(output: String) -> String {
    let trimmed = output.trim_matches(char::is_whitespace);

    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

#[cfg(test)]
mod tests;
