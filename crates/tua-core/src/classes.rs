use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::Path;

use crate::parser::{parse_tua_type_text, SourcePosition, SourceSpan, TuaType};
use crate::{Diagnostic, DiagnosticCode};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClassDeclaration {
    pub name: String,
    pub parent: Option<String>,
    pub parent_span: Option<SourceSpan>,
    pub is_open: bool,
    pub is_final: bool,
    pub fields: Vec<ClassField>,
    pub init: Option<ClassMethod>,
    pub duplicate_init_spans: Vec<SourceSpan>,
    pub methods: Vec<ClassMethod>,
    pub span: SourceSpan,
    pub name_span: SourceSpan,
    pub type_id: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClassField {
    pub name: String,
    pub ty: String,
    pub parsed_ty: TuaType,
    pub default: Option<String>,
    pub span: SourceSpan,
    pub name_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClassMethod {
    pub name: String,
    pub parameters: String,
    pub return_type: Option<String>,
    pub body: String,
    pub is_final: bool,
    pub is_override: bool,
    pub span: SourceSpan,
    pub name_span: SourceSpan,
    pub parameters_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClassBlock {
    declaration: ClassDeclaration,
    start: usize,
    end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClassLoweringEdit {
    pub range: Range<usize>,
    pub replacement: String,
    pub source_span: SourceSpan,
}

#[derive(Clone, Debug)]
struct ClassHeader {
    name: String,
    parent: Option<String>,
    parent_span: Option<SourceSpan>,
    is_open: bool,
    is_final: bool,
    name_span: SourceSpan,
    span_start: SourcePosition,
    body_start: usize,
    body_open: usize,
}

#[derive(Clone, Debug)]
struct FlattenedField {
    name: String,
    value: String,
}

type ClassMethodParameterSignature = (TuaType, bool, bool);
type ClassMethodSignature = (Vec<ClassMethodParameterSignature>, Vec<TuaType>);

pub(crate) fn parse_class_declarations(
    path: &Path,
    source: &str,
) -> (Vec<ClassDeclaration>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let mut declarations = class_blocks(source)
        .into_iter()
        .map(|block| block.declaration)
        .collect::<Vec<_>>();

    validate_classes(path, &declarations, &mut diagnostics);

    for (index, declaration) in declarations.iter_mut().enumerate() {
        declaration.type_id = index + 1;
    }

    (declarations, diagnostics)
}

pub(crate) fn class_lowering_edits(source: &str) -> Vec<ClassLoweringEdit> {
    let blocks = class_blocks(source);

    if blocks.is_empty() {
        return Vec::new();
    }

    let mut declarations = blocks
        .iter()
        .map(|block| block.declaration.clone())
        .collect::<Vec<_>>();

    for (index, declaration) in declarations.iter_mut().enumerate() {
        declaration.type_id = index + 1;
    }
    blocks
        .into_iter()
        .zip(declarations.iter())
        .map(|(block, declaration)| ClassLoweringEdit {
            range: block.start..block.end,
            replacement: lower_class_declaration(declaration, &declarations),
            source_span: declaration.span,
        })
        .collect()
}

pub(crate) fn class_by_name<'a>(
    classes: &'a [ClassDeclaration],
    name: &str,
) -> Option<&'a ClassDeclaration> {
    classes.iter().find(|class| class.name == name)
}

pub(crate) fn class_method_is_final(
    classes: &[ClassDeclaration],
    class_name: &str,
    method: &str,
) -> bool {
    let Some(class) = class_by_name(classes, class_name) else {
        return false;
    };

    class.is_final
        || class_method_declaration(classes, class_name, method)
            .is_some_and(|(_, method)| method.is_final)
}

pub(crate) fn class_method_exists(
    classes: &[ClassDeclaration],
    class_name: &str,
    method: &str,
) -> bool {
    class_method_declaration(classes, class_name, method).is_some()
}

pub(crate) fn class_method_owner<'a>(
    classes: &'a [ClassDeclaration],
    class_name: &str,
    method: &str,
) -> Option<&'a str> {
    class_method_declaration(classes, class_name, method).map(|(class, _)| class.name.as_str())
}

