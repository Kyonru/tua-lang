use std::path::Path;

use full_moon::ast::{
    self, Expression, Field, FunctionCall, Index, Prefix, Suffix, TableConstructor, Var,
};
use full_moon::node::Node;

use crate::asset;
use crate::lua::{self, LuaItem, LuaItemKind};
use crate::module_index::required_module_export_for_fields;
use crate::semantic::{resolve_aliases, resolve_field_chain_type, resolve_table_field_type};
use crate::source_text::{byte_for_column, is_identifier_continue, is_identifier_start};
use crate::types::{
    build_type_environment_with_diagnostics, FunctionType, TableField, Type, TypeEnvironment,
    TypedFunction,
};
use crate::{parse_tua_source, Diagnostic, ParsedSource, SourcePosition};

const MAX_SIGNATURE_INFERENCE_DEPTH: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureHelp {
    pub signatures: Vec<SignatureInformation>,
    pub active_signature: usize,
    pub active_parameter: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureInformation {
    pub label: String,
    pub parameters: Vec<ParameterInformation>,
    pub documentation: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterInformation {
    pub label: String,
    pub documentation: Option<String>,
}

pub fn signature_help_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    position: SourcePosition,
) -> Result<Option<SignatureHelp>, Vec<Diagnostic>> {
    let path = path.as_ref().to_path_buf();
    let source = source.into();
    let Some(context) = call_context(&source, position) else {
        return Ok(None);
    };

    let parsed = match parse_signature_help_source(&path, &source, position) {
        Ok(parsed) => parsed,
        Err(diagnostics) => {
            return lua_signature_help(&context).map(Some).ok_or(diagnostics);
        }
    };

    Ok(signature_help_parsed_source_with_context(&parsed, context))
}

pub fn signature_help_parsed_source(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Result<Option<SignatureHelp>, Vec<Diagnostic>> {
    let Some(context) = call_context(&parsed.source, position) else {
        return Ok(None);
    };

    Ok(signature_help_parsed_source_with_context(parsed, context))
}

fn signature_help_parsed_source_with_context(
    parsed: &ParsedSource,
    context: CallContext,
) -> Option<SignatureHelp> {
    let (environment, _) = build_type_environment_with_diagnostics(parsed);
    let resolver = SignatureResolver {
        parsed,
        environment: &environment,
    };
    let signature = resolver.signature_for_context(&context)?;
    let active_parameter = active_parameter_for_signature(context.active_parameter, &signature);

    Some(SignatureHelp {
        signatures: vec![signature],
        active_signature: 0,
        active_parameter,
    })
}

struct SignatureResolver<'a> {
    parsed: &'a ParsedSource,
    environment: &'a TypeEnvironment,
}

impl<'a> SignatureResolver<'a> {
    fn signature_for_context(&self, context: &CallContext) -> Option<SignatureInformation> {
        self.signature_for_parts(&context.callee_parts, context.position)
    }

    fn signature_for_parts(
        &self,
        parts: &[String],
        position: SourcePosition,
    ) -> Option<SignatureInformation> {
        let (root, fields) = parts.split_first()?;
        let qualified_name = parts.join(".");

        if let Some(function) = self.environment.function(&qualified_name) {
            return Some(signature_from_typed_function(
                function,
                Some(&qualified_name),
            ));
        }

        if fields.is_empty() {
            if let Some(ty) = self.environment.local(root) {
                return signature_from_type(root, ty, self.environment);
            }

            if let Some(function) = self.environment.function(root) {
                return Some(signature_from_typed_function(function, Some(root)));
            }
        } else if let Some(ty) = resolve_field_chain_type(self.environment, parts) {
            if let Some(signature) = signature_from_type(&qualified_name, &ty, self.environment) {
                return Some(signature);
            }
        }

        if !fields.is_empty() {
            if let Some(signature) = self.love_object_method_signature(parts, position) {
                return Some(signature);
            }

            if let Some(export) = required_module_export_for_fields(self.parsed, root, fields) {
                if let Some(mut signature) =
                    signature_from_type(&qualified_name, &export.ty, self.environment)
                {
                    if signature.documentation.is_none() {
                        signature.documentation = export.documentation;
                    }
                    apply_parameter_documentation(&mut signature, export.parameter_documentation);

                    return Some(signature);
                }
            }

            if self.environment.local(root).is_some() {
                return None;
            }
        }

        lua_signature_for_parts(parts)
    }

    fn love_object_method_signature(
        &self,
        parts: &[String],
        position: SourcePosition,
    ) -> Option<SignatureInformation> {
        let method = parts.last()?;
        let receiver_parts = parts.get(..parts.len().checked_sub(1)?)?;
        let receiver_type = self.infer_receiver_type(receiver_parts, position)?;
        let item = lua_type_field_item(self.environment, &receiver_type, method)?;

        (item.kind == LuaItemKind::Function)
            .then(|| signature_from_lua_item(item))
            .flatten()
    }

    fn infer_receiver_type(&self, parts: &[String], position: SourcePosition) -> Option<Type> {
        let (root, fields) = parts.split_first()?;
        let mut current = self
            .environment
            .local(root)
            .cloned()
            .or_else(|| self.infer_named_value(root, position, 0))?;

        for field in fields {
            current = resolve_table_field_type(self.environment, &current, field)?;
        }

        Some(current)
    }

    fn infer_named_value(
        &self,
        name: &str,
        position: SourcePosition,
        depth: usize,
    ) -> Option<Type> {
        if depth >= MAX_SIGNATURE_INFERENCE_DEPTH {
            return None;
        }

        self.environment
            .local(name)
            .cloned()
            .or_else(|| {
                self.environment
                    .function(name)
                    .and_then(TypedFunction::as_type)
            })
            .or_else(|| {
                let local = self.local_assignment_value(name, position)?;

                local.and_then(|expression| self.infer_expression(expression, position, depth + 1))
            })
    }

    fn local_assignment_value(
        &self,
        name: &str,
        position: SourcePosition,
    ) -> Option<Option<&'a Expression>> {
        let mut candidates = Vec::new();

        for stmt in self.parsed.ast.nodes().stmts() {
            let ast::Stmt::LocalAssignment(local) = stmt else {
                continue;
            };
            let expressions = local.expressions().iter().collect::<Vec<_>>();

            for (index, local_name) in local.names().iter().enumerate() {
                let Some(span) = node_span(local_name) else {
                    continue;
                };

                if token_text(local_name) != name || !starts_before_or_at(span.start, position) {
                    continue;
                }

                candidates.push((span.start, expressions.get(index).copied()));
            }
        }

        candidates
            .into_iter()
            .max_by_key(|(position, _)| position_key(*position))
            .map(|(_, expression)| expression)
    }

    fn infer_expression(
        &self,
        expression: &Expression,
        position: SourcePosition,
        depth: usize,
    ) -> Option<Type> {
        if depth >= MAX_SIGNATURE_INFERENCE_DEPTH {
            return None;
        }

        match expression {
            Expression::Number(_) => Some(Type::number()),
            Expression::String(_) => Some(Type::string()),
            Expression::Symbol(symbol) => infer_symbol_type(&token_text(symbol)),
            Expression::TableConstructor(table) => {
                Some(self.infer_table_constructor(table, position, depth + 1))
            }
            Expression::FunctionCall(function_call) => {
                self.infer_function_call_return_type(function_call)
            }
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. } => {
                self.infer_expression(expression, position, depth + 1)
            }
            Expression::Var(var) => self.infer_var_value(var, position, depth + 1),
            _ => None,
        }
    }

    fn infer_table_constructor(
        &self,
        table: &TableConstructor,
        position: SourcePosition,
        depth: usize,
    ) -> Type {
        Type::table(table.fields().iter().filter_map(|field| {
            let Field::NameKey { key, value, .. } = field else {
                return None;
            };
            let ty = self
                .infer_expression(value, position, depth + 1)
                .unwrap_or(Type::Any);

            Some((token_text(key), TableField::required(ty)))
        }))
    }

    fn infer_function_call_return_type(&self, function_call: &FunctionCall) -> Option<Type> {
        let parts = field_chain_before_first_call(function_call)?;
        let (root, fields) = parts.split_first()?;

        if fields.is_empty() {
            if let Some(function) = self.environment.function(root) {
                return function.return_types_or_any().into_iter().next();
            }

            if let Some(kind) = asset::loader_kind(root) {
                return Some(kind.return_type());
            }
        }

        let Type::Function(function) = resolve_field_chain_type(self.environment, &parts)? else {
            return None;
        };

        function.return_types_or_any().first().cloned()
    }

    fn infer_var_value(&self, var: &Var, position: SourcePosition, depth: usize) -> Option<Type> {
        match var {
            Var::Name(name) => self.infer_named_value(&token_text(name), position, depth + 1),
            Var::Expression(expression) => {
                self.infer_field_chain_type(&plain_field_chain(expression)?, position)
            }
            _ => None,
        }
    }

    fn infer_field_chain_type(&self, parts: &[String], position: SourcePosition) -> Option<Type> {
        let (root, fields) = parts.split_first()?;
        let mut current = self.infer_named_value(root, position, 0)?;

        for field in fields {
            current = resolve_table_field_type(self.environment, &current, field)?;
        }

        Some(current)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallContext {
    callee_parts: Vec<String>,
    active_parameter: usize,
    position: SourcePosition,
}

fn call_context(source: &str, position: SourcePosition) -> Option<CallContext> {
    let cursor = byte_offset_for_position(source, position)?;
    let open = active_call_open(source, cursor)?;
    let callee_parts = callee_parts_before(source, open)?;
    let active_parameter = active_parameter_index(source, open + 1, cursor);

    Some(CallContext {
        callee_parts,
        active_parameter,
        position,
    })
}

fn active_call_open(source: &str, cursor: usize) -> Option<usize> {
    let mut stack = Vec::new();
    let mut scanner = Scanner::new(source, cursor);

    while let Some((index, byte)) = scanner.next_code_byte() {
        match byte {
            b'(' => stack.push(index),
            b')' => {
                stack.pop();
            }
            _ => {}
        }
    }

    stack
        .into_iter()
        .rev()
        .find(|open| callee_parts_before(source, *open).is_some())
}

fn callee_parts_before(source: &str, open: usize) -> Option<Vec<String>> {
    let bytes = source.as_bytes();
    let mut end = trim_ascii_whitespace_before(bytes, open);
    let mut parts = Vec::new();

    loop {
        let start = reverse_identifier_start(bytes, end)?;

        parts.push(source[start..end].to_owned());
        end = trim_ascii_whitespace_before(bytes, start);

        if end == 0 || !matches!(bytes[end - 1], b'.' | b':') {
            break;
        }

        end = trim_ascii_whitespace_before(bytes, end - 1);
    }

    parts.reverse();
    (!parts.is_empty()).then_some(parts)
}

fn active_parameter_index(source: &str, start: usize, cursor: usize) -> usize {
    let mut active = 0;
    let mut depth = 0usize;
    let mut scanner = Scanner::new(
        &source[start.min(source.len())..],
        cursor.saturating_sub(start),
    );

    while let Some((_, byte)) = scanner.next_code_byte() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => active += 1,
            _ => {}
        }
    }

    active
}

