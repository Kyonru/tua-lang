use std::path::Path;

use crate::lua::{self, LuaItem, LuaItemKind};
use crate::module_index::required_module_export_for_fields;
use crate::semantic::{resolve_aliases, resolve_field_chain_type, SemanticModel};
use crate::source_text::{byte_for_column, is_identifier_continue, is_identifier_start};
use crate::types::{
    build_type_environment_with_diagnostics, FunctionType, Type, TypeEnvironment, TypedFunction,
    TypedFunctionOverload, TypedParameter,
};
use crate::{parse_tua_source, Diagnostic, ParsedSource, SourcePosition};

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
    let (environment, diagnostics) = build_type_environment_with_diagnostics(&parsed);

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(signature_help_parsed_source_with_context(
        &parsed,
        context,
        &environment,
    ))
}

pub fn signature_help_parsed_source(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Result<Option<SignatureHelp>, Vec<Diagnostic>> {
    let (environment, diagnostics) = build_type_environment_with_diagnostics(parsed);

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(signature_help_parsed_source_with_environment(
        parsed,
        position,
        &environment,
    ))
}

pub(crate) fn signature_help_parsed_source_with_environment(
    parsed: &ParsedSource,
    position: SourcePosition,
    environment: &TypeEnvironment,
) -> Option<SignatureHelp> {
    let context = call_context(&parsed.source, position)?;

    signature_help_parsed_source_with_context(parsed, context, environment)
}

fn signature_help_parsed_source_with_context(
    parsed: &ParsedSource,
    context: CallContext,
    environment: &TypeEnvironment,
) -> Option<SignatureHelp> {
    let resolver = SignatureResolver {
        parsed,
        environment,
        semantic: SemanticModel::new(parsed, environment),
    };
    let mut signatures = resolver.signatures_for_context(&context)?;
    if context.is_method_call {
        let receiver_type = resolver.method_receiver_type(&context);
        for signature in &mut signatures {
            strip_method_receiver(
                signature,
                &context,
                receiver_type.as_ref(),
                resolver.environment,
            );
        }
    }
    let active_signature =
        active_signature_for_argument_count(&signatures, context.active_parameter);
    let active_parameter =
        active_parameter_for_signature(context.active_parameter, &signatures[active_signature]);

    Some(SignatureHelp {
        signatures,
        active_signature,
        active_parameter,
    })
}

struct SignatureResolver<'a> {
    parsed: &'a ParsedSource,
    environment: &'a TypeEnvironment,
    semantic: SemanticModel<'a>,
}

impl<'a> SignatureResolver<'a> {
    fn signatures_for_context(&self, context: &CallContext) -> Option<Vec<SignatureInformation>> {
        self.signatures_for_parts(context)
    }

    fn method_receiver_type(&self, context: &CallContext) -> Option<Type> {
        if !context.is_method_call || context.callee_parts.len() < 2 {
            return None;
        }

        self.semantic.type_of_field_chain(
            &context.callee_parts[..context.callee_parts.len() - 1],
            context.position,
        )
    }