fn class_method_declaration<'a>(
    classes: &'a [ClassDeclaration],
    class_name: &str,
    method: &str,
) -> Option<(&'a ClassDeclaration, &'a ClassMethod)> {
    class_method_declaration_with_visited(classes, class_name, method, &mut BTreeSet::new())
}

fn class_method_declaration_with_visited<'a>(
    classes: &'a [ClassDeclaration],
    class_name: &str,
    method: &str,
    visited: &mut BTreeSet<String>,
) -> Option<(&'a ClassDeclaration, &'a ClassMethod)> {
    if !visited.insert(class_name.to_owned()) {
        return None;
    }
    let class = class_by_name(classes, class_name)?;

    class
        .methods
        .iter()
        .find(|candidate| candidate.name == method)
        .map(|method| (class, method))
        .or_else(|| {
            class.parent.as_deref().and_then(|parent| {
                class_method_declaration_with_visited(classes, parent, method, visited)
            })
        })
}

fn class_blocks(source: &str) -> Vec<ClassBlock> {
    let mut blocks = Vec::new();
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

        let Some(header) = parse_class_header(source, index) else {
            index += next_char_len(source, index);
            continue;
        };
        let Some(body_end) = find_matching_brace(source, header.body_open) else {
            index += next_char_len(source, index);
            continue;
        };

        let body = &source[header.body_start..body_end];
        let declaration = parse_class_body(source, body, header.clone(), body_end);

        blocks.push(ClassBlock {
            declaration,
            start: index,
            end: body_end + 1,
        });

        index = body_end + 1;
    }

    blocks
}

fn parse_class_header(source: &str, index: usize) -> Option<ClassHeader> {
    if !can_start_keyword(source, index) {
        return None;
    }

    let span_start = source_position_from_byte(source, index);
    let mut cursor = index;
    let mut is_open = false;
    let mut is_final = false;

    if starts_keyword(source, cursor, "open") {
        is_open = true;
        cursor = skip_whitespace(source, cursor + "open".len());
    } else if starts_keyword(source, cursor, "final") {
        is_final = true;
        cursor = skip_whitespace(source, cursor + "final".len());
    }

    if !starts_keyword(source, cursor, "class") {
        return None;
    }

    cursor = skip_whitespace(source, cursor + "class".len());
    let (name, name_start, name_end) = take_identifier(source, cursor)?;
    cursor = skip_whitespace(source, name_end);

    let (parent, parent_span) = if starts_keyword(source, cursor, "extends") {
        cursor = skip_whitespace(source, cursor + "extends".len());
        let (parent, parent_start, parent_end) = take_identifier(source, cursor)?;
        cursor = skip_whitespace(source, parent_end);
        (
            Some(parent),
            Some(SourceSpan {
                start: source_position_from_byte(source, parent_start),
                end: source_position_from_byte(source, parent_end),
            }),
        )
    } else {
        (None, None)
    };

    if !source[cursor..].starts_with('{') {
        return None;
    }

    Some(ClassHeader {
        name,
        parent,
        parent_span,
        is_open,
        is_final: is_final || !is_open,
        name_span: SourceSpan {
            start: source_position_from_byte(source, name_start),
            end: source_position_from_byte(source, name_end),
        },
        span_start,
        body_start: cursor + 1,
        body_open: cursor,
    })
}

fn parse_class_body(
    source: &str,
    body: &str,
    header: ClassHeader,
    body_end: usize,
) -> ClassDeclaration {
    let mut fields = Vec::new();
    let mut init = None;
    let mut duplicate_init_spans = Vec::new();
    let mut methods = Vec::new();
    let mut cursor = 0usize;

    while cursor < body.len() {
        cursor = skip_body_ws_and_comments(body, cursor);

        if cursor >= body.len() {
            break;
        }

        if let Some((method, next)) = parse_class_method(source, body, header.body_start, cursor) {
            if method.name == "init" {
                if init.is_some() {
                    duplicate_init_spans.push(method.name_span);
                } else {
                    init = Some(method);
                }
            } else {
                methods.push(method);
            }
            cursor = next;
            continue;
        }

        if let Some((field, next)) = parse_class_field(source, body, header.body_start, cursor) {
            fields.push(field);
            cursor = next;
            continue;
        }

        cursor += next_char_len(body, cursor);
    }

    ClassDeclaration {
        name: header.name,
        parent: header.parent,
        parent_span: header.parent_span,
        is_open: header.is_open,
        is_final: header.is_final,
        fields,
        init,
        duplicate_init_spans,
        methods,
        span: SourceSpan {
            start: header.span_start,
            end: source_position_from_byte(source, body_end + 1),
        },
        name_span: header.name_span,
        type_id: 0,
    }
}