struct Scanner<'a> {
    source: &'a str,
    cursor: usize,
    index: usize,
    quote: Option<u8>,
    escaped: bool,
    line_comment: bool,
}

impl<'a> Scanner<'a> {
    fn new(source: &'a str, cursor: usize) -> Self {
        Self {
            source,
            cursor: cursor.min(source.len()),
            index: 0,
            quote: None,
            escaped: false,
            line_comment: false,
        }
    }

    fn next_code_byte(&mut self) -> Option<(usize, u8)> {
        let bytes = self.source.as_bytes();

        while self.index < self.cursor {
            let index = self.index;
            let byte = bytes[index];

            self.index += 1;

            if self.line_comment {
                if byte == b'\n' {
                    self.line_comment = false;
                }
                continue;
            }

            if let Some(quote) = self.quote {
                if self.escaped {
                    self.escaped = false;
                } else if byte == b'\\' {
                    self.escaped = true;
                } else if byte == quote {
                    self.quote = None;
                }
                continue;
            }

            if byte == b'-' && self.index < self.cursor && bytes[self.index] == b'-' {
                self.index += 1;
                self.line_comment = true;
                continue;
            }

            if byte == b'"' || byte == b'\'' {
                self.quote = Some(byte);
                continue;
            }

            return Some((index, byte));
        }

        None
    }
}

