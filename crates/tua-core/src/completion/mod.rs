use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use full_moon::ast::{
    self, Expression, Field, FunctionCall, Index, Prefix, Suffix, TableConstructor, Var,
};
use full_moon::node::Node;

use self::context::{completion_context, CompletionContext};
use self::require::complete_require_paths;
use self::symbol::lua_symbols_before_position;
use crate::asset;
use crate::lua::{self, LuaItem, LuaItemKind};
use crate::module_index::{exported_field_for_fields, required_module_exports_for_receiver};
use crate::semantic::{resolve_aliases, resolve_field_chain_type, resolve_table_field_type};
use crate::source_text::line_at;
use crate::types::{
    build_type_environment_with_diagnostics, TableField, TableShape, Type, TypeEnvironment,
    TypedFunction,
};
use crate::{
    parse_tua_source, Diagnostic, FunctionAnnotation, ParsedSource, SourcePosition, TextEdit,
    TuaType,
};

mod context;
mod require;
mod symbol;

const MAX_COMPLETION_INFERENCE_DEPTH: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Completion {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: Option<String>,
    pub is_snippet: bool,
    pub module_path: Option<PathBuf>,
    pub source_module: Option<String>,
    pub additional_text_edits: Vec<TextEdit>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionKind {
    Snippet,
    Function,
    TypeAlias,
    Field,
    Keyword,
    Module,
    Variable,
    Value,
}

impl CompletionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Snippet => "snippet",
            Self::Function => "function",
            Self::TypeAlias => "typeAlias",
            Self::Field => "field",
            Self::Keyword => "keyword",
            Self::Module => "module",
            Self::Variable => "variable",
            Self::Value => "value",
        }
    }

    pub fn parse_kind(value: &str) -> Option<Self> {
        match value {
            "snippet" => Some(Self::Snippet),
            "function" => Some(Self::Function),
            "typeAlias" => Some(Self::TypeAlias),
            "field" => Some(Self::Field),
            "keyword" => Some(Self::Keyword),
            "module" => Some(Self::Module),
            "variable" => Some(Self::Variable),
            "value" => Some(Self::Value),
            _ => None,
        }
    }
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
        CompletionContext::AnnotationTag { prefix } => complete_annotation_tags(prefix.as_str()),
        CompletionContext::DocumentationSnippet { position } => {
            complete_documentation_snippet(parsed, position)
        }
        CompletionContext::TypeAnnotation { prefix } => {
            complete_type_names(parsed, prefix.as_str())
        }
        CompletionContext::FieldAccess {
            receiver_parts,
            prefix,
            position,
        } => {
            let environment = build_completion_environment(parsed);
            let fields = complete_fields(
                parsed,
                &environment,
                &receiver_parts,
                prefix.as_str(),
                position,
            );

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
        CompletionContext::AnnotationTag { prefix } => complete_annotation_tags(prefix.as_str()),
        CompletionContext::DocumentationSnippet { position } => {
            complete_lossy_documentation_snippet(source, position)
        }
        CompletionContext::TypeAnnotation { prefix } => complete_builtin_types(prefix.as_str()),
        CompletionContext::FieldAccess {
            receiver_parts,
            prefix,
            ..
        } => complete_lua_fields(&receiver_parts, prefix.as_str()),
        CompletionContext::RequirePath { prefix } => complete_require_paths(path, prefix.as_str()),
        CompletionContext::Identifier { prefix, position } => {
            complete_lossy_identifier_items(source, prefix.as_str(), position)
        }
    }
}

fn complete_annotation_tags(prefix: &str) -> Vec<Completion> {
    [
        ("alias", "type alias annotation"),
        ("class", "class annotation"),
        ("field", "field annotation"),
        ("generic", "generic annotation"),
        ("param", "function parameter annotation"),
        ("return", "function return annotation"),
        ("type", "type annotation"),
        ("version", "version annotation"),
    ]
    .into_iter()
    .filter(|(tag, _)| tag.starts_with(prefix))
    .map(|(tag, detail)| Completion {
        label: tag.to_owned(),
        kind: CompletionKind::Keyword,
        detail: Some(detail.to_owned()),
        documentation: None,
        insert_text: None,
        is_snippet: false,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    })
    .collect()
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
    [
        "Image", "Source", "any", "boolean", "nil", "number", "string",
    ]
    .into_iter()
    .filter(|primitive| primitive.starts_with(prefix))
    .map(|primitive| Completion {
        label: primitive.to_owned(),
        kind: CompletionKind::TypeAlias,
        detail: Some("built-in type".to_owned()),
        documentation: None,
        insert_text: None,
        is_snippet: false,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    })
    .collect()
}