fn parse_class_method(
    source: &str,
    body: &str,
    body_offset: usize,
    index: usize,
) -> Option<(ClassMethod, usize)> {
    let mut cursor = index;
    let mut is_final = false;
    let mut is_override = false;

    loop {
        if starts_keyword(body, cursor, "final") {
            is_final = true;
            cursor = skip_whitespace(body, cursor + "final".len());
            continue;
        }

        if starts_keyword(body, cursor, "override") {
            is_override = true;
            cursor = skip_whitespace(body, cursor + "override".len());
            continue;
        }

        break;
    }

    let (name, name_start, name_end) = take_identifier(body, cursor)?;
    cursor = skip_whitespace(body, name_end);

    if !body[cursor..].starts_with('(') {
        return None;
    }

    let params_close = find_matching_paren(body, cursor)?;
    let parameters_span = SourceSpan {
        start: source_position_from_byte(source, body_offset + cursor + 1),
        end: source_position_from_byte(source, body_offset + params_close),
    };
    let parameters = body[cursor + 1..params_close].trim().to_owned();
    cursor = skip_whitespace(body, params_close + 1);

    let return_type = if body[cursor..].starts_with(':') {
        let return_start = skip_whitespace(body, cursor + 1);
        let brace = find_next_body_brace(body, return_start)?;
        let ty = body[return_start..brace].trim().to_owned();
        cursor = brace;
        (!ty.is_empty()).then_some(ty)
    } else {
        None
    };

    if !body[cursor..].starts_with('{') {
        return None;
    }

    let method_close = find_matching_brace(body, cursor)?;
    let method_body = body[cursor + 1..method_close].to_owned();

    Some((
        ClassMethod {
            name,
            parameters,
            return_type,
            body: method_body,
            is_final,
            is_override,
            span: SourceSpan {
                start: source_position_from_byte(source, body_offset + index),
                end: source_position_from_byte(source, body_offset + method_close + 1),
            },
            name_span: SourceSpan {
                start: source_position_from_byte(source, body_offset + name_start),
                end: source_position_from_byte(source, body_offset + name_end),
            },
            parameters_span,
        },
        method_close + 1,
    ))
}

fn parse_class_field(
    source: &str,
    body: &str,
    body_offset: usize,
    index: usize,
) -> Option<(ClassField, usize)> {
    let (name, name_start, name_end) = take_identifier(body, index)?;
    let mut cursor = skip_whitespace(body, name_end);

    if !body[cursor..].starts_with(':') {
        return None;
    }

    cursor = skip_whitespace(body, cursor + 1);
    let line_end = body[cursor..]
        .find('\n')
        .map(|offset| cursor + offset)
        .unwrap_or(body.len());
    let line = &body[cursor..line_end];
    let (ty, default) = match line.split_once('=') {
        Some((ty, default)) => (ty.trim().to_owned(), Some(default.trim().to_owned())),
        None => (line.trim().to_owned(), None),
    };

    if ty.is_empty() {
        return None;
    }

    Some((
        ClassField {
            name,
            parsed_ty: parse_tua_type_text(&ty),
            ty,
            default,
            span: SourceSpan {
                start: source_position_from_byte(source, body_offset + index),
                end: source_position_from_byte(source, body_offset + line_end),
            },
            name_span: SourceSpan {
                start: source_position_from_byte(source, body_offset + name_start),
                end: source_position_from_byte(source, body_offset + name_end),
            },
        },
        line_end,
    ))
}