fn signature_from_typed_function(
    function: &TypedFunction,
    name: Option<&str>,
) -> SignatureInformation {
    let name = name.or(function.name.as_deref()).unwrap_or("<anonymous>");
    let mut parameter_labels = function
        .parameters
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name, parameter.ty))
        .collect::<Vec<_>>();
    let mut parameters = function
        .parameters
        .iter()
        .map(|parameter| ParameterInformation {
            label: format!("{}: {}", parameter.name, parameter.ty),
            documentation: parameter.documentation.clone(),
        })
        .collect::<Vec<_>>();

    if function.has_varargs {
        parameter_labels.push("...".to_owned());
        parameters.push(ParameterInformation {
            label: "...".to_owned(),
            documentation: None,
        });
    }

    signature_from_parameter_information(
        parameters,
        format!(
            "function {name}({}): {}",
            parameter_labels.join(", "),
            format_type_list(&function.return_types_or_any())
        ),
        function.documentation.clone(),
    )
}

fn signature_from_type(
    name: &str,
    ty: &Type,
    environment: &TypeEnvironment,
) -> Option<SignatureInformation> {
    match resolve_aliases(environment, ty, 0) {
        Type::Function(function) => Some(signature_from_function_type(name, &function)),
        Type::Union(types) => types
            .iter()
            .find_map(|ty| signature_from_type(name, ty, environment)),
        Type::Any | Type::Alias(_) | Type::Primitive(_) | Type::Table(_) => None,
    }
}

fn signature_from_function_type(name: &str, function: &FunctionType) -> SignatureInformation {
    let parameters = function
        .parameters
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    signature_from_parts(
        format!(
            "function {name}({}): {}",
            parameters.join(", "),
            format_type_list(function.return_types_or_any())
        ),
        parameters,
        None,
    )
}