    fn signatures_for_parts(&self, context: &CallContext) -> Option<Vec<SignatureInformation>> {
        let parts = &context.callee_parts;
        let position = context.position;
        let (root, fields) = parts.split_first()?;
        let qualified_name = parts.join(".");

        if let Some(function) = self.environment.function(&qualified_name) {
            return Some(signatures_from_typed_function(
                function,
                Some(&qualified_name),
            ));
        }

        if fields.is_empty() {
            if let Some(ty) = self.environment.local(root) {
                return signature_from_type(root, ty, self.environment)
                    .map(|signature| vec![signature]);
            }

            if let Some(function) = self.environment.function(root) {
                return Some(signatures_from_typed_function(function, Some(root)));
            }
        }

        if !fields.is_empty() {
            if let Some(export) = required_module_export_for_fields(self.parsed, root, fields) {
                if let Some(mut signature) =
                    signature_from_type(&qualified_name, &export.ty, self.environment)
                {
                    if signature.documentation.is_none() {
                        signature.documentation = export.documentation;
                    }
                    apply_parameter_documentation(&mut signature, export.parameter_documentation);

                    return Some(vec![signature]);
                }
            }

            if let Some(item) = self
                .semantic
                .lua_function_item_for_call_parts(parts, position)
            {
                return signature_from_lua_item(item).map(|signature| vec![signature]);
            }

            if let Some(function) = self.semantic.typed_function_for_call_parts(parts, position) {
                return Some(signatures_from_typed_function(
                    &function,
                    Some(&qualified_name),
                ));
            }

            if let Some(ty) = resolve_field_chain_type(self.environment, parts) {
                if let Some(signature) = signature_from_type(&qualified_name, &ty, self.environment)
                {
                    return Some(vec![signature]);
                }
            }

            if self.environment.local(root).is_some() {
                return None;
            }
        }

        if let Some(signature) = lua_signature_for_parts(parts) {
            return Some(vec![signature]);
        }

        self.semantic
            .typed_function_for_call_parts(parts, position)
            .map(|function| signatures_from_typed_function(&function, Some(&qualified_name)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallContext {
    callee_parts: Vec<String>,
    active_parameter: usize,
    position: SourcePosition,
    is_method_call: bool,
}

fn call_context(source: &str, position: SourcePosition) -> Option<CallContext> {
    let cursor = byte_offset_for_position(source, position)?;
    let open = active_call_open(source, cursor)?;
    let (callee_parts, is_method_call) = callee_info_before(source, open)?;
    let active_parameter = active_parameter_index(source, open + 1, cursor);

    Some(CallContext {
        callee_parts,
        active_parameter,
        position,
        is_method_call,
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
        .find(|open| callee_info_before(source, *open).is_some())
}

fn callee_info_before(source: &str, open: usize) -> Option<(Vec<String>, bool)> {
    let bytes = source.as_bytes();
    let mut end = trim_ascii_whitespace_before(bytes, open);
    let mut parts = Vec::new();
    let mut is_method_call = false;

    loop {
        let start = reverse_identifier_start(bytes, end)?;

        parts.push(source[start..end].to_owned());
        end = trim_ascii_whitespace_before(bytes, start);

        if end == 0 || !matches!(bytes[end - 1], b'.' | b':') {
            break;
        }

        if parts.len() == 1 && bytes[end - 1] == b':' {
            is_method_call = true;
        }

        end = trim_ascii_whitespace_before(bytes, end - 1);
    }

    parts.reverse();
    (!parts.is_empty()).then_some((parts, is_method_call))
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
        .map(typed_parameter_label)
        .collect::<Vec<_>>();
    let mut parameters = function
        .parameters
        .iter()
        .map(|parameter| ParameterInformation {
            label: typed_parameter_label(parameter),
            documentation: parameter.documentation.clone(),
        })
        .collect::<Vec<_>>();

    if function.has_varargs {
        parameter_labels.push(typed_vararg_label(function.vararg_type.as_ref()));
        parameters.push(ParameterInformation {
            label: typed_vararg_label(function.vararg_type.as_ref()),
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

fn signatures_from_typed_function(
    function: &TypedFunction,
    name: Option<&str>,
) -> Vec<SignatureInformation> {
    let mut signatures = Vec::new();
    let name = name.or(function.name.as_deref()).unwrap_or("<anonymous>");

    if should_show_primary_typed_signature(function) {
        signatures.push(signature_from_typed_function(function, Some(name)));
    }

    signatures.extend(
        function
            .overloads
            .iter()
            .map(|overload| signature_from_typed_overload(name, overload)),
    );

    signatures
}

fn should_show_primary_typed_signature(function: &TypedFunction) -> bool {
    function.overloads.is_empty()
        || function
            .parameters
            .iter()
            .any(|parameter| parameter.ty != Type::Any)
        || function.vararg_type.is_some()
}

fn signature_from_typed_overload(
    name: &str,
    overload: &TypedFunctionOverload,
) -> SignatureInformation {
    let mut parameter_labels = overload
        .parameters
        .iter()
        .map(typed_parameter_label)
        .collect::<Vec<_>>();
    let mut parameters = overload
        .parameters
        .iter()
        .map(|parameter| ParameterInformation {
            label: typed_parameter_label(parameter),
            documentation: parameter.documentation.clone(),
        })
        .collect::<Vec<_>>();

    if overload.has_varargs {
        parameter_labels.push(typed_vararg_label(overload.vararg_type.as_ref()));
        parameters.push(ParameterInformation {
            label: typed_vararg_label(overload.vararg_type.as_ref()),
            documentation: None,
        });
    }

    signature_from_parameter_information(
        parameters,
        format!(
            "function {name}({}): {}",
            parameter_labels.join(", "),
            format_type_list(&overload.return_types_or_any())
        ),
        overload.documentation.clone(),
    )
}

fn typed_parameter_label(parameter: &TypedParameter) -> String {
    if parameter.name.is_empty() {
        parameter.ty.to_string()
    } else {
        format!("{}: {}", parameter.name, parameter.ty)
    }
}

fn typed_vararg_label(ty: Option<&Type>) -> String {
    ty.map(|ty| format!("...: {ty}"))
        .unwrap_or_else(|| "...".to_owned())
}

fn strip_method_receiver(
    signature: &mut SignatureInformation,
    context: &CallContext,
    receiver_type: Option<&Type>,
    environment: &TypeEnvironment,
) {
    let Some(first) = signature.parameters.first() else {
        return;
    };

    if !is_synthetic_method_receiver(&first.label, receiver_type, environment) {
        return;
    }

    signature.parameters.remove(0);

    let parameter_labels = signature
        .parameters
        .iter()
        .map(|parameter| parameter.label.clone())
        .collect::<Vec<_>>();
    let return_label = signature
        .label
        .rsplit_once("): ")
        .map(|(_, return_label)| return_label)
        .unwrap_or("any");

    signature.label = format!(
        "function {}({}): {}",
        method_display_name(&context.callee_parts),
        parameter_labels.join(", "),
        return_label
    );
}

fn is_synthetic_method_receiver(
    label: &str,
    receiver_type: Option<&Type>,
    environment: &TypeEnvironment,
) -> bool {
    label.starts_with("self:")
        || label == "any"
        || receiver_type.is_some_and(|ty| {
            label == ty.to_string()
                || environment
                    .to_type_context()
                    .is_assignable(ty, &Type::named(label))
        })
}

fn method_display_name(parts: &[String]) -> String {
    let Some((method, receiver_parts)) = parts.split_last() else {
        return "<anonymous>".to_owned();
    };

    if receiver_parts.is_empty() {
        method.clone()
    } else {
        format!("{}:{method}", receiver_parts.join("."))
    }
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
        Type::Any
        | Type::Alias(_)
        | Type::Primitive(_)
        | Type::Table(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => None,
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

fn active_signature_for_argument_count(
    signatures: &[SignatureInformation],
    active_parameter: usize,
) -> usize {
    signatures
        .iter()
        .position(|signature| active_parameter < signature.parameters.len())
        .unwrap_or(0)
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