fn lower_class_declaration(class: &ClassDeclaration, classes: &[ClassDeclaration]) -> String {
    let mut output = String::new();
    output.push_str(&format!("type {} = {{}}\n\n", class.name));
    output.push_str(&format!("local {} = {{}}\n", class.name));
    output.push_str(&format!("{}.__index = {}\n", class.name, class.name));
    output.push_str(&format!("{}.__type_id = {}\n", class.name, class.type_id));
    output.push_str(&format!("{}.__is = {{\n", class.name));

    for type_id in inherited_type_ids(class, classes) {
        output.push_str(&format!("  [{}] = true,\n", type_id));
    }

    output.push_str("}\n");

    output.push('\n');
    output.push_str(&lower_init_function(class, classes));
    output.push('\n');

    for method in &class.methods {
        output.push_str(&lower_method_function(class, method));
        output.push('\n');
    }

    if let Some(parent) = &class.parent {
        output.push_str(&format!(
            "setmetatable({}, {{ __index = {} }})\n\n",
            class.name, parent
        ));
    }

    output.push_str(&lower_constructor_function(class, classes));
    output.push('\n');
    output.push_str(&format!("{}.new = {}_new\n", class.name, class.name));
    output
}

fn lower_init_function(class: &ClassDeclaration, classes: &[ClassDeclaration]) -> String {
    let parameters = class
        .init
        .as_ref()
        .map(|init| init.parameters.as_str())
        .unwrap_or("");
    let mut output = String::new();

    output.push_str(&format!(
        "local function {}_init(self: {}{})\n",
        class.name,
        class.name,
        comma_prefixed(parameters)
    ));

    if let Some(init) = &class.init {
        output.push_str(&transform_method_body(class, init, classes));
    } else if let Some(parent) = &class.parent {
        output.push_str(&format!("  {}_init(self)\n", parent));
    }

    for field in &class.fields {
        if let Some(default) = &field.default {
            output.push_str(&format!("  self.{} = {}\n", field.name, default));
        }
    }

    output.push_str("end\n");
    output
}

fn lower_method_function(class: &ClassDeclaration, method: &ClassMethod) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "local function {}_{}(self: {}{}{}){}\n",
        class.name,
        method.name,
        class.name,
        comma_prefixed(&method.parameters),
        "",
        method
            .return_type
            .as_ref()
            .map(|ty| format!(": {ty}"))
            .unwrap_or_default()
    ));
    output.push_str(&transform_method_body(class, method, &[]));
    output.push_str("end\n");
    output.push_str(&format!(
        "{}.{} = {}_{}\n",
        class.name, method.name, class.name, method.name
    ));
    output
}

fn lower_constructor_function(class: &ClassDeclaration, classes: &[ClassDeclaration]) -> String {
    let parameters = class
        .init
        .as_ref()
        .map(|init| init.parameters.as_str())
        .unwrap_or("");
    let mut output = String::new();

    output.push_str(&format!(
        "local function {}_new({})\n",
        class.name, parameters
    ));

    if let Some(fields) = flatten_class_constructor(class, classes) {
        output.push_str("  return setmetatable({\n");
        for field in fields {
            output.push_str(&format!("    {} = {},\n", field.name, field.value));
        }
        output.push_str(&format!("  }}, {})\n", class.name));
    } else {
        output.push_str(&format!(
            "  local self = setmetatable({{}}, {})\n",
            class.name
        ));
        output.push_str(&format!(
            "  {}_init(self{})\n",
            class.name,
            comma_prefixed_call(parameters)
        ));
        output.push_str("  return self\n");
    }

    output.push_str("end\n");
    output
}

fn transform_method_body(
    class: &ClassDeclaration,
    method: &ClassMethod,
    classes: &[ClassDeclaration],
) -> String {
    let mut body = method.body.clone();

    if let Some(parent) = &class.parent {
        body = replace_super_calls(&body, parent);
    }

    let mut output = String::new();
    for line in body.lines() {
        if line.trim().is_empty() {
            output.push('\n');
        } else {
            output.push_str("  ");
            output.push_str(line.trim_start());
            output.push('\n');
        }
    }

    if classes.is_empty() {
        return output;
    }

    output
}