fn complete_type_aliases(parsed: &ParsedSource, prefix: &str) -> Vec<Completion> {
    let (environment, _) = build_type_environment_with_diagnostics(parsed);

    environment
        .aliases()
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, _)| Completion {
            label: name.to_owned(),
            kind: CompletionKind::TypeAlias,
            detail: Some("type alias".to_owned()),
            documentation: environment.alias_documentation(name).map(str::to_owned),
            insert_text: None,
            is_snippet: false,
            module_path: None,
            source_module: None,
            additional_text_edits: Vec::new(),
        })
        .collect()
}

fn complete_documentation_snippet(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Vec<Completion> {
    let Some(function) = function_after_documentation_line(parsed, position) else {
        return Vec::new();
    };
    let Some(snippet) = documentation_snippet_for_function(parsed, function) else {
        return Vec::new();
    };

    vec![Completion {
        label: "document function".to_owned(),
        kind: CompletionKind::Snippet,
        detail: Some("LuaDoc function documentation".to_owned()),
        documentation: Some(
            "Generate LuaHelper-style parameter and return annotations.".to_owned(),
        ),
        insert_text: Some(snippet),
        is_snippet: true,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    }]
}

fn complete_lossy_documentation_snippet(source: &str, position: SourcePosition) -> Vec<Completion> {
    let Some(snippet) = lossy_documentation_snippet_after_line(source, position.line) else {
        return Vec::new();
    };

    vec![Completion {
        label: "document function".to_owned(),
        kind: CompletionKind::Snippet,
        detail: Some("LuaDoc function documentation".to_owned()),
        documentation: Some(
            "Generate LuaHelper-style parameter and return annotations.".to_owned(),
        ),
        insert_text: Some(snippet),
        is_snippet: true,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    }]
}

fn function_after_documentation_line(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Option<&FunctionAnnotation> {
    parsed
        .function_annotations
        .iter()
        .filter_map(|function| Some((function, function.span?.start.line)))
        .filter(|(_, line)| *line > position.line)
        .min_by_key(|(_, line)| *line)
        .and_then(|(function, line)| {
            doc_gap_is_clear(&parsed.source, position.line + 1, line).then_some(function)
        })
}

fn doc_gap_is_clear(source: &str, start_line: usize, function_line: usize) -> bool {
    (start_line..function_line).all(|line_number| {
        line_at(source, line_number)
            .map(|line| {
                let trimmed = line.trim();

                trimmed.is_empty() || trimmed.starts_with("---")
            })
            .unwrap_or(true)
    })
}

fn documentation_snippet_for_function(
    parsed: &ParsedSource,
    function: &FunctionAnnotation,
) -> Option<String> {
    let line = function
        .span
        .and_then(|span| line_at(&parsed.source, span.start.line))?;
    let mut signature = parse_function_signature_line(line)?;

    for parameter in &mut signature.parameters {
        if parameter.ty == "any" {
            if let Some(annotated) = function
                .parameters
                .iter()
                .find(|annotated| annotated.name == parameter.name)
            {
                parameter.ty = format_tua_type(&annotated.ty);
            }
        }
    }

    if signature.return_type.is_none() {
        signature.return_type = function.return_type.as_ref().map(format_tua_type);
    }

    documentation_snippet_from_signature(signature)
}

fn lossy_documentation_snippet_after_line(source: &str, line_number: usize) -> Option<String> {
    let function_line = ((line_number + 1)..)
        .map_while(|line| line_at(source, line).map(|content| (line, content)))
        .find(|(_, line)| !line.trim().is_empty() && !line.trim_start().starts_with("---"))?
        .1;

    documentation_snippet_from_signature(parse_function_signature_line(function_line)?)
}

fn documentation_snippet_from_signature(signature: FunctionSignature) -> Option<String> {
    if signature.parameters.is_empty() && signature.return_type.is_none() {
        return None;
    }

    let mut lines = Vec::new();
    let mut placeholder = 1;

    for parameter in signature.parameters {
        lines.push(format!(
            "@param {} {} ${{{}:description}}",
            parameter.name, parameter.ty, placeholder
        ));
        placeholder += 1;
    }

    if let Some(return_type) = signature.return_type {
        lines.push(format!(
            "@return {} ${{{}:description}}",
            return_type, placeholder
        ));
    }

    let (first, rest) = lines.split_first()?;
    let mut snippet = first.clone();

    for line in rest {
        snippet.push('\n');
        snippet.push_str("---");
        snippet.push_str(line);
    }

    Some(snippet)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FunctionSignature {
    parameters: Vec<DocParameter>,
    return_type: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DocParameter {
    name: String,
    ty: String,
}

fn parse_function_signature_line(line: &str) -> Option<FunctionSignature> {
    let open = line.find('(')?;
    let close = matching_close_paren(line, open)?;
    let parameters = line[open + 1..close]
        .split(',')
        .filter_map(parse_doc_parameter)
        .collect::<Vec<_>>();
    let return_type = line[close + 1..]
        .trim_start()
        .strip_prefix(':')
        .map(|ty| ty.trim())
        .filter(|ty| !ty.is_empty())
        .map(type_prefix);

    Some(FunctionSignature {
        parameters,
        return_type,
    })
}

fn matching_close_paren(line: &str, open: usize) -> Option<usize> {
    let mut depth = 0;

    for (index, character) in line[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;

                if depth == 0 {
                    return Some(open + index);
                }
            }
            _ => {}
        }
    }

    None
}

fn parse_doc_parameter(raw: &str) -> Option<DocParameter> {
    let raw = raw.trim();

    if raw.is_empty() {
        return None;
    }

    if raw == "..." {
        return Some(DocParameter {
            name: "...".to_owned(),
            ty: "any".to_owned(),
        });
    }

    let (name, ty) = raw
        .split_once(':')
        .map(|(name, ty)| (name.trim(), type_prefix(ty.trim())))
        .unwrap_or((raw, "any".to_owned()));

    if name.is_empty() {
        return None;
    }

    Some(DocParameter {
        name: name.to_owned(),
        ty,
    })
}

fn type_prefix(ty: &str) -> String {
    ty.split("--")
        .next()
        .map(str::trim)
        .filter(|ty| !ty.is_empty())
        .unwrap_or("any")
        .to_owned()
}

fn format_tua_type(ty: &TuaType) -> String {
    match ty {
        TuaType::Named(name) => name.clone(),
        TuaType::Union(types) => types
            .iter()
            .map(format_tua_type)
            .collect::<Vec<_>>()
            .join(" | "),
        TuaType::Table(fields) => {
            let fields = fields
                .iter()
                .map(|field| {
                    let marker = if field.optional { "?:" } else { ":" };

                    format!("{}{marker} {}", field.name, format_tua_type(&field.ty))
                })
                .collect::<Vec<_>>()
                .join(", ");

            format!("{{ {fields} }}")
        }
    }
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
                    documentation: None,
                    insert_text: None,
                    is_snippet: false,
                    module_path: None,
                    source_module: None,
                    additional_text_edits: Vec::new(),
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
                    documentation: None,
                    insert_text: None,
                    is_snippet: false,
                    module_path: None,
                    source_module: None,
                    additional_text_edits: Vec::new(),
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
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    receiver_parts: &[String],
    prefix: &str,
    position: SourcePosition,
) -> Vec<Completion> {
    let Some(receiver_type) = resolve_field_chain_type(environment, receiver_parts)
        .or_else(|| infer_field_chain_type(parsed, environment, receiver_parts, position))
    else {
        return Vec::new();
    };
    let mut completions = BTreeMap::new();
    let mut fields = BTreeMap::new();

    collect_table_fields(environment, &receiver_type, &mut fields);

    for (name, detail) in fields
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
    {
        completions.insert(
            name.clone(),
            Completion {
                label: name,
                kind: CompletionKind::Field,
                detail: Some(detail),
                documentation: None,
                insert_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            },
        );
    }

    collect_love_object_fields(environment, &receiver_type, prefix, &mut completions);

    completions.into_values().collect()
}

fn collect_love_object_fields(
    environment: &TypeEnvironment,
    ty: &Type,
    prefix: &str,
    completions: &mut BTreeMap<String, Completion>,
) {
    match resolve_aliases(environment, ty, 0) {
        Type::Alias(name) => {
            for item in lua::type_field_items(&name, prefix) {
                insert_lua_item(completions, item);
            }
        }
        Type::Union(types) => {
            for ty in types {
                collect_love_object_fields(environment, &ty, prefix, completions);
            }
        }
        Type::Any | Type::Function(_) | Type::Primitive(_) | Type::Table(_) => {}
    }
}

fn infer_field_chain_type(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    parts: &[String],
    position: SourcePosition,
) -> Option<Type> {
    let (root, fields) = parts.split_first()?;
    let mut current = environment
        .local(root)
        .cloned()
        .or_else(|| infer_local_assignment_type(parsed, environment, root, position, 0))?;

    for field in fields {
        current = resolve_table_field_type(environment, &current, field)?;
    }

    Some(current)
}

fn infer_local_assignment_type(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    name: &str,
    position: SourcePosition,
    depth: usize,
) -> Option<Type> {
    if depth >= MAX_COMPLETION_INFERENCE_DEPTH {
        return None;
    }

    infer_local_assignment_type_in_block(
        parsed,
        environment,
        parsed.ast.nodes(),
        name,
        position,
        depth,
    )
}

fn infer_local_assignment_type_in_block(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    block: &ast::Block,
    name: &str,
    position: SourcePosition,
    depth: usize,
) -> Option<Type> {
    for stmt in block.stmts() {
        if !starts_before(node_start(stmt), position) {
            continue;
        }

        match stmt {
            ast::Stmt::LocalAssignment(local) => {
                for (local_name, expression) in local.names().iter().zip(local.expressions()) {
                    if token_text(local_name) != name {
                        continue;
                    }

                    return infer_expression_type(
                        parsed,
                        environment,
                        expression,
                        position,
                        depth + 1,
                    );
                }
            }
            ast::Stmt::Do(block) => {
                if let Some(ty) = infer_local_assignment_type_in_block(
                    parsed,
                    environment,
                    block.block(),
                    name,
                    position,
                    depth + 1,
                ) {
                    return Some(ty);
                }
            }
            ast::Stmt::FunctionDeclaration(function) => {
                if let Some(ty) = infer_local_assignment_type_in_block(
                    parsed,
                    environment,
                    function.body().block(),
                    name,
                    position,
                    depth + 1,
                ) {
                    return Some(ty);
                }
            }
            ast::Stmt::GenericFor(for_stmt) => {
                if let Some(ty) = infer_local_assignment_type_in_block(
                    parsed,
                    environment,
                    for_stmt.block(),
                    name,
                    position,
                    depth + 1,
                ) {
                    return Some(ty);
                }
            }
            ast::Stmt::If(if_stmt) => {
                if let Some(ty) = infer_local_assignment_type_in_block(
                    parsed,
                    environment,
                    if_stmt.block(),
                    name,
                    position,
                    depth + 1,
                ) {
                    return Some(ty);
                }

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        if let Some(ty) = infer_local_assignment_type_in_block(
                            parsed,
                            environment,
                            else_if.block(),
                            name,
                            position,
                            depth + 1,
                        ) {
                            return Some(ty);
                        }
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    if let Some(ty) = infer_local_assignment_type_in_block(
                        parsed,
                        environment,
                        else_block,
                        name,
                        position,
                        depth + 1,
                    ) {
                        return Some(ty);
                    }
                }
            }
            ast::Stmt::LocalFunction(function) => {
                if let Some(ty) = infer_local_assignment_type_in_block(
                    parsed,
                    environment,
                    function.body().block(),
                    name,
                    position,
                    depth + 1,
                ) {
                    return Some(ty);
                }
            }
            ast::Stmt::NumericFor(for_stmt) => {
                if let Some(ty) = infer_local_assignment_type_in_block(
                    parsed,
                    environment,
                    for_stmt.block(),
                    name,
                    position,
                    depth + 1,
                ) {
                    return Some(ty);
                }
            }
            ast::Stmt::Repeat(repeat) => {
                if let Some(ty) = infer_local_assignment_type_in_block(
                    parsed,
                    environment,
                    repeat.block(),
                    name,
                    position,
                    depth + 1,
                ) {
                    return Some(ty);
                }
            }
            ast::Stmt::While(while_stmt) => {
                if let Some(ty) = infer_local_assignment_type_in_block(
                    parsed,
                    environment,
                    while_stmt.block(),
                    name,
                    position,
                    depth + 1,
                ) {
                    return Some(ty);
                }
            }
            _ => {}
        }
    }

    None
}

fn infer_expression_type(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    expression: &Expression,
    position: SourcePosition,
    depth: usize,
) -> Option<Type> {
    if depth >= MAX_COMPLETION_INFERENCE_DEPTH {
        return None;
    }

    match expression {
        Expression::Number(_) => Some(Type::number()),
        Expression::String(_) => Some(Type::string()),
        Expression::Symbol(symbol) => infer_symbol_type(&token_text(symbol)),
        Expression::TableConstructor(table) => Some(infer_table_constructor_type(
            parsed,
            environment,
            table,
            position,
            depth + 1,
        )),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => {
            infer_expression_type(parsed, environment, expression, position, depth + 1)
        }
        Expression::FunctionCall(function_call) => {
            infer_function_call_return_type(environment, function_call)
        }
        Expression::Var(var) => infer_var_type(parsed, environment, var, position, depth + 1),
        _ => None,
    }
}

fn infer_table_constructor_type(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    table: &TableConstructor,
    position: SourcePosition,
    depth: usize,
) -> Type {
    Type::table(table.fields().iter().filter_map(|field| {
        let Field::NameKey { key, value, .. } = field else {
            return None;
        };
        let ty = infer_expression_type(parsed, environment, value, position, depth + 1)
            .unwrap_or(Type::Any);

        Some((token_text(key), TableField::required(ty)))
    }))
}

fn infer_var_type(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    var: &Var,
    position: SourcePosition,
    depth: usize,
) -> Option<Type> {
    match var {
        Var::Name(name) => {
            let name = token_text(name);

            environment
                .local(&name)
                .cloned()
                .or_else(|| environment.function(&name).and_then(TypedFunction::as_type))
                .or_else(|| {
                    infer_local_assignment_type(parsed, environment, &name, position, depth + 1)
                })
        }
        Var::Expression(expression) => infer_field_chain_type(
            parsed,
            environment,
            &plain_field_chain(expression)?,
            position,
        ),
        _ => None,
    }
}

fn infer_function_call_return_type(
    environment: &TypeEnvironment,
    function_call: &FunctionCall,
) -> Option<Type> {
    let parts = field_chain_before_first_call(function_call)?;
    let (root, fields) = parts.split_first()?;

    if fields.is_empty() {
        if let Some(function) = environment.function(root) {
            return function.return_types_or_any().into_iter().next();
        }

        if let Some(kind) = asset::loader_kind(root) {
            return Some(kind.return_type());
        }
    }

    let callee_type = resolve_field_chain_type(environment, &parts)?;
    let Type::Function(function) = callee_type else {
        return None;
    };

    function.return_types_or_any().first().cloned()
}

fn plain_field_chain(expression: &ast::VarExpression) -> Option<Vec<String>> {
    let Prefix::Name(name) = expression.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in expression.suffixes() {
        let Suffix::Index(Index::Dot { name, .. }) = suffix else {
            return None;
        };

        parts.push(token_text(name));
    }

    Some(parts)
}

fn field_chain_before_first_call(function_call: &FunctionCall) -> Option<Vec<String>> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in function_call.suffixes() {
        match suffix {
            Suffix::Index(Index::Dot { name, .. }) => parts.push(token_text(name)),
            Suffix::Call(_) => return Some(parts),
            _ => return None,
        }
    }

    None
}

