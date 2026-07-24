use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::Path;

use full_moon::ast::{self, LastStmt};
use full_moon::node::Node;
use full_moon::visitors::{Visit, Visitor};

use crate::ast_utils::{field_chain_before_first_call, plain_field_chain_from_var, token_text};
use crate::parser::{
    parse_raw_tua_source, parse_tua_type_text, ParsedSource, RawParsedSource, SourcePosition,
    SourceSpan, TuaType, TypedSourceModel,
};
use crate::scope::{scope_map_parsed_source, ScopeDeclarationKind, ScopeSymbolId};
use crate::types::{Type, TypeEnvironment};
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
    pub body_span: SourceSpan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClassMemberKind {
    Field,
    Method,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClassMemberTarget {
    pub kind: ClassMemberKind,
    pub span: SourceSpan,
    pub ty: TuaType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClassLintFact {
    ParentConstructorPath {
        class_name: String,
        parent_name: String,
        span: SourceSpan,
    },
    VirtualCallInInit {
        class_name: String,
        method_name: String,
        span: SourceSpan,
    },
    RedundantInheritedField {
        class_name: String,
        parent_name: String,
        field_name: String,
        span: SourceSpan,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClassBlock {
    declaration: ClassDeclaration,
    invalid_member_span: Option<SourceSpan>,
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
type InitializedFields = BTreeSet<String>;

pub(crate) fn nominal_class_identity(path: &Path, class_name: &str) -> String {
    format!("{}::class::{class_name}", path.to_string_lossy())
}

pub(crate) fn class_member_target(
    parsed: &ParsedSource,
    class_name: &str,
    member: &str,
) -> Option<ClassMemberTarget> {
    let mut current = class_name;
    let mut visited = BTreeSet::new();

    while visited.insert(current.to_owned()) {
        let class = parsed
            .class_declarations
            .iter()
            .find(|class| class.name == current)?;

        if let Some(field) = class.fields.iter().find(|field| field.name == member) {
            return Some(ClassMemberTarget {
                kind: ClassMemberKind::Field,
                span: field.name_span,
                ty: field.parsed_ty.clone(),
            });
        }

        if let Some(method) = class.methods.iter().find(|method| method.name == member) {
            return Some(ClassMemberTarget {
                kind: ClassMemberKind::Method,
                span: method.name_span,
                ty: parse_tua_type_text(&format!(
                    "({}) -> {}",
                    method.parameters,
                    method.return_type.as_deref().unwrap_or("any")
                )),
            });
        }

        current = class.parent.as_deref()?;
    }

    None
}

pub(crate) fn parse_class_declarations(
    path: &Path,
    source: &str,
) -> (Vec<ClassDeclaration>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let blocks = class_blocks(source);

    for span in blocks.iter().filter_map(|block| block.invalid_member_span) {
        diagnostics.push(class_diagnostic(
            path,
            span.start,
            "unrecognized class member; expected a typed field or method declaration",
        ));
    }

    let mut declarations = blocks
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

    let declaration_indices = declarations
        .iter()
        .enumerate()
        .map(|(index, declaration)| (declaration.name.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let forward_parent_names = declarations
        .iter()
        .enumerate()
        .filter_map(|(index, declaration)| {
            let parent = declaration.parent.as_ref()?;
            declaration_indices
                .get(parent)
                .is_some_and(|parent_index| *parent_index > index)
                .then(|| parent.clone())
        })
        .collect::<BTreeSet<_>>();
    let forward_parent_declarations = declarations
        .iter()
        .filter(|declaration| forward_parent_names.contains(&declaration.name))
        .map(|declaration| format!("local {} = {{}}\n", declaration.name))
        .collect::<String>();

    blocks
        .into_iter()
        .zip(declarations.iter())
        .enumerate()
        .map(|(index, (block, declaration))| {
            let parent_is_forward = declaration.parent.as_ref().is_some_and(|parent| {
                declaration_indices
                    .get(parent)
                    .is_some_and(|parent_index| *parent_index > index)
            });
            let mut replacement = lower_class_declaration(
                declaration,
                &declarations,
                forward_parent_names.contains(&declaration.name),
                parent_is_forward,
            );
            if index == 0 {
                replacement.insert_str(0, &forward_parent_declarations);
            }

            ClassLoweringEdit {
                range: block.start..block.end,
                replacement,
                source_span: declaration.span,
            }
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
        let (declaration, invalid_member_span) =
            parse_class_body(source, body, header.clone(), body_end);

        blocks.push(ClassBlock {
            declaration,
            invalid_member_span,
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
) -> (ClassDeclaration, Option<SourceSpan>) {
    let mut fields = Vec::new();
    let mut init = None;
    let mut duplicate_init_spans = Vec::new();
    let mut methods = Vec::new();
    let mut invalid_member_span = None;
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

        let line_end = body[cursor..]
            .find('\n')
            .map(|offset| cursor + offset)
            .unwrap_or(body.len());
        invalid_member_span = Some(SourceSpan {
            start: source_position_from_byte(source, header.body_start + cursor),
            end: source_position_from_byte(source, header.body_start + line_end),
        });
        break;
    }

    (
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
        },
        invalid_member_span,
    )
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
        let brace = find_method_body_brace(body, return_start)?;
        let ty = body[return_start..brace].trim().to_owned();
        cursor = brace;
        (!ty.is_empty()).then_some(ty)
    } else {
        None
    };

    if !body[cursor..].starts_with('{') {
        return None;
    }

    let method_body_start = cursor + 1;
    let method_close = find_matching_brace(body, cursor)?;
    let method_body = body[method_body_start..method_close].to_owned();

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
            body_span: SourceSpan {
                start: source_position_from_byte(source, body_offset + method_body_start),
                end: source_position_from_byte(source, body_offset + method_close),
            },
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

fn lower_class_declaration(
    class: &ClassDeclaration,
    classes: &[ClassDeclaration],
    table_predeclared: bool,
    parent_is_forward: bool,
) -> String {
    let mut output = String::new();
    output.push_str(&format!("type {} = {{}}\n\n", class.name));
    if !table_predeclared {
        output.push_str(&format!("local {} = {{}}\n", class.name));
    }
    output.push_str(&format!("{}.__index = {}\n", class.name, class.name));
    output.push_str(&format!("{}.__type_id = {}\n", class.name, class.type_id));
    output.push_str(&format!("{}.__is = {{\n", class.name));

    for type_id in inherited_type_ids(class, classes) {
        output.push_str(&format!("  [{}] = true,\n", type_id));
    }

    output.push_str("}\n");

    output.push('\n');
    output.push_str(&lower_init_function(class, classes, parent_is_forward));
    if table_predeclared {
        output.push_str(&format!("{}.__init = {}_init\n", class.name, class.name));
    }
    output.push('\n');

    for method in &class.methods {
        output.push_str(&lower_method_function(class, method, parent_is_forward));
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

fn lower_init_function(
    class: &ClassDeclaration,
    classes: &[ClassDeclaration],
    parent_is_forward: bool,
) -> String {
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
        output.push_str(&transform_method_body(
            class,
            init,
            classes,
            parent_is_forward,
        ));
    } else if let Some(parent) = &class.parent {
        if parent_is_forward {
            output.push_str(&format!("  {}.__init(self)\n", parent));
        } else {
            output.push_str(&format!("  {}_init(self)\n", parent));
        }
    }

    for field in &class.fields {
        if let Some(default) = &field.default {
            output.push_str(&format!("  self.{} = {}\n", field.name, default));
        }
    }

    output.push_str("end\n");
    output
}

fn lower_method_function(
    class: &ClassDeclaration,
    method: &ClassMethod,
    parent_is_forward: bool,
) -> String {
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
    output.push_str(&transform_method_body(
        class,
        method,
        &[],
        parent_is_forward,
    ));
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
            comma_prefixed_parameter_values(parameters)
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
    parent_is_forward: bool,
) -> String {
    let mut body = method.body.clone();

    if let Some(parent) = &class.parent {
        body = replace_super_calls(&body, parent, parent_is_forward);
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

fn replace_super_calls(body: &str, parent: &str, parent_is_forward: bool) -> String {
    let mut output = String::with_capacity(body.len());
    let mut index = 0usize;
    let mut last_significant = None;

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
            last_significant = Some('"');
            continue;
        }

        if starts_super_reference(body, index, last_significant) {
            let after = skip_whitespace(body, index + "super".len());

            if body[after..].starts_with('(') {
                if let Some(close) = find_matching_paren(body, after) {
                    let args = body[after + 1..close].trim();
                    let init = if parent_is_forward {
                        format!("{parent}.__init")
                    } else {
                        format!("{parent}_init")
                    };
                    output.push_str(&format!("{}(self{})", init, comma_prefixed_arguments(args)));
                    index = close + 1;
                    last_significant = Some(')');
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
                            let method = if parent_is_forward {
                                format!("{parent}.{method}")
                            } else {
                                format!("{parent}_{method}")
                            };
                            output.push_str(&format!(
                                "{}(self{})",
                                method,
                                comma_prefixed_arguments(args)
                            ));
                            index = close + 1;
                            last_significant = Some(')');
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
        if !character.is_whitespace() {
            last_significant = Some(character);
        }
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

    if let Some(init) = &class.init {
        for (name, value) in self_assignments(&init.body, class.parent.is_some())? {
            let value = substitute_expr(&value, substitutions)?;
            if !is_flattenable_constructor_value(&value) {
                return None;
            }
            fields.push(FlattenedField { name, value });
        }
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

pub(crate) fn class_contract_diagnostics(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
) -> Vec<Diagnostic> {
    // Class syntax has already been lowered into the same init functions that
    // checking and emission consume. Analyze those functions so field defaults,
    // super calls, forward parents, and source behavior cannot drift apart.
    let mut diagnostics = Vec::new();
    let context = environment.to_type_context();
    let mut initialization = ClassInitializationAnalysis::new(parsed);

    for class in &parsed.class_declarations {
        for field in &class.fields {
            let Some((parent, parent_field)) =
                inherited_field(&parsed.class_declarations, class, &field.name)
            else {
                continue;
            };
            let Some(field_type) = class_field_type(environment, &class.name, &field.name) else {
                continue;
            };
            let Some(parent_type) = class_field_type(environment, &parent.name, &parent_field.name)
            else {
                continue;
            };

            if environment.resolve_aliases(field_type) != environment.resolve_aliases(parent_type) {
                diagnostics.push(class_diagnostic(
                    &parsed.path,
                    field.name_span.start,
                    format!(
                        "inherited field `{}` in class `{}` has type `{}`, but class `{}` declares `{}`",
                        field.name, class.name, field.ty, parent.name, parent_field.ty
                    ),
                ));
            }
        }

        let initialized = initialization.initialized_fields(&class.name);
        let mut effective_fields = BTreeMap::new();
        collect_effective_class_fields(
            &parsed.class_declarations,
            class,
            &mut effective_fields,
            &mut BTreeSet::new(),
        );

        for (name, (owner, field)) in effective_fields {
            let Some(field_type) = class_field_type(environment, &class.name, &name) else {
                continue;
            };

            if context.is_assignable(&Type::nil(), field_type) || initialized.contains(&name) {
                continue;
            }

            if owner.name != class.name {
                let parent_initializes_field = class.parent.as_deref().is_some_and(|parent| {
                    initialization.initialized_fields(parent).contains(&name)
                });
                if !parent_initializes_field {
                    continue;
                }
            }

            let (position, message) = if owner.name == class.name {
                (
                    field.name_span.start,
                    format!(
                        "class `{}` does not initialize required field `{name}`",
                        class.name
                    ),
                )
            } else {
                (
                    class.name_span.start,
                    format!(
                        "class `{}` does not initialize inherited field `{name}`",
                        class.name
                    ),
                )
            };
            diagnostics.push(class_diagnostic(&parsed.path, position, message));
        }
    }

    diagnostics
}

pub(crate) fn class_lint_facts(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
) -> Vec<ClassLintFact> {
    let mut facts = Vec::new();
    let mut initialization = ClassInitializationAnalysis::new(parsed);

    for class in &parsed.class_declarations {
        if let (Some(init), Some(parent_name)) = (&class.init, &class.parent) {
            let initialization_facts = initialization.initialization_facts(&class.name);
            if initialization_facts.has_returning_exit
                && !initialization_facts.parent_called_on_every_exit
            {
                facts.push(ClassLintFact::ParentConstructorPath {
                    class_name: class.name.clone(),
                    parent_name: parent_name.clone(),
                    span: init.name_span,
                });
            }
        }

        if let Some(init) = class.init.as_ref().filter(|_| class.is_open) {
            for (method_name, span) in direct_self_calls_in_method(init) {
                if class_method_declaration(&parsed.class_declarations, &class.name, &method_name)
                    .is_some_and(|(_, method)| !method.is_final)
                {
                    facts.push(ClassLintFact::VirtualCallInInit {
                        class_name: class.name.clone(),
                        method_name,
                        span,
                    });
                }
            }
        }

        for field in class.fields.iter().filter(|field| field.default.is_none()) {
            let Some((parent, parent_field)) =
                inherited_field(&parsed.class_declarations, class, &field.name)
            else {
                continue;
            };
            let Some(field_type) = class_field_type(environment, &class.name, &field.name) else {
                continue;
            };
            let Some(parent_type) = class_field_type(environment, &parent.name, &parent_field.name)
            else {
                continue;
            };

            if environment.resolve_aliases(field_type) == environment.resolve_aliases(parent_type) {
                facts.push(ClassLintFact::RedundantInheritedField {
                    class_name: class.name.clone(),
                    parent_name: parent.name.clone(),
                    field_name: field.name.clone(),
                    span: field.name_span,
                });
            }
        }
    }

    facts
}

#[derive(Default)]
struct DirectFunctionCallCollector {
    nested_function_depth: usize,
    calls: Vec<ast::FunctionCall>,
}

impl Visitor for DirectFunctionCallCollector {
    fn visit_function_declaration(&mut self, _function: &ast::FunctionDeclaration) {
        self.nested_function_depth += 1;
    }

    fn visit_function_declaration_end(&mut self, _function: &ast::FunctionDeclaration) {
        self.nested_function_depth = self.nested_function_depth.saturating_sub(1);
    }

    fn visit_local_function(&mut self, _function: &ast::LocalFunction) {
        self.nested_function_depth += 1;
    }

    fn visit_local_function_end(&mut self, _function: &ast::LocalFunction) {
        self.nested_function_depth = self.nested_function_depth.saturating_sub(1);
    }

    fn visit_anonymous_function(&mut self, _function: &ast::AnonymousFunction) {
        self.nested_function_depth += 1;
    }

    fn visit_anonymous_function_end(&mut self, _function: &ast::AnonymousFunction) {
        self.nested_function_depth = self.nested_function_depth.saturating_sub(1);
    }

    fn visit_function_call(&mut self, call: &ast::FunctionCall) {
        if self.nested_function_depth == 0 {
            self.calls.push(call.clone());
        }
    }
}

fn direct_self_calls_in_method(method: &ClassMethod) -> Vec<(String, SourceSpan)> {
    const WRAPPER_PREFIX: &str = "local function __tua_class_init(self)\n";

    let wrapper = format!("{WRAPPER_PREFIX}{}\nend", method.body);
    let Ok(raw) = parse_raw_tua_source("<class-init>", wrapper) else {
        return Vec::new();
    };
    let parsed = ParsedSource::from_raw_and_typed(raw, TypedSourceModel::default());
    let scope = scope_map_parsed_source(&parsed);
    let Some(self_symbol) = scope.declarations().iter().find_map(|declaration| {
        (declaration.name == "self"
            && declaration.kind == ScopeDeclarationKind::Parameter
            && declaration.name_span.start.line == 1)
            .then_some(declaration.id)
    }) else {
        return Vec::new();
    };
    let Some(block) = parsed.ast.nodes().stmts().find_map(|statement| {
        let ast::Stmt::LocalFunction(function) = statement else {
            return None;
        };
        Some(function.body().block().clone())
    }) else {
        return Vec::new();
    };

    let mut collector = DirectFunctionCallCollector::default();
    block.visit(&mut collector);

    collector
        .calls
        .into_iter()
        .filter_map(|call| {
            let parts = field_chain_before_first_call(&call)?;
            let [receiver, method_name] = parts.as_slice() else {
                return None;
            };
            if receiver != "self" {
                return None;
            }
            if !call_root_resolves_to(&scope, &call, self_symbol) {
                return None;
            }

            let span = method_body_node_span(&parsed.raw, method, &call)?;
            Some((method_name.clone(), span))
        })
        .collect()
}

fn call_root_resolves_to(
    scope: &crate::scope::ScopeMap,
    call: &ast::FunctionCall,
    symbol: ScopeSymbolId,
) -> bool {
    let ast::Prefix::Name(root) = call.prefix() else {
        return false;
    };
    let Some((start, _)) = root.range() else {
        return false;
    };

    scope.symbol_at_position(SourcePosition::from_full_moon(start)) == Some(symbol)
}

fn method_body_node_span(
    raw: &RawParsedSource,
    method: &ClassMethod,
    node: &impl Node,
) -> Option<SourceSpan> {
    let (start, end) = node.range()?;
    let start = raw
        .parser_position_origin(SourcePosition::from_full_moon(start))?
        .source_position;
    let end = raw
        .parser_position_origin(SourcePosition::from_full_moon(end))?
        .source_position;

    Some(SourceSpan {
        start: method_body_source_position(method, start)?,
        end: method_body_source_position(method, end)?,
    })
}

fn method_body_source_position(
    method: &ClassMethod,
    wrapper_position: SourcePosition,
) -> Option<SourcePosition> {
    let body_line = wrapper_position.line.checked_sub(1)?;
    if body_line == 0 {
        return None;
    }

    Some(SourcePosition {
        line: method.body_span.start.line + body_line - 1,
        column: if body_line == 1 {
            method.body_span.start.column + wrapper_position.column - 1
        } else {
            wrapper_position.column
        },
    })
}

fn class_field_type<'a>(
    environment: &'a TypeEnvironment,
    class: &str,
    field: &str,
) -> Option<&'a Type> {
    let Type::Table(table) = environment.alias(class)? else {
        return None;
    };

    table.field(field).map(|field| &field.ty)
}

fn inherited_field<'a>(
    classes: &'a [ClassDeclaration],
    class: &ClassDeclaration,
    field: &str,
) -> Option<(&'a ClassDeclaration, &'a ClassField)> {
    let parent = class
        .parent
        .as_deref()
        .and_then(|parent| class_by_name(classes, parent))?;

    parent
        .fields
        .iter()
        .find(|candidate| candidate.name == field)
        .map(|field| (parent, field))
        .or_else(|| inherited_field(classes, parent, field))
}

fn collect_effective_class_fields<'a>(
    classes: &'a [ClassDeclaration],
    class: &'a ClassDeclaration,
    fields: &mut BTreeMap<String, (&'a ClassDeclaration, &'a ClassField)>,
    visited: &mut BTreeSet<String>,
) {
    if !visited.insert(class.name.clone()) {
        return;
    }

    if let Some(parent) = class
        .parent
        .as_deref()
        .and_then(|parent| class_by_name(classes, parent))
    {
        collect_effective_class_fields(classes, parent, fields, visited);
    }

    for field in &class.fields {
        fields.insert(field.name.clone(), (class, field));
    }
}

#[derive(Clone, Debug, Default)]
struct InitializationState {
    fields: InitializedFields,
    parent_called: bool,
}

#[derive(Clone, Debug, Default)]
struct ClassInitializationFacts {
    fields: InitializedFields,
    has_returning_exit: bool,
    parent_called_on_every_exit: bool,
}

#[derive(Clone, Debug)]
struct InitializationFlow {
    fallthrough: Option<InitializationState>,
    returns: Vec<InitializationState>,
    breaks: Vec<InitializationState>,
    continues: Vec<InitializationState>,
}

impl InitializationFlow {
    fn fallthrough(state: InitializationState) -> Self {
        Self {
            fallthrough: Some(state),
            returns: Vec::new(),
            breaks: Vec::new(),
            continues: Vec::new(),
        }
    }

    fn append_exits(&mut self, other: &mut Self) {
        self.returns.append(&mut other.returns);
        self.breaks.append(&mut other.breaks);
        self.continues.append(&mut other.continues);
    }
}

struct ClassInitializationAnalysis<'a> {
    parsed: &'a ParsedSource,
    init_blocks: BTreeMap<String, ast::Block>,
    facts: BTreeMap<String, ClassInitializationFacts>,
    visiting: BTreeSet<String>,
}

impl<'a> ClassInitializationAnalysis<'a> {
    fn new(parsed: &'a ParsedSource) -> Self {
        let class_names = parsed
            .class_declarations
            .iter()
            .map(|class| class.name.as_str())
            .collect::<BTreeSet<_>>();
        let init_blocks = parsed
            .ast
            .nodes()
            .stmts()
            .filter_map(|statement| {
                let ast::Stmt::LocalFunction(function) = statement else {
                    return None;
                };
                let name = token_text(function.name());
                let class = name.strip_suffix("_init")?;
                if !class_names.contains(class) || !is_class_generated_node(parsed, function.name())
                {
                    return None;
                }

                Some((class.to_owned(), function.body().block().clone()))
            })
            .collect();

        Self {
            parsed,
            init_blocks,
            facts: BTreeMap::new(),
            visiting: BTreeSet::new(),
        }
    }

    fn initialized_fields(&mut self, class_name: &str) -> InitializedFields {
        self.initialization_facts(class_name).fields
    }

    fn initialization_facts(&mut self, class_name: &str) -> ClassInitializationFacts {
        if let Some(facts) = self.facts.get(class_name) {
            return facts.clone();
        }
        if !self.visiting.insert(class_name.to_owned()) {
            return ClassInitializationFacts::default();
        }

        let Some(class) = class_by_name(&self.parsed.class_declarations, class_name) else {
            self.visiting.remove(class_name);
            return ClassInitializationFacts::default();
        };
        let parent = class.parent.clone();
        let flow = self.init_blocks.get(class_name).cloned().map(|block| {
            self.analyze_block(&block, InitializationState::default(), parent.as_deref())
        });
        let facts = flow.map(initialization_facts_on_exits).unwrap_or_default();

        self.visiting.remove(class_name);
        self.facts.insert(class_name.to_owned(), facts.clone());
        facts
    }

    fn analyze_block(
        &mut self,
        block: &ast::Block,
        state: InitializationState,
        parent: Option<&str>,
    ) -> InitializationFlow {
        let mut flow = InitializationFlow::fallthrough(state);

        for statement in block.stmts() {
            let Some(current) = flow.fallthrough.take() else {
                break;
            };
            let mut statement_flow = self.analyze_statement(statement, current, parent);
            flow.fallthrough = statement_flow.fallthrough.take();
            flow.append_exits(&mut statement_flow);
        }

        let Some(current) = flow.fallthrough.take() else {
            return flow;
        };
        match block.last_stmt() {
            Some(LastStmt::Return(_)) => flow.returns.push(current),
            Some(LastStmt::Break(_)) => flow.breaks.push(current),
            Some(LastStmt::Continue(_)) => flow.continues.push(current),
            Some(_) | None => flow.fallthrough = Some(current),
        }

        flow
    }

    fn analyze_statement(
        &mut self,
        statement: &ast::Stmt,
        mut state: InitializationState,
        parent: Option<&str>,
    ) -> InitializationFlow {
        match statement {
            ast::Stmt::Assignment(assignment) => {
                for variable in assignment.variables() {
                    let Some(parts) = plain_field_chain_from_var(variable) else {
                        continue;
                    };
                    if let [receiver, field] = parts.as_slice() {
                        if receiver == "self" {
                            state.fields.insert(field.clone());
                        }
                    }
                }
                InitializationFlow::fallthrough(state)
            }
            ast::Stmt::FunctionCall(call) => {
                if let Some(parent) = parent {
                    if calls_parent_initializer(call, parent) {
                        state.fields.extend(self.initialized_fields(parent));
                        state.parent_called = true;
                    }
                }
                InitializationFlow::fallthrough(state)
            }
            ast::Stmt::Do(statement) => self.analyze_block(statement.block(), state, parent),
            ast::Stmt::If(statement) => self.analyze_if(statement, state, parent),
            ast::Stmt::GenericFor(statement) => {
                self.analyze_zero_or_more_loop(statement.block(), state, parent)
            }
            ast::Stmt::NumericFor(statement) => {
                self.analyze_zero_or_more_loop(statement.block(), state, parent)
            }
            ast::Stmt::While(statement) => {
                self.analyze_zero_or_more_loop(statement.block(), state, parent)
            }
            ast::Stmt::Repeat(statement) => {
                let mut body = self.analyze_block(statement.block(), state, parent);
                let mut exits = Vec::new();
                exits.extend(body.fallthrough.take());
                exits.append(&mut body.breaks);
                exits.append(&mut body.continues);

                InitializationFlow {
                    fallthrough: intersect_initialization_states(exits),
                    returns: body.returns,
                    breaks: Vec::new(),
                    continues: Vec::new(),
                }
            }
            _ => InitializationFlow::fallthrough(state),
        }
    }

    fn analyze_if(
        &mut self,
        statement: &ast::If,
        state: InitializationState,
        parent: Option<&str>,
    ) -> InitializationFlow {
        let mut branches = vec![self.analyze_block(statement.block(), state.clone(), parent)];

        if let Some(else_ifs) = statement.else_if() {
            branches.extend(
                else_ifs
                    .iter()
                    .map(|else_if| self.analyze_block(else_if.block(), state.clone(), parent)),
            );
        }
        branches.push(
            statement
                .else_block()
                .map(|block| self.analyze_block(block, state.clone(), parent))
                .unwrap_or_else(|| InitializationFlow::fallthrough(state)),
        );

        merge_initialization_branches(branches)
    }

    fn analyze_zero_or_more_loop(
        &mut self,
        block: &ast::Block,
        state: InitializationState,
        parent: Option<&str>,
    ) -> InitializationFlow {
        let body = self.analyze_block(block, state.clone(), parent);

        InitializationFlow {
            fallthrough: Some(state),
            returns: body.returns,
            breaks: Vec::new(),
            continues: Vec::new(),
        }
    }
}

fn calls_parent_initializer(call: &ast::FunctionCall, parent: &str) -> bool {
    let Some(parts) = field_chain_before_first_call(call) else {
        return false;
    };

    match parts.as_slice() {
        [function] => function.strip_suffix("_init") == Some(parent),
        [receiver, method] => receiver == parent && method == "__init",
        _ => false,
    }
}

fn is_class_generated_node(parsed: &ParsedSource, node: &impl Node) -> bool {
    node.start_position()
        .map(SourcePosition::from_full_moon)
        .and_then(|position| parsed.raw.parser_position_origin(position))
        .is_some_and(|origin| origin.class_generated)
}

fn merge_initialization_branches(
    branches: impl IntoIterator<Item = InitializationFlow>,
) -> InitializationFlow {
    let mut fallthrough = Vec::new();
    let mut returns = Vec::new();
    let mut breaks = Vec::new();
    let mut continues = Vec::new();

    for mut branch in branches {
        fallthrough.extend(branch.fallthrough.take());
        returns.append(&mut branch.returns);
        breaks.append(&mut branch.breaks);
        continues.append(&mut branch.continues);
    }

    InitializationFlow {
        fallthrough: intersect_initialization_states(fallthrough),
        returns,
        breaks,
        continues,
    }
}

fn initialization_facts_on_exits(mut flow: InitializationFlow) -> ClassInitializationFacts {
    let mut exits = Vec::new();
    exits.extend(flow.fallthrough.take());
    exits.append(&mut flow.returns);
    let has_returning_exit = !exits.is_empty();
    let state = intersect_initialization_states(exits).unwrap_or_default();

    ClassInitializationFacts {
        fields: state.fields,
        has_returning_exit,
        parent_called_on_every_exit: has_returning_exit && state.parent_called,
    }
}

fn intersect_initialization_states(
    mut states: Vec<InitializationState>,
) -> Option<InitializationState> {
    let mut intersection = states.pop()?;

    for state in states {
        intersection
            .fields
            .retain(|field| state.fields.contains(field));
        intersection.parent_called &= state.parent_called;
    }

    Some(intersection)
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

fn comma_prefixed_parameter_values(value: &str) -> String {
    let names = parameter_names(value).join(", ");

    if names.is_empty() {
        String::new()
    } else {
        format!(", {names}")
    }
}

fn comma_prefixed_arguments(value: &str) -> String {
    comma_prefixed(value)
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
            let name = name.strip_suffix('?').unwrap_or(name).trim();

            (name == "..." || is_identifier(name)).then(|| name.to_owned())
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

fn find_method_body_brace(body: &str, return_start: usize) -> Option<usize> {
    let mut index = return_start;

    while index < body.len() {
        if body[index..].starts_with("--") {
            index = consume_comment(body, index);
            continue;
        }

        if let Some(end) = consume_string_literal(body, index) {
            index = end;
            continue;
        }

        if body[index..].starts_with('{') {
            let prefix = body[return_start..index].trim();

            if complete_type_prefix(prefix) {
                return Some(index);
            }

            index = find_matching_brace(body, index)? + 1;
            continue;
        }

        index += next_char_len(body, index);
    }

    None
}

fn complete_type_prefix(prefix: &str) -> bool {
    let prefix = prefix.trim_end();

    !prefix.is_empty()
        && !prefix.ends_with('|')
        && !prefix.ends_with("->")
        && !matches!(prefix.chars().next_back(), Some(',' | ':' | '<'))
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

fn starts_super_reference(source: &str, index: usize, last_significant: Option<char>) -> bool {
    starts_keyword(source, index, "super") && !matches!(last_significant, Some('.' | ':'))
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
    if let Some(end) = consume_long_bracket(source, index + 2) {
        return end;
    }

    source[index..]
        .find('\n')
        .map_or(source.len(), |offset| index + offset)
}

fn consume_string_literal(source: &str, index: usize) -> Option<usize> {
    if let Some(end) = consume_long_bracket(source, index) {
        return Some(end);
    }

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

fn consume_long_bracket(source: &str, index: usize) -> Option<usize> {
    let bytes = source.as_bytes();

    if bytes.get(index) != Some(&b'[') {
        return None;
    }

    let mut open_end = index + 1;
    while bytes.get(open_end) == Some(&b'=') {
        open_end += 1;
    }

    if bytes.get(open_end) != Some(&b'[') {
        return None;
    }

    let equals = open_end - index - 1;
    let mut cursor = open_end + 1;

    while cursor < bytes.len() {
        if bytes[cursor] != b']' {
            cursor += 1;
            continue;
        }

        let mut close_end = cursor + 1;
        for _ in 0..equals {
            if bytes.get(close_end) != Some(&b'=') {
                break;
            }
            close_end += 1;
        }

        if close_end == cursor + equals + 1 && bytes.get(close_end) == Some(&b']') {
            return Some(close_end + 1);
        }

        cursor += 1;
    }

    Some(source.len())
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