fn replace_super_calls(body: &str, parent: &str) -> String {
    let mut output = String::with_capacity(body.len());
    let mut index = 0usize;

    while index < body.len() {
        if body[index..].starts_with("--") {
            let end = consume_comment(body, index);
            output.push_str(&body[index..end]);
            index = end;
            continue;
        }

        if let Some(end) = consume_string_literal(body, index) {
            output.push_str(&body[index..end]);
            index = end;
            continue;
        }

        if starts_keyword(body, index, "super") {
            let after = skip_whitespace(body, index + "super".len());

            if body[after..].starts_with('(') {
                if let Some(close) = find_matching_paren(body, after) {
                    let args = body[after + 1..close].trim();
                    output.push_str(&format!(
                        "{}_init(self{})",
                        parent,
                        comma_prefixed_call(args)
                    ));
                    index = close + 1;
                    continue;
                }
            }

            if body[after..].starts_with('.') {
                let method_start = skip_whitespace(body, after + 1);
                if let Some((method, _, method_end)) = take_identifier(body, method_start) {
                    let call_start = skip_whitespace(body, method_end);
                    if body[call_start..].starts_with('(') {
                        if let Some(close) = find_matching_paren(body, call_start) {
                            let args = body[call_start + 1..close].trim();
                            output.push_str(&format!(
                                "{}_{}(self{})",
                                parent,
                                method,
                                comma_prefixed_call(args)
                            ));
                            index = close + 1;
                            continue;
                        }
                    }
                }
            }
        }

        let character = body[index..]
            .chars()
            .next()
            .expect("index should stay on a valid character boundary");
        output.push(character);
        index += character.len_utf8();
    }

    output
}

fn flatten_class_constructor(
    class: &ClassDeclaration,
    classes: &[ClassDeclaration],
) -> Option<Vec<FlattenedField>> {
    let mut substitutions = BTreeMap::new();
    let mut visiting = BTreeSet::new();
    let parameters = class
        .init
        .as_ref()
        .map(|init| parameter_names(&init.parameters))
        .unwrap_or_default();

    for parameter in &parameters {
        substitutions.insert(parameter.clone(), parameter.clone());
    }

    flatten_class_constructor_with_substitutions(class, classes, &substitutions, &mut visiting)
}

fn flatten_class_constructor_with_substitutions(
    class: &ClassDeclaration,
    classes: &[ClassDeclaration],
    substitutions: &BTreeMap<String, String>,
    visiting: &mut BTreeSet<String>,
) -> Option<Vec<FlattenedField>> {
    if !visiting.insert(class.name.clone()) {
        return None;
    }

    let mut fields = Vec::new();

    if let Some(parent_name) = &class.parent {
        let parent = class_by_name(classes, parent_name)?;
        let super_args = class
            .init
            .as_ref()
            .and_then(|init| leading_super_args(&init.body))
            .unwrap_or_default();
        let parent_parameters = parent
            .init
            .as_ref()
            .map(|init| parameter_names(&init.parameters))
            .unwrap_or_default();
        let mut parent_substitutions = BTreeMap::new();

        for (parameter, argument) in parent_parameters
            .into_iter()
            .zip(split_arguments(&super_args))
        {
            parent_substitutions.insert(parameter, substitute_expr(&argument, substitutions)?);
        }

        fields.extend(flatten_class_constructor_with_substitutions(
            parent,
            classes,
            &parent_substitutions,
            visiting,
        )?);
    }

    for field in &class.fields {
        if let Some(default) = &field.default {
            let value = substitute_expr(default, substitutions)?;
            if !is_flattenable_constructor_value(&value) {
                return None;
            }
            fields.push(FlattenedField {
                name: field.name.clone(),
                value,
            });
        }
    }

    if let Some(init) = &class.init {
        for (name, value) in self_assignments(&init.body, class.parent.is_some())? {
            let value = substitute_expr(&value, substitutions)?;
            if !is_flattenable_constructor_value(&value) {
                return None;
            }
            fields.push(FlattenedField { name, value });
        }
    }

    Some(fields)
}

fn is_flattenable_constructor_value(value: &str) -> bool {
    !value.trim_start().starts_with('{')
}