fn infer_symbol_type(symbol: &str) -> Option<Type> {
    match symbol {
        "true" | "false" => Some(Type::boolean()),
        "nil" => Some(Type::nil()),
        _ => None,
    }
}

fn node_start(node: &impl Node) -> SourcePosition {
    node.range()
        .map(|(start, _)| SourcePosition::from_full_moon(start))
        .unwrap_or(SourcePosition { line: 1, column: 1 })
}

fn starts_before(left: SourcePosition, right: SourcePosition) -> bool {
    (left.line, left.column) < (right.line, right.column)
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
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

    if fields.is_empty() {
        let module_path = exports.path.clone();

        return exports
            .fields
            .into_iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(name, field)| Completion {
                kind: completion_kind_for_type(&field.ty),
                detail: Some(format!("field: {}", field.ty)),
                label: name,
                documentation: field.documentation,
                insert_text: None,
                is_snippet: false,
                module_path: Some(module_path.clone()),
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect();
    }

    let Some(export) = exported_field_for_fields(&exports, fields) else {
        return Vec::new();
    };
    let module_path = exports.path;
    let mut fields = BTreeMap::new();

    collect_export_fields(&export.ty, &mut fields);

    fields
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, ty)| Completion {
            kind: completion_kind_for_type(&ty),
            detail: Some(format!("field: {ty}")),
            label: name,
            documentation: export.documentation.clone(),
            insert_text: None,
            is_snippet: false,
            module_path: Some(module_path.clone()),
            source_module: None,
            additional_text_edits: Vec::new(),
        })
        .collect()
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
        documentation: Some(item.documentation.to_owned()),
        insert_text: None,
        is_snippet: false,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
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
