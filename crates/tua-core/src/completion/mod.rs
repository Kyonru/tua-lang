use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use self::context::{completion_context, CompletionContext, FieldAccessSeparator};
use self::require::{complete_import_paths, complete_require_paths};
use self::symbol::lua_symbols_before_position;
use crate::lua::{self, LuaItem, LuaItemKind};
use crate::module_index::{exported_field_for_fields, required_module_exports_for_receiver};
use crate::profile;
use crate::semantic::{resolve_aliases, SemanticModel};
use crate::source_text::line_at;
use crate::text_match::{text_match_rank, text_matches, TextMatchRank};
use crate::types::{build_type_environment_with_diagnostics, TableShape, Type, TypeEnvironment};
use crate::{asset, love_conf, vector};
use crate::{
    parse_tua_source, Diagnostic, FunctionAnnotation, ParsedSource, SourcePosition, TextEdit,
    TuaType,
};

pub(crate) mod context;
mod require;
mod symbol;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Completion {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: Option<String>,
    pub filter_text: Option<String>,
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

pub(crate) fn filter_and_rank_completions(
    completions: Vec<Completion>,
    prefix: &str,
) -> Vec<Completion> {
    if prefix.is_empty() {
        return completions;
    }

    let mut ranked = completions
        .into_iter()
        .filter_map(|completion| {
            completion_match_rank(&completion, prefix).map(|rank| (rank, completion))
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|(left_rank, left), (right_rank, right)| {
        left_rank
            .cmp(right_rank)
            .then_with(|| left.label.cmp(&right.label))
    });

    ranked
        .into_iter()
        .map(|(_, mut completion)| {
            completion
                .filter_text
                .get_or_insert_with(|| prefix.to_owned());
            completion
        })
        .collect()
}

fn completion_match_rank(completion: &Completion, prefix: &str) -> Option<TextMatchRank> {
    let label_rank = completion_text_rank(&completion.label, prefix);
    let filter_rank = completion
        .filter_text
        .as_deref()
        .and_then(|filter_text| completion_text_rank(filter_text, prefix));

    label_rank.into_iter().chain(filter_rank).min()
}

fn completion_text_matches(value: &str, prefix: &str) -> bool {
    text_matches(value, prefix)
}

fn completion_text_rank(value: &str, prefix: &str) -> Option<TextMatchRank> {
    text_match_rank(value, prefix)
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
    let (environment, _) = build_type_environment_with_diagnostics(&parsed);

    Ok(complete_parsed_source_with_context(
        &parsed,
        &source,
        context,
        &environment,
    ))
}

pub fn complete_parsed_source(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Result<Vec<Completion>, Vec<Diagnostic>> {
    let (environment, _) = build_type_environment_with_diagnostics(parsed);

    Ok(complete_parsed_source_with_environment(
        parsed,
        position,
        &environment,
    ))
}

pub(crate) fn complete_parsed_source_with_environment(
    parsed: &ParsedSource,
    position: SourcePosition,
    environment: &TypeEnvironment,
) -> Vec<Completion> {
    complete_source_with_parsed_environment(&parsed.source, parsed, position, environment)
}

pub(crate) fn complete_source_with_parsed_environment(
    source: &str,
    parsed: &ParsedSource,
    position: SourcePosition,
    environment: &TypeEnvironment,
) -> Vec<Completion> {
    let Some(context) = completion_context(source, position) else {
        return Vec::new();
    };

    complete_parsed_source_with_context(parsed, source, context, environment)
}

pub(crate) fn complete_field_access_in_source_with_environment(
    source: &str,
    parsed: &ParsedSource,
    position: SourcePosition,
    environment: &TypeEnvironment,
) -> Vec<Completion> {
    let Some(CompletionContext::FieldAccess {
        receiver_parts,
        prefix,
        position,
        separator,
        separator_span,
    }) = completion_context(source, position)
    else {
        return Vec::new();
    };

    complete_fields(
        parsed,
        source,
        environment,
        &receiver_parts,
        prefix.as_str(),
        position,
        separator,
        separator_span,
    )
}

fn complete_parsed_source_with_context(
    parsed: &ParsedSource,
    source: &str,
    context: CompletionContext,
    environment: &TypeEnvironment,
) -> Vec<Completion> {
    let prefix = context.filter_prefix().map(str::to_owned);
    let completions = match context {
        CompletionContext::AnnotationTag { prefix } => complete_annotation_tags(prefix.as_str()),
        CompletionContext::DocumentationSnippet { position } => {
            complete_documentation_snippet(parsed, position)
        }
        CompletionContext::TypeAnnotation { prefix } => {
            complete_type_names(environment, prefix.as_str())
        }
        CompletionContext::FieldAccess {
            receiver_parts,
            prefix,
            position,
            separator,
            separator_span,
        } => {
            let module_fields =
                complete_required_module_fields(parsed, &receiver_parts, prefix.as_str());

            if !module_fields.is_empty() {
                module_fields
            } else {
                let fields = complete_fields(
                    parsed,
                    source,
                    environment,
                    &receiver_parts,
                    prefix.as_str(),
                    position,
                    separator,
                    separator_span,
                );

                if !fields.is_empty() || environment.local(&receiver_parts[0]).is_some() {
                    fields
                } else {
                    complete_lua_fields(source, &receiver_parts, prefix.as_str(), position)
                }
            }
        }
        CompletionContext::StringArgument {
            callee_parts,
            active_parameter,
            prefix,
            position,
        } => complete_string_argument_values(
            parsed,
            environment,
            &callee_parts,
            active_parameter,
            prefix.as_str(),
            position,
        ),
        CompletionContext::RequirePath { prefix } => {
            complete_require_paths(&parsed.path, prefix.as_str())
        }
        CompletionContext::ImportPath { prefix } => {
            complete_import_paths(&parsed.path, prefix.as_str())
        }
        CompletionContext::Identifier { prefix, position } => {
            complete_identifier_items(parsed, prefix.as_str(), position)
        }
    };

    rank_completions_for_prefix(completions, prefix.as_deref())
}

fn complete_lossy_source(path: &Path, source: &str, context: CompletionContext) -> Vec<Completion> {
    let prefix = context.filter_prefix().map(str::to_owned);
    let completions = match context {
        CompletionContext::AnnotationTag { prefix } => complete_annotation_tags(prefix.as_str()),
        CompletionContext::DocumentationSnippet { position } => {
            complete_lossy_documentation_snippet(source, position)
        }
        CompletionContext::TypeAnnotation { prefix } => complete_builtin_types(prefix.as_str()),
        CompletionContext::FieldAccess {
            receiver_parts,
            prefix,
            position,
            ..
        } => complete_lua_fields(source, &receiver_parts, prefix.as_str(), position),
        CompletionContext::StringArgument { .. } => Vec::new(),
        CompletionContext::RequirePath { prefix } => complete_require_paths(path, prefix.as_str()),
        CompletionContext::ImportPath { prefix } => complete_import_paths(path, prefix.as_str()),
        CompletionContext::Identifier { prefix, position } => {
            complete_lossy_identifier_items(source, prefix.as_str(), position)
        }
    };

    rank_completions_for_prefix(completions, prefix.as_deref())
}

fn rank_completions_for_prefix(
    completions: Vec<Completion>,
    prefix: Option<&str>,
) -> Vec<Completion> {
    match prefix {
        Some(prefix) => filter_and_rank_completions(completions, prefix),
        None => completions,
    }
}

fn complete_annotation_tags(prefix: &str) -> Vec<Completion> {
    [
        ("alias", "type alias annotation"),
        ("class", "class annotation"),
        ("field", "field annotation"),
        ("generic", "generic annotation"),
        ("overload", "function overload annotation"),
        ("param", "function parameter annotation"),
        ("return", "function return annotation"),
        ("type", "type annotation"),
        ("vararg", "function vararg annotation"),
        ("version", "version annotation"),
    ]
    .into_iter()
    .filter(|(tag, _)| completion_text_matches(tag, prefix))
    .map(|(tag, detail)| Completion {
        label: tag.to_owned(),
        kind: CompletionKind::Keyword,
        detail: Some(detail.to_owned()),
        documentation: None,
        insert_text: None,
        filter_text: None,
        is_snippet: false,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    })
    .collect()
}

fn complete_type_names(environment: &TypeEnvironment, prefix: &str) -> Vec<Completion> {
    let mut completions = BTreeMap::new();

    for completion in complete_type_aliases(environment, prefix) {
        completions.insert(completion.label.clone(), completion);
    }

    for completion in complete_builtin_types(prefix) {
        completions.insert(completion.label.clone(), completion);
    }

    completions.into_values().collect()
}

fn complete_builtin_types(prefix: &str) -> Vec<Completion> {
    ["any", "boolean", "nil", "number", "string"]
        .into_iter()
        .chain(asset::builtin_type_names().iter().copied())
        .chain(lua::love_type_names().iter().copied())
        .chain(vector::builtin_type_names().iter().copied())
        .filter(|primitive| completion_text_matches(primitive, prefix))
        .map(|primitive| Completion {
            label: primitive.to_owned(),
            kind: CompletionKind::TypeAlias,
            detail: Some("built-in type".to_owned()),
            documentation: None,
            insert_text: None,
            filter_text: None,
            is_snippet: false,
            module_path: None,
            source_module: None,
            additional_text_edits: Vec::new(),
        })
        .collect()
}

fn complete_type_aliases(environment: &TypeEnvironment, prefix: &str) -> Vec<Completion> {
    environment
        .aliases()
        .filter(|(name, _)| completion_text_matches(name, prefix))
        .map(|(name, _)| Completion {
            label: name.to_owned(),
            kind: CompletionKind::TypeAlias,
            detail: Some("type alias".to_owned()),
            documentation: environment.alias_documentation(name).map(str::to_owned),
            insert_text: None,
            filter_text: None,
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
        filter_text: None,
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
        filter_text: None,
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
                    let marker = if field.optional { "?:" } else { ":" };

                    format!("{}{marker} {}", field.name, format_tua_type(&field.ty))
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
            let parameters = parameters
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
                format!("fun({parameters})")
            } else {
                format!("fun({parameters}): {returns}")
            }
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
        if completion_text_matches(&local.name, prefix) {
            completions.insert(
                local.name.clone(),
                Completion {
                    label: local.name.clone(),
                    kind: CompletionKind::Variable,
                    detail: Some("local".to_owned()),
                    documentation: None,
                    insert_text: None,
                    filter_text: None,
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

        if completion_text_matches(name, prefix) {
            completions.insert(
                name.clone(),
                Completion {
                    label: name.clone(),
                    kind: CompletionKind::Function,
                    detail: Some("function".to_owned()),
                    documentation: None,
                    insert_text: None,
                    filter_text: None,
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
        if completion_text_matches(&symbol.name, prefix) {
            completions.insert(symbol.name.clone(), symbol.into_completion());
        }
    }
}

fn complete_fields(
    parsed: &ParsedSource,
    source: &str,
    environment: &TypeEnvironment,
    receiver_parts: &[String],
    prefix: &str,
    position: SourcePosition,
    separator: FieldAccessSeparator,
    separator_span: crate::SourceSpan,
) -> Vec<Completion> {
    let _profile = profile::span("analysis.completion.receiver");
    let semantic = SemanticModel::new(parsed, environment);
    let Some(receiver_type) = love_conf_receiver_type(parsed, source, receiver_parts, position)
        .or_else(|| semantic.type_of_field_chain(receiver_parts, position))
    else {
        return Vec::new();
    };
    let mut completions = BTreeMap::new();
    let mut fields = BTreeMap::new();

    collect_table_fields(environment, &receiver_type, &mut fields);

    for (name, detail) in fields
        .into_iter()
        .filter(|(name, _)| completion_text_matches(name, prefix))
    {
        completions.insert(
            name.clone(),
            Completion {
                label: name,
                kind: CompletionKind::Field,
                detail: Some(detail),
                documentation: None,
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            },
        );
    }

    {
        let _profile = profile::span("analysis.completion.love_fields");

        collect_love_object_fields(
            parsed,
            environment,
            &receiver_type,
            prefix,
            separator,
            separator_span,
            &mut completions,
        );
    }

    if receiver_parts.len() == 1 && receiver_parts[0] == "love" {
        apply_love_callback_snippets(source, position, &mut completions);
    }

    completions.into_values().collect()
}

fn complete_string_argument_values(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    callee_parts: &[String],
    active_parameter: usize,
    prefix: &str,
    position: SourcePosition,
) -> Vec<Completion> {
    let _profile = profile::span("analysis.completion.string_argument");
    let semantic = SemanticModel::new(parsed, environment);
    if let Some(function) = semantic.function_type_for_call_parts(callee_parts, position) {
        let Some(parameter) = function.parameters.get(active_parameter) else {
            return Vec::new();
        };

        let parameter = resolve_aliases(environment, parameter, 0);

        return complete_lua_enum_values(&parameter.to_string(), prefix);
    }

    let Some(item) = semantic.lua_function_item_for_call_parts(callee_parts, position) else {
        return Vec::new();
    };
    let Some(parameter_type) = parameter_type_from_lua_item(item, active_parameter) else {
        return Vec::new();
    };

    complete_lua_enum_values(&parameter_type, prefix)
}

fn parameter_type_from_lua_item(item: LuaItem, index: usize) -> Option<String> {
    if let Some(signatures) = lua::signature_variants(item) {
        let mut types = Vec::new();

        for signature in signatures {
            let Some(parameter) = lua_signature_parameter_at(signature, index) else {
                continue;
            };
            let Some(ty) = parameter_type_from_label(parameter.label) else {
                continue;
            };

            if !types.iter().any(|existing| existing == &ty) {
                types.push(ty);
            }
        }

        if !types.is_empty() {
            return Some(types.join(" | "));
        }
    }

    parameter_type_from_function_detail(item.detail, index)
}

fn lua_signature_parameter_at(
    signature: &lua::LuaSignature,
    index: usize,
) -> Option<&lua::LuaSignatureParameter> {
    signature.parameters.get(index).or_else(|| {
        signature
            .parameters
            .last()
            .filter(|parameter| parameter.label.starts_with("..."))
    })
}

fn parameter_type_from_function_detail(detail: &str, index: usize) -> Option<String> {
    let start = detail.find('(')? + 1;
    let end = detail[start..].find(')')? + start;
    let parameter = detail[start..end]
        .split(',')
        .map(str::trim)
        .filter(|parameter| !parameter.is_empty())
        .nth(index)?;
    parameter_type_from_label(parameter)
}

fn parameter_type_from_label(parameter: &str) -> Option<String> {
    let (_, ty) = parameter.split_once(':')?;
    Some(ty.trim().to_owned())
}

fn complete_lua_enum_values(type_label: &str, prefix: &str) -> Vec<Completion> {
    let completions = complete_lua_enum_value_candidates(type_label, prefix);

    if !completions.is_empty() || prefix.is_empty() {
        return completions;
    }

    let mut fallback = complete_lua_enum_value_candidates(type_label, "");

    for completion in &mut fallback {
        completion.filter_text = Some(prefix.to_owned());
    }

    fallback
}

fn complete_lua_enum_value_candidates(type_label: &str, prefix: &str) -> Vec<Completion> {
    type_label
        .split('|')
        .map(str::trim)
        .flat_map(|type_name| {
            literal_value_item(type_name, prefix).into_iter().chain(
                lua::enum_value_items(type_name, prefix)
                    .into_iter()
                    .map(completion_from_lua_item),
            )
        })
        .collect()
}

fn literal_value_item(type_name: &str, prefix: &str) -> Option<Completion> {
    let value = quoted_string_literal_value(type_name)?;

    if !completion_text_matches(&value, prefix) {
        return None;
    }

    Some(Completion {
        label: value,
        kind: CompletionKind::Value,
        detail: Some(type_name.to_owned()),
        documentation: None,
        insert_text: None,
        filter_text: None,
        is_snippet: false,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    })
}

fn quoted_string_literal_value(value: &str) -> Option<String> {
    let value = value.trim();
    let quote = value.chars().next()?;

    if !matches!(quote, '\'' | '"') || !value.ends_with(quote) || value.len() < 2 {
        return None;
    }

    Some(value[quote.len_utf8()..value.len() - quote.len_utf8()].to_owned())
}

fn collect_love_object_fields(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    ty: &Type,
    prefix: &str,
    separator: FieldAccessSeparator,
    separator_span: crate::SourceSpan,
    completions: &mut BTreeMap<String, Completion>,
) {
    match resolve_aliases(environment, ty, 0) {
        Type::Alias(name) => {
            for item in lua::type_field_items(&name, prefix) {
                insert_love_object_item(completions, parsed, item, separator, separator_span);
            }
        }
        Type::Union(types) => {
            for ty in types {
                collect_love_object_fields(
                    parsed,
                    environment,
                    &ty,
                    prefix,
                    separator,
                    separator_span,
                    completions,
                );
            }
        }
        Type::Any
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::Table(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => {}
    }
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
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => {}
    }
}

fn collect_shape_fields(table: &TableShape, fields: &mut BTreeMap<String, String>) {
    for (name, field) in table.semantic_fields() {
        let suffix = if field.optional { " optional" } else { "" };

        fields.insert(name, format!("field{suffix}: {}", field.ty));
    }
}

fn complete_lua_fields(
    source: &str,
    receiver_parts: &[String],
    prefix: &str,
    position: SourcePosition,
) -> Vec<Completion> {
    lua::field_items(receiver_parts, prefix)
        .into_iter()
        .map(|item| completion_from_lua_field_item(source, receiver_parts, item, position))
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
            .filter(|(name, _)| completion_text_matches(name, prefix))
            .map(|(name, field)| Completion {
                kind: completion_kind_for_type(&field.ty),
                detail: Some(format!("field: {}", field.ty)),
                label: name,
                documentation: field.documentation,
                insert_text: None,
                filter_text: None,
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
        .filter(|(name, _)| completion_text_matches(name, prefix))
        .map(|(name, ty)| Completion {
            kind: completion_kind_for_type(&ty),
            detail: Some(format!("field: {ty}")),
            label: name,
            documentation: export.documentation.clone(),
            insert_text: None,
            filter_text: None,
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
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => {}
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
        documentation: Some(lua::documentation_with_love_wiki_link(item)),
        insert_text: None,
        filter_text: None,
        is_snippet: false,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    }
}

fn completion_from_lua_field_item(
    source: &str,
    receiver_parts: &[String],
    item: LuaItem,
    position: SourcePosition,
) -> Completion {
    let mut completion = completion_from_lua_item(item);

    if receiver_parts.len() == 1 && receiver_parts[0] == "love" {
        if let Some(snippet) = love_callback_snippet(item.label, source, position) {
            completion.insert_text = Some(snippet);
            completion.is_snippet = true;
        }
    } else if receiver_parts.first().map(String::as_str) == Some("love")
        && item.kind == LuaItemKind::Function
    {
        if let Some(snippet) = function_call_snippet_from_lua_item(item) {
            completion.insert_text = Some(snippet);
            completion.is_snippet = true;
        }
    }

    completion
}

fn apply_love_callback_snippets(
    source: &str,
    position: SourcePosition,
    completions: &mut BTreeMap<String, Completion>,
) {
    for completion in completions.values_mut() {
        if let Some(snippet) = love_callback_snippet(&completion.label, source, position) {
            completion.insert_text = Some(snippet);
            completion.is_snippet = true;
        }
    }
}

fn love_callback_snippet(label: &str, source: &str, position: SourcePosition) -> Option<String> {
    if !field_completion_is_function_declaration(source, position) {
        return None;
    }

    let parameters = match label {
        "conf" => "t",
        "draw" => "",
        "focus" => "focus",
        "keypressed" => "key, scancode, isrepeat",
        "keyreleased" => "key, scancode",
        "load" => "args",
        "mousemoved" => "x, y, dx, dy, istouch",
        "mousepressed" => "x, y, button, istouch, presses",
        "mousereleased" => "x, y, button, istouch, presses",
        "quit" => "",
        "resize" => "width, height",
        "textinput" => "text",
        "update" => "dt",
        "wheelmoved" => "x, y",
        _ => return None,
    };

    Some(format!("{label}({parameters})\n  $0\nend"))
}

fn function_call_snippet_from_lua_item(item: LuaItem) -> Option<String> {
    let signature = parse_function_signature_line(item.detail)?;
    let mut arguments = Vec::new();
    let mut placeholder = 1;

    for parameter in signature.parameters {
        let Some(argument) = function_call_snippet_argument(&parameter, placeholder) else {
            continue;
        };

        arguments.push(argument);
        placeholder += 1;
    }

    if arguments.is_empty() {
        return Some(format!("{}($0)", item.label));
    }

    Some(format!("{}({})$0", item.label, arguments.join(", ")))
}

fn function_call_snippet_argument(parameter: &DocParameter, placeholder: usize) -> Option<String> {
    let name = parameter.name.trim();

    if name.ends_with('?') {
        return None;
    }

    if name == "..." {
        return Some(format!(
            "${{{placeholder}:{}}}",
            escape_snippet_placeholder("...")
        ));
    }

    let name = name.trim_end_matches('?');
    let enum_values = lua_enum_choice_values(&parameter.ty);

    if !enum_values.is_empty() {
        let choices = enum_values
            .into_iter()
            .map(|value| escape_snippet_choice(&value))
            .collect::<Vec<_>>()
            .join(",");

        return Some(format!("\"${{{placeholder}|{choices}|}}\""));
    }

    Some(format!(
        "${{{placeholder}:{}}}",
        escape_snippet_placeholder(name)
    ))
}

fn lua_enum_choice_values(type_label: &str) -> Vec<String> {
    let mut values = Vec::new();

    for type_name in type_label.split('|').map(str::trim) {
        if let Some(value) = quoted_string_literal_value(type_name) {
            values.push(value);
        }

        for item in lua::enum_value_items(type_name, "") {
            values.push(item.label.to_owned());
        }
    }

    values.dedup();
    values
}

fn escape_snippet_placeholder(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('$', "\\$")
        .replace('}', "\\}")
}

fn escape_snippet_choice(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace('|', "\\|")
}

fn field_completion_is_function_declaration(source: &str, position: SourcePosition) -> bool {
    let Some(line) = line_at(source, position.line) else {
        return false;
    };
    let cursor = crate::source_text::byte_for_column(line, position.column).min(line.len());

    line[..cursor].trim_start().starts_with("function ")
}

fn love_conf_receiver_type(
    parsed: &ParsedSource,
    source: &str,
    receiver_parts: &[String],
    position: SourcePosition,
) -> Option<Type> {
    let receiver = receiver_parts.first()?;
    let parameter = love_conf::parameter_name_at(parsed, position)
        .or_else(|| love_conf_parameter_in_source(source, position))?;

    if receiver != &parameter {
        return None;
    }

    field_chain_type(love_conf::config_type(), &receiver_parts[1..])
}

fn field_chain_type(mut ty: Type, fields: &[String]) -> Option<Type> {
    for field in fields {
        ty = match ty {
            Type::Table(table) => table.semantic_field(field)?.ty.clone(),
            Type::Union(types) => {
                let mut resolved = Vec::new();

                for ty in types {
                    if let Some(field_ty) = field_chain_type(ty, std::slice::from_ref(field)) {
                        resolved.push(field_ty);
                    }
                }

                match resolved.len() {
                    0 => return None,
                    1 => resolved.remove(0),
                    _ => Type::union(resolved),
                }
            }
            Type::Any
            | Type::Alias(_)
            | Type::Function(_)
            | Type::Primitive(_)
            | Type::StringLiteral(_)
            | Type::Array(_)
            | Type::Tuple(_)
            | Type::Dictionary { .. } => return None,
        };
    }

    Some(ty)
}

fn love_conf_parameter_in_source(source: &str, position: SourcePosition) -> Option<String> {
    let mut parameter = None;

    for line_number in 1..=position.line {
        let line = line_at(source, line_number)?;
        let trimmed = line.trim_start();

        if let Some(rest) = trimmed.strip_prefix("function love.conf(") {
            parameter = rest
                .split([',', ')'])
                .next()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned);
            continue;
        }

        if parameter.is_some() && trimmed == "end" {
            parameter = None;
        }
    }

    parameter
}

fn insert_lua_item(completions: &mut BTreeMap<String, Completion>, item: LuaItem) {
    completions
        .entry(item.label.to_owned())
        .or_insert_with(|| completion_from_lua_item(item));
}

fn insert_love_object_item(
    completions: &mut BTreeMap<String, Completion>,
    parsed: &ParsedSource,
    item: LuaItem,
    separator: FieldAccessSeparator,
    separator_span: crate::SourceSpan,
) {
    completions.entry(item.label.to_owned()).or_insert_with(|| {
        let mut completion = completion_from_lua_item(item);

        if item.kind == LuaItemKind::Function {
            completion.insert_text = Some(
                function_call_snippet_from_lua_item(item)
                    .unwrap_or_else(|| format!("{}($0)", item.label)),
            );
            completion.is_snippet = true;

            if separator == FieldAccessSeparator::Dot {
                completion.additional_text_edits.push(TextEdit {
                    path: parsed.path.clone(),
                    span: separator_span,
                    new_text: ":".to_owned(),
                });
            }
        }

        completion
    });
}

fn completion_kind_from_lua_item(kind: LuaItemKind) -> CompletionKind {
    match kind {
        LuaItemKind::Function => CompletionKind::Function,
        LuaItemKind::Keyword => CompletionKind::Keyword,
        LuaItemKind::Module => CompletionKind::Module,
        LuaItemKind::Value => CompletionKind::Value,
    }
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