fn self_assignments(body: &str, allow_leading_super: bool) -> Option<Vec<(String, String)>> {
    let mut assignments = Vec::new();
    let mut saw_super = false;

    for raw_line in body.lines() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with("--") {
            continue;
        }

        if allow_leading_super && !saw_super && line.starts_with("super(") {
            saw_super = true;
            continue;
        }

        let rest = line.strip_prefix("self.")?;
        let (name, value) = rest.split_once('=')?;
        let name = name.trim();
        let value = value.trim();

        if !is_identifier(name) || !is_simple_expr(value) {
            return None;
        }

        assignments.push((name.to_owned(), value.to_owned()));
    }

    Some(assignments)
}

fn leading_super_args(body: &str) -> Option<String> {
    for raw_line in body.lines() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with("--") {
            continue;
        }

        let args = line.strip_prefix("super(")?.strip_suffix(')')?;
        return Some(args.trim().to_owned());
    }

    None
}

fn inherited_type_ids(class: &ClassDeclaration, classes: &[ClassDeclaration]) -> Vec<usize> {
    inherited_type_ids_with_visited(class, classes, &mut BTreeSet::new())
}

fn inherited_type_ids_with_visited(
    class: &ClassDeclaration,
    classes: &[ClassDeclaration],
    visited: &mut BTreeSet<String>,
) -> Vec<usize> {
    if !visited.insert(class.name.clone()) {
        return Vec::new();
    }

    let mut ids = Vec::new();

    if let Some(parent) = class
        .parent
        .as_ref()
        .and_then(|parent| class_by_name(classes, parent))
    {
        ids.extend(inherited_type_ids_with_visited(parent, classes, visited));
    }

    ids.push(class.type_id);
    ids
}

fn validate_classes(path: &Path, classes: &[ClassDeclaration], diagnostics: &mut Vec<Diagnostic>) {
    let mut names = BTreeSet::new();

    for class in classes {
        if !names.insert(class.name.clone()) {
            diagnostics.push(class_diagnostic(
                path,
                class.name_span.start,
                format!("duplicate class `{}`", class.name),
            ));
        }

        validate_class_members(path, class, diagnostics);
    }

    if validate_inheritance_cycles(path, classes, diagnostics) {
        return;
    }

    for class in classes {
        let Some(parent_name) = &class.parent else {
            continue;
        };
        let Some(parent) = class_by_name(classes, parent_name) else {
            diagnostics.push(class_diagnostic(
                path,
                class.name_span.start,
                format!("unknown parent class `{parent_name}`"),
            ));
            continue;
        };

        if !parent.is_open {
            diagnostics.push(class_diagnostic(
                path,
                class.name_span.start,
                format!("class `{}` extends final class `{parent_name}`", class.name),
            ));
        }

        validate_overrides(path, class, classes, diagnostics);
    }
}

fn validate_class_members(
    path: &Path,
    class: &ClassDeclaration,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut names = BTreeSet::new();

    for field in &class.fields {
        if !names.insert(field.name.clone()) {
            diagnostics.push(class_diagnostic(
                path,
                field.span.start,
                format!("duplicate class member `{}`", field.name),
            ));
        }
    }

    for method in &class.methods {
        if !names.insert(method.name.clone()) {
            diagnostics.push(class_diagnostic(
                path,
                method.name_span.start,
                format!("duplicate class member `{}`", method.name),
            ));
        }
    }

    for span in &class.duplicate_init_spans {
        diagnostics.push(class_diagnostic(
            path,
            span.start,
            "duplicate class member `init`",
        ));
    }
}

fn validate_inheritance_cycles(
    path: &Path,
    classes: &[ClassDeclaration],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let mut reported = BTreeSet::new();

    for class in classes {
        let mut chain = BTreeSet::new();
        let mut current = Some(class);

        while let Some(candidate) = current {
            if !chain.insert(candidate.name.clone()) {
                if reported.insert(candidate.name.clone()) {
                    diagnostics.push(class_diagnostic(
                        path,
                        candidate.name_span.start,
                        format!("inheritance cycle involving class `{}`", candidate.name),
                    ));
                }
                break;
            }

            current = candidate
                .parent
                .as_deref()
                .and_then(|parent| class_by_name(classes, parent));
        }
    }

    !reported.is_empty()
}