fn format_type_list(types: &[Type]) -> String {
    types
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn lua_signature_help(context: &CallContext) -> Option<SignatureHelp> {
    let signature = lua_signature_for_parts(&context.callee_parts)?;
    let active_parameter = active_parameter_for_signature(context.active_parameter, &signature);

    Some(SignatureHelp {
        signatures: vec![signature],
        active_signature: 0,
        active_parameter,
    })
}

fn lua_signature_for_parts(parts: &[String]) -> Option<SignatureInformation> {
    let item = lua::hover_item(parts)?;

    if item.kind != LuaItemKind::Function {
        return None;
    }

    signature_from_lua_item(item)
}

fn lua_type_field_item(environment: &TypeEnvironment, ty: &Type, field: &str) -> Option<LuaItem> {
    match resolve_aliases(environment, ty, 0) {
        Type::Alias(name) => lua::type_field_items(&name, field)
            .into_iter()
            .find(|item| item.label == field),
        Type::Union(types) => types
            .iter()
            .find_map(|ty| lua_type_field_item(environment, ty, field)),
        Type::Any | Type::Function(_) | Type::Primitive(_) | Type::Table(_) => None,
    }
}

fn signature_from_lua_item(item: LuaItem) -> Option<SignatureInformation> {
    let parameters = parameters_from_function_label(item.detail)?;

    Some(signature_from_parts(
        item.detail.to_owned(),
        parameters,
        Some(item.documentation.to_owned()),
    ))
}

fn parameters_from_function_label(label: &str) -> Option<Vec<String>> {
    let start = label.find('(')? + 1;
    let end = label[start..].find(')')? + start;
    let parameters = label[start..end].trim();

    if parameters.is_empty() {
        return Some(Vec::new());
    }

    Some(
        parameters
            .split(',')
            .map(str::trim)
            .filter(|parameter| !parameter.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

fn signature_from_parts(
    label: String,
    parameters: Vec<String>,
    documentation: Option<String>,
) -> SignatureInformation {
    signature_from_parameter_information(
        parameters
            .into_iter()
            .map(|label| ParameterInformation {
                label,
                documentation: None,
            })
            .collect(),
        label,
        documentation,
    )
}

fn signature_from_parameter_information(
    parameters: Vec<ParameterInformation>,
    label: String,
    documentation: Option<String>,
) -> SignatureInformation {
    SignatureInformation {
        label,
        parameters,
        documentation,
    }
}

fn apply_parameter_documentation(
    signature: &mut SignatureInformation,
    documentation: Vec<Option<String>>,
) {
    for (parameter, documentation) in signature.parameters.iter_mut().zip(documentation) {
        if parameter.documentation.is_none() {
            parameter.documentation = documentation;
        }
    }
}

fn active_parameter_for_signature(active: usize, signature: &SignatureInformation) -> usize {
    signature
        .parameters
        .len()
        .checked_sub(1)
        .map(|last| active.min(last))
        .unwrap_or(0)
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

fn node_span(node: &impl Node) -> Option<crate::SourceSpan> {
    node.range()
        .map(|(start, end)| crate::SourceSpan::from_range(start, end))
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn starts_before_or_at(left: SourcePosition, right: SourcePosition) -> bool {
    position_key(left) <= position_key(right)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn parse_signature_help_source(
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

fn byte_offset_for_position(source: &str, position: SourcePosition) -> Option<usize> {
    let mut offset = 0;

    for (line_index, segment) in source.split_inclusive('\n').enumerate() {
        let line_number = line_index + 1;
        let line = segment
            .strip_suffix('\n')
            .unwrap_or(segment)
            .strip_suffix('\r')
            .unwrap_or_else(|| segment.strip_suffix('\n').unwrap_or(segment));

        if line_number == position.line {
            return Some(offset + byte_for_column(line, position.column).min(line.len()));
        }

        offset += segment.len();
    }

    if position.line == source.split('\n').count() {
        let line = source
            .rsplit_once('\n')
            .map(|(_, line)| line)
            .unwrap_or(source);

        return Some(offset + byte_for_column(line, position.column).min(line.len()));
    }

    None
}

fn trim_ascii_whitespace_before(bytes: &[u8], mut end: usize) -> usize {
    end = end.min(bytes.len());

    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }

    end
}

fn reverse_identifier_start(bytes: &[u8], end: usize) -> Option<usize> {
    if end == 0 || !is_identifier_continue(*bytes.get(end - 1)?) {
        return None;
    }

    let mut start = end - 1;

    while start > 0 && is_identifier_continue(bytes[start - 1]) {
        start -= 1;
    }

    is_identifier_start(bytes[start]).then_some(start)
}

#[cfg(test)]
mod tests;