fn validate_overrides(
    path: &Path,
    class: &ClassDeclaration,
    classes: &[ClassDeclaration],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for method in &class.methods {
        let inherited = inherited_method(classes, class, &method.name);

        match (method.is_override, inherited) {
            (true, None) => diagnostics.push(class_diagnostic(
                path,
                method.name_span.start,
                format!(
                    "method `{}` is marked override but no parent method exists",
                    method.name
                ),
            )),
            (false, Some(_)) => diagnostics.push(class_diagnostic(
                path,
                method.name_span.start,
                format!(
                    "method `{}` overrides a parent method and must use `override`",
                    method.name
                ),
            )),
            (true, Some(parent_method)) if parent_method.is_final => {
                diagnostics.push(class_diagnostic(
                    path,
                    method.name_span.start,
                    format!("method `{}` overrides final parent method", method.name),
                ))
            }
            (true, Some(parent_method))
                if !class_method_signatures_match(method, parent_method) =>
            {
                diagnostics.push(class_diagnostic(
                    path,
                    method.name_span.start,
                    format!(
                        "method `{}` has a signature incompatible with its parent method",
                        method.name
                    ),
                ))
            }
            _ => {}
        }
    }
}

fn class_method_signatures_match(method: &ClassMethod, parent: &ClassMethod) -> bool {
    let Some((method_parameters, method_returns)) = class_method_signature(method) else {
        return false;
    };
    let Some((parent_parameters, parent_returns)) = class_method_signature(parent) else {
        return false;
    };

    method_parameters == parent_parameters && method_returns == parent_returns
}

fn class_method_signature(method: &ClassMethod) -> Option<ClassMethodSignature> {
    let return_type = method.return_type.as_deref().unwrap_or("any");
    let signature = parse_tua_type_text(&format!("({}) -> {return_type}", method.parameters));
    let TuaType::Function {
        parameters,
        returns,
    } = signature
    else {
        return None;
    };

    Some((
        parameters
            .into_iter()
            .map(|parameter| (parameter.ty, parameter.optional, parameter.variadic))
            .collect(),
        returns,
    ))
}

fn inherited_method<'a>(
    classes: &'a [ClassDeclaration],
    class: &ClassDeclaration,
    method: &str,
) -> Option<&'a ClassMethod> {
    let parent = class
        .parent
        .as_ref()
        .and_then(|parent| class_by_name(classes, parent))?;

    parent
        .methods
        .iter()
        .find(|candidate| candidate.name == method)
        .or_else(|| inherited_method(classes, parent, method))
}

fn class_diagnostic(
    path: &Path,
    position: SourcePosition,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(path, position.line, position.column, message)
        .with_code(DiagnosticCode::TYPE_CLASS)
}

fn comma_prefixed(value: &str) -> String {
    let value = value.trim();

    if value.is_empty() {
        String::new()
    } else {
        format!(", {value}")
    }
}

fn comma_prefixed_call(value: &str) -> String {
    let names = parameter_names(value).join(", ");

    if names.is_empty() {
        String::new()
    } else {
        format!(", {names}")
    }
}

fn parameter_names(parameters: &str) -> Vec<String> {
    split_arguments(parameters)
        .into_iter()
        .filter_map(|parameter| {
            let name = parameter
                .split_once(':')
                .map(|(name, _)| name)
                .unwrap_or(&parameter)
                .trim();

            is_identifier(name).then(|| name.to_owned())
        })
        .collect()
}

fn split_arguments(arguments: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    let mut index = 0usize;

    while index < arguments.len() {
        if let Some(end) = consume_string_literal(arguments, index) {
            index = end;
            continue;
        }

        let character = arguments[index..]
            .chars()
            .next()
            .expect("index should stay on a valid character boundary");

        match character {
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                result.push(arguments[start..index].trim().to_owned());
                start = index + 1;
            }
            _ => {}
        }

        index += character.len_utf8();
    }

    if start <= arguments.len() {
        let value = arguments[start..].trim();
        if !value.is_empty() {
            result.push(value.to_owned());
        }
    }

    result
}

fn substitute_expr(expr: &str, substitutions: &BTreeMap<String, String>) -> Option<String> {
    let expr = expr.trim();

    if !is_simple_expr(expr) {
        return None;
    }

    Some(
        substitutions
            .get(expr)
            .cloned()
            .unwrap_or_else(|| expr.to_owned()),
    )
}

fn is_simple_expr(expr: &str) -> bool {
    is_identifier(expr)
        || matches!(expr, "true" | "false" | "nil")
        || expr.parse::<f64>().is_ok()
        || ((expr.starts_with('"') && expr.ends_with('"'))
            || (expr.starts_with('\'') && expr.ends_with('\'')))
}

fn skip_body_ws_and_comments(body: &str, mut index: usize) -> usize {
    loop {
        index = skip_whitespace(body, index);

        if body[index..].starts_with("--") {
            index = consume_comment(body, index);
            continue;
        }

        return index;
    }
}

fn find_next_body_brace(body: &str, mut index: usize) -> Option<usize> {
    while index < body.len() {
        if let Some(end) = consume_string_literal(body, index) {
            index = end;
            continue;
        }

        if body[index..].starts_with('{') {
            return Some(index);
        }

        index += next_char_len(body, index);
    }

    None
}

fn find_matching_brace(source: &str, open: usize) -> Option<usize> {
    find_matching_delimiter(source, open, '{', '}')
}

fn find_matching_paren(source: &str, open: usize) -> Option<usize> {
    find_matching_delimiter(source, open, '(', ')')
}

fn find_matching_delimiter(source: &str, open: usize, left: char, right: char) -> Option<usize> {
    if !source[open..].starts_with(left) {
        return None;
    }

    let mut depth = 0usize;
    let mut index = open;

    while index < source.len() {
        if source[index..].starts_with("--") {
            index = consume_comment(source, index);
            continue;
        }

        if let Some(end) = consume_string_literal(source, index) {
            index = end;
            continue;
        }

        let character = source[index..]
            .chars()
            .next()
            .expect("index should stay on a valid character boundary");

        if character == left {
            depth += 1;
        } else if character == right {
            depth = depth.checked_sub(1)?;

            if depth == 0 {
                return Some(index);
            }
        }

        index += character.len_utf8();
    }

    None
}

fn take_identifier(source: &str, index: usize) -> Option<(String, usize, usize)> {
    let mut chars = source[index..].char_indices();
    let (_, first) = chars.next()?;

    if !is_identifier_start(first) {
        return None;
    }

    let mut end = index + first.len_utf8();

    for (offset, character) in chars {
        if !is_identifier_part(character) {
            return Some((
                source[index..index + offset].to_owned(),
                index,
                index + offset,
            ));
        }

        end = index + offset + character.len_utf8();
    }

    Some((source[index..end].to_owned(), index, end))
}

fn starts_keyword(source: &str, index: usize, keyword: &str) -> bool {
    can_start_keyword(source, index)
        && source[index..].starts_with(keyword)
        && source[index + keyword.len()..]
            .chars()
            .next()
            .map(|character| !is_identifier_part(character))
            .unwrap_or(true)
}

fn can_start_keyword(source: &str, index: usize) -> bool {
    source[..index]
        .chars()
        .next_back()
        .map(|character| !is_identifier_part(character))
        .unwrap_or(true)
}

fn skip_whitespace(source: &str, mut index: usize) -> usize {
    while index < source.len() {
        let character = source[index..]
            .chars()
            .next()
            .expect("index should stay on a valid character boundary");

        if !character.is_whitespace() {
            break;
        }

        index += character.len_utf8();
    }

    index
}

fn consume_comment(source: &str, index: usize) -> usize {
    source[index..]
        .find('\n')
        .map_or(source.len(), |offset| index + offset)
}

fn consume_string_literal(source: &str, index: usize) -> Option<usize> {
    let quote = match source[index..].chars().next()? {
        '"' => '"',
        '\'' => '\'',
        _ => return None,
    };
    let mut cursor = index + quote.len_utf8();
    let mut escaped = false;

    while cursor < source.len() {
        let character = source[cursor..]
            .chars()
            .next()
            .expect("cursor should stay on a valid character boundary");

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

fn next_char_len(source: &str, index: usize) -> usize {
    source[index..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or(1)
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

fn is_identifier(input: &str) -> bool {
    let mut chars = input.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    is_identifier_start(first) && chars.all(is_identifier_part)
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_identifier_part(character: char) -> bool {
    is_identifier_start(character) || character.is_ascii_digit()
}
