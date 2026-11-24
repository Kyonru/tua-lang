use crate::ast_utils::string_literal_runtime_bytes;
use crate::types::{FunctionParameter, FunctionType, TableField, Type};
use crate::{
    Diagnostic, DiagnosticCode, DiagnosticSeverity, I18nConfig, ParsedSource, SourcePosition,
    SourceSpan,
};
use full_moon::ast::{
    Call, Expression, Field, FunctionArgs, FunctionCall, Index, LastStmt, Prefix, Suffix,
    TableConstructor, Var,
};
use full_moon::node::Node;
use full_moon::visitors::Visitor;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub(crate) const MODULE_NAME: &str = "I18n";
pub(crate) const RUNTIME_MODULE: &str = "tuai18n";
pub(crate) const KEY_TYPE: &str = "I18nKey";
pub(crate) const LOCALE_TYPE: &str = "I18nLocale";

pub(crate) fn literal_key_from_call(call: &FunctionCall) -> Option<String> {
    if crate::ast_utils::field_chain_before_first_call(call)?.as_slice() != [MODULE_NAME, "t"] {
        return None;
    }
    let first = crate::lua::first_call_argument(call)?;
    let Expression::String(token) = first else {
        return None;
    };
    string_value(token)
}

pub(crate) fn builtin_aliases() -> Vec<(String, Type)> {
    vec![
        (KEY_TYPE.to_owned(), Type::string()),
        (LOCALE_TYPE.to_owned(), Type::string()),
    ]
}

pub(crate) fn builtin_locals() -> Vec<(String, Type)> {
    let translated = Type::union([Type::string(), Type::array(Type::string()), Type::nil()]);
    let args = Type::dictionary(Type::string(), Type::Any);
    let field = |parameters: Vec<(&str, Type)>, required, returns| {
        TableField::required(Type::Function(FunctionType::new_signature(
            parameters
                .into_iter()
                .map(|(name, ty)| FunctionParameter::named(name, ty)),
            required,
            None,
            [returns],
        )))
    };
    vec![(
        MODULE_NAME.to_owned(),
        Type::table([
            (
                "t",
                field(
                    vec![
                        ("key", Type::alias(KEY_TYPE)),
                        ("args", args),
                        ("locale", Type::alias(LOCALE_TYPE)),
                    ],
                    1,
                    translated,
                ),
            ),
            (
                "dialogue",
                field(
                    vec![
                        ("key", Type::alias(KEY_TYPE)),
                        ("source", Type::string()),
                        (
                            "args",
                            Type::dictionary(
                                Type::string(),
                                Type::union([Type::string(), Type::number(), Type::boolean()]),
                            ),
                        ),
                        ("locale", Type::alias(LOCALE_TYPE)),
                    ],
                    4,
                    Type::string(),
                ),
            ),
            (
                "setLocale",
                field(vec![("locale", Type::alias(LOCALE_TYPE))], 1, Type::nil()),
            ),
            ("getLocale", field(vec![], 0, Type::alias(LOCALE_TYPE))),
            (
                "getLocales",
                field(vec![], 0, Type::array(Type::alias(LOCALE_TYPE))),
            ),
            (
                "formatNumber",
                field(vec![("number", Type::number())], 1, Type::string()),
            ),
            (
                "formatPrice",
                field(vec![("amount", Type::number())], 1, Type::string()),
            ),
            (
                "formatDate",
                field(
                    vec![
                        ("pattern", Type::union([Type::string(), Type::nil()])),
                        (
                            "date",
                            Type::union([Type::alias(crate::lua::OS_DATE_TABLE_TYPE), Type::nil()]),
                        ),
                    ],
                    0,
                    Type::string(),
                ),
            ),
        ]),
    )]
}

pub fn runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/tuai18n.lua")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CatalogValue {
    Text(String),
    Dialogue(Vec<String>),
    Plural(BTreeMap<String, String>),
    Dynamic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CatalogEntry {
    pub locale: String,
    pub key: String,
    pub path: PathBuf,
    pub span: SourceSpan,
    pub value: CatalogValue,
    pub placeholders: BTreeSet<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CatalogIndex {
    pub entries: BTreeMap<String, BTreeMap<String, CatalogEntry>>,
    pub diagnostics: BTreeMap<PathBuf, Vec<Diagnostic>>,
    pub duplicate_keys: BTreeSet<String>,
    pub dialogue_sources: BTreeMap<String, CatalogEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalizationCell {
    pub locale: String,
    pub status: &'static str,
    pub preview: Option<String>,
    pub forms: Vec<(String, String)>,
    pub placeholders: Vec<String>,
    pub path: Option<PathBuf>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalizationRow {
    pub key: String,
    pub source: Option<LocalizationCell>,
    pub cells: Vec<LocalizationCell>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalizationPage {
    pub enabled: bool,
    pub locales: Vec<String>,
    pub total: usize,
    pub rows: Vec<LocalizationRow>,
}

mod dialogue;

impl CatalogIndex {
    /// Return only result types that are safe for every configured locale.
    /// Dynamic or mixed-shape entries keep the public gradual return type.
    pub(crate) fn guaranteed_result_types(&self, config: &I18nConfig) -> BTreeMap<String, Type> {
        self.entries
            .iter()
            .filter_map(|(key, entries)| {
                let fallback = entries.get(&config.fallback_locale)?;
                let (expected, is_dialogue) = match fallback.value {
                    CatalogValue::Text(_) | CatalogValue::Plural(_) => (Type::string(), false),
                    CatalogValue::Dialogue(_) => (Type::array(Type::string()), true),
                    CatalogValue::Dynamic => return None,
                };
                let consistent = entries.values().all(|entry| {
                    if is_dialogue {
                        matches!(entry.value, CatalogValue::Dialogue(_))
                    } else {
                        matches!(entry.value, CatalogValue::Text(_) | CatalogValue::Plural(_))
                    }
                });
                consistent.then(|| (key.clone(), expected))
            })
            .collect()
    }

    pub(crate) fn add_catalog(
        &mut self,
        locale: &str,
        path: PathBuf,
        parsed: Option<&ParsedSource>,
    ) {
        let Some(parsed) = parsed else {
            self.diagnostics.entry(path.clone()).or_default().push(
                Diagnostic::error(
                    &path,
                    1,
                    1,
                    "configured i18n catalog is missing or cannot be parsed",
                )
                .with_code(DiagnosticCode::DOMAIN_I18N),
            );
            return;
        };
        if parsed.source.len() > 1_048_576 {
            self.diagnostics.entry(path.clone()).or_default().push(
                Diagnostic::error(
                    &path,
                    1,
                    1,
                    "i18n catalog exceeds the 1 MiB static-index limit",
                )
                .with_code(DiagnosticCode::DOMAIN_I18N),
            );
            return;
        }
        let Some(LastStmt::Return(statement)) = parsed.ast.nodes().last_stmt() else {
            self.bad_shape(&path, "i18n catalog must return a literal table");
            return;
        };
        let Some(Expression::TableConstructor(root)) = statement.returns().iter().next() else {
            self.bad_shape(&path, "i18n catalog must return a literal table");
            return;
        };
        let mut found_root = false;
        for field in root.fields().iter() {
            let Some((name, _, value)) = field_key(field) else {
                continue;
            };
            if name == locale {
                found_root = true;
                if let Expression::TableConstructor(table) = value {
                    self.walk_table(locale, &path, "", table, 0);
                } else {
                    self.bad_shape(&path, "i18n locale root must be a literal table");
                }
            }
        }
        if !found_root {
            self.bad_shape(&path, format!("i18n catalog root must contain `{locale}`"));
        }
    }

    fn bad_shape(&mut self, path: &PathBuf, message: impl Into<String>) {
        self.diagnostics
            .entry(path.clone())
            .or_default()
            .push(Diagnostic::error(path, 1, 1, message).with_code(DiagnosticCode::DOMAIN_I18N));
    }

    fn walk_table(
        &mut self,
        locale: &str,
        path: &PathBuf,
        prefix: &str,
        table: &TableConstructor,
        depth: usize,
    ) {
        if depth >= 16 || self.entries.len() >= 32_768 {
            let diagnostics = self.diagnostics.entry(path.clone()).or_default();
            if !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("static-index limit"))
            {
                diagnostics.push(Diagnostic::new(path, 1, 1, DiagnosticSeverity::Warning,
                    "i18n catalog reached the static-index limit; remaining entries are available only at game runtime")
                    .with_code(DiagnosticCode::DOMAIN_I18N));
            }
            return;
        }
        for field in table.fields().iter() {
            let Some((name, span, value)) = field_key(field) else {
                continue;
            };
            if name == "_formats" {
                continue;
            }
            let key = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}.{name}")
            };
            match value {
                Expression::TableConstructor(child) if plural_table(child) => {
                    let mut forms = BTreeMap::new();
                    let mut all_literal = true;
                    let mut has_other = false;
                    for form in child.fields().iter() {
                        if let Some((category, _, value)) = field_key(form) {
                            has_other |= category == "other";
                            if let Expression::String(text) = value {
                                if let Some(text) = string_value(text) {
                                    forms.insert(category, text);
                                    continue;
                                }
                            }
                            all_literal = false;
                        }
                    }
                    if !has_other {
                        self.diagnostics.entry(path.clone()).or_default().push(
                            Diagnostic::error(
                                path,
                                span.start.line,
                                span.start.column,
                                format!("plural key `{key}` needs an `other` form"),
                            )
                            .with_code(DiagnosticCode::DOMAIN_I18N),
                        );
                    }
                    self.add_entry(
                        locale,
                        path,
                        key,
                        span,
                        if all_literal && has_other {
                            CatalogValue::Plural(forms)
                        } else {
                            CatalogValue::Dynamic
                        },
                    );
                }
                Expression::TableConstructor(child) if dialogue_table(child) => {
                    let values = child
                        .fields()
                        .iter()
                        .map(|field| match field {
                            Field::NoKey(Expression::String(text)) => string_value(text),
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>();
                    self.add_entry(
                        locale,
                        path,
                        key,
                        span,
                        values.map_or(CatalogValue::Dynamic, CatalogValue::Dialogue),
                    );
                }
                Expression::TableConstructor(child) if child.fields().is_empty() => {
                    self.add_entry(locale, path, key, span, CatalogValue::Dynamic);
                }
                Expression::TableConstructor(child) => {
                    self.walk_table(locale, path, &key, child, depth + 1)
                }
                Expression::String(text) => self.add_entry(
                    locale,
                    path,
                    key,
                    span,
                    string_value(text)
                        .map(CatalogValue::Text)
                        .unwrap_or(CatalogValue::Dynamic),
                ),
                _ => self.add_entry(locale, path, key, span, CatalogValue::Dynamic),
            }
        }
    }

    fn add_entry(
        &mut self,
        locale: &str,
        path: &PathBuf,
        key: String,
        span: SourceSpan,
        value: CatalogValue,
    ) {
        let placeholders = match &value {
            CatalogValue::Text(text) => placeholders(text),
            CatalogValue::Dialogue(lines) => {
                lines.iter().flat_map(|line| placeholders(line)).collect()
            }
            CatalogValue::Plural(forms) => forms
                .values()
                .flat_map(|text| placeholders(text))
                .chain(["count".to_owned()])
                .collect(),
            CatalogValue::Dynamic => BTreeSet::new(),
        };
        let entry = CatalogEntry {
            locale: locale.to_owned(),
            key: key.clone(),
            path: path.clone(),
            span,
            value,
            placeholders,
        };
        if self
            .entries
            .entry(key.clone())
            .or_default()
            .insert(locale.to_owned(), entry)
            .is_some()
        {
            self.duplicate_keys.insert(key.clone());
            self.diagnostics.entry(path.clone()).or_default().push(
                Diagnostic::error(
                    path,
                    span.start.line,
                    span.start.column,
                    format!("duplicate i18n key `{key}`"),
                )
                .with_code(DiagnosticCode::DOMAIN_I18N),
            );
        }
    }

    pub(crate) fn finish(&mut self, config: &I18nConfig) {
        let fallback_locale = &config.fallback_locale;
        let mut missing_by_locale = BTreeMap::<String, usize>::new();
        for (key, by_locale) in &self.entries {
            let Some(fallback) = by_locale.get(fallback_locale) else {
                if let Some(entry) = by_locale.values().next() {
                    self.diagnostics.entry(entry.path.clone()).or_default().push(
                        Diagnostic::new(&entry.path, entry.span.start.line, entry.span.start.column,
                            if self.dialogue_sources.contains_key(key) { DiagnosticSeverity::Warning } else { DiagnosticSeverity::Error },
                            format!("i18n key `{key}` is missing from fallback locale `{fallback_locale}`"))
                            .with_code(DiagnosticCode::DOMAIN_I18N),
                    );
                }
                continue;
            };
            for entry in by_locale.values() {
                if entry.locale != *fallback_locale
                    && !self.dialogue_sources.contains_key(key)
                    && entry.placeholders != fallback.placeholders
                    && !matches!(entry.value, CatalogValue::Dynamic)
                    && !matches!(fallback.value, CatalogValue::Dynamic)
                {
                    self.diagnostics.entry(entry.path.clone()).or_default().push(
                        Diagnostic::error(&entry.path, entry.span.start.line, entry.span.start.column,
                            format!("i18n key `{key}` uses placeholders {:?}; fallback `{fallback_locale}` uses {:?}", entry.placeholders, fallback.placeholders))
                            .with_code(DiagnosticCode::DOMAIN_I18N),
                    );
                }
            }
            for catalog in &config.catalogs {
                if catalog.locale != *fallback_locale && !by_locale.contains_key(&catalog.locale) {
                    *missing_by_locale.entry(catalog.locale.clone()).or_default() += 1;
                }
            }
        }
        for catalog in &config.catalogs {
            let Some(missing) = missing_by_locale.get(&catalog.locale) else {
                continue;
            };
            let path = self
                .entries
                .values()
                .filter_map(|entries| entries.get(&catalog.locale))
                .next()
                .map(|entry| entry.path.clone());
            if let Some(path) = path {
                self.diagnostics.entry(path.clone()).or_default().push(
                    Diagnostic::new(
                        &path,
                        1,
                        1,
                        DiagnosticSeverity::Warning,
                        format!(
                            "i18n catalog `{}` is missing {missing} key{} from fallback `{fallback_locale}`; open the Localization Explorer for details",
                            catalog.locale,
                            if *missing == 1 { "" } else { "s" }
                        ),
                    )
                    .with_code(DiagnosticCode::DOMAIN_I18N),
                );
            }
        }
    }

    pub(crate) fn resolved_entry(
        &self,
        key: &str,
        selected: &str,
        fallback: &str,
    ) -> Option<&CatalogEntry> {
        let by_locale = self.entries.get(key)?;
        fn ancestor<'a>(
            entries: &'a BTreeMap<String, CatalogEntry>,
            locale: &str,
        ) -> Option<&'a CatalogEntry> {
            let mut candidate = locale;
            loop {
                if let Some(entry) = entries.get(candidate) {
                    return Some(entry);
                }
                candidate = candidate.rsplit_once('-')?.0;
            }
        }
        ancestor(by_locale, selected).or_else(|| ancestor(by_locale, fallback))
    }

    pub(crate) fn entry_at(
        &self,
        path: &std::path::Path,
        position: SourcePosition,
    ) -> Option<&CatalogEntry> {
        self.entries
            .values()
            .flat_map(BTreeMap::values)
            .find(|entry| entry.path == path && contains(entry.span, position))
    }
}

/// Prepare a stub only when every parent table already exists literally in
/// the configured fallback catalog. Dynamic parents or missing paths are not
/// safe destinations for an automatic edit.
pub(crate) fn missing_key_stub(
    parsed: &ParsedSource,
    fallback_locale: &str,
    key: &str,
) -> Option<crate::TextEdit> {
    let segments = key.split('.').collect::<Vec<_>>();
    if segments.is_empty()
        || !segments
            .iter()
            .all(|part| crate::is_valid_rename_identifier(part))
    {
        return None;
    }
    let LastStmt::Return(statement) = parsed.ast.nodes().last_stmt()? else {
        return None;
    };
    let Expression::TableConstructor(root_table) = statement.returns().iter().next()? else {
        return None;
    };
    let mut table = root_table;
    for segment in
        std::iter::once(fallback_locale).chain(segments.iter().copied().take(segments.len() - 1))
    {
        let value = table.fields().iter().find_map(|field| {
            let (name, _, value) = field_key(field)?;
            (name == segment).then_some(value)
        })?;
        let Expression::TableConstructor(next) = value else {
            return None;
        };
        table = next;
    }
    let leaf = segments.last()?;
    if table
        .fields()
        .iter()
        .any(|field| field_key(field).is_some_and(|(name, _, _)| name == *leaf))
    {
        return None;
    }
    let (_, end) = table.range()?;
    let end = SourcePosition::from_full_moon(end);
    let closing = crate::source_text::byte_for_position(&parsed.source, end)?.checked_sub(1)?;
    (parsed.source.as_bytes().get(closing) == Some(&b'}')).then_some(())?;
    let previous = parsed.source[..closing]
        .trim_end()
        .as_bytes()
        .last()
        .copied();
    let separator = if matches!(previous, Some(b'{' | b',' | b';')) {
        ""
    } else {
        ","
    };
    let position = crate::source_text::SourcePositionIndex::new(&parsed.source).position(closing);
    Some(crate::TextEdit {
        path: parsed.path.clone(),
        span: SourceSpan {
            start: position,
            end: position,
        },
        new_text: format!("{separator} {leaf} = \"TODO\" "),
    })
}

fn contains(span: SourceSpan, position: SourcePosition) -> bool {
    (span.start.line, span.start.column) <= (position.line, position.column)
        && (position.line, position.column) <= (span.end.line, span.end.column)
}

fn field_key(field: &Field) -> Option<(String, SourceSpan, &Expression)> {
    match field {
        Field::NameKey { key, value, .. } => {
            let (start, end) = key.range()?;
            Some((
                key.token().to_string().trim().to_owned(),
                SourceSpan::from_range(start, end),
                value,
            ))
        }
        Field::ExpressionKey {
            key: Expression::String(text),
            value,
            ..
        } => {
            let (start, end) = text.range()?;
            Some((
                string_value(text)?,
                SourceSpan::from_range(start, end),
                value,
            ))
        }
        _ => None,
    }
}

fn string_value(token: &full_moon::tokenizer::TokenReference) -> Option<String> {
    String::from_utf8(string_literal_runtime_bytes(token)?).ok()
}

fn plural_table(table: &TableConstructor) -> bool {
    let mut any = false;
    for field in table.fields().iter() {
        let Some((name, _, _)) = field_key(field) else {
            return false;
        };
        if !matches!(
            name.as_str(),
            "zero" | "one" | "two" | "few" | "many" | "other"
        ) {
            return false;
        }
        any = true;
    }
    any
}

fn dialogue_table(table: &TableConstructor) -> bool {
    !table.fields().is_empty()
        && table
            .fields()
            .iter()
            .all(|field| matches!(field, Field::NoKey(_)))
}

pub(crate) fn placeholders(text: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for marker in ["%{", "%<"] {
        let mut rest = text;
        while let Some(start) = rest.find(marker) {
            let after = &rest[start + marker.len()..];
            let end_char = if marker == "%{" { '}' } else { '>' };
            let Some(end) = after.find(end_char) else {
                break;
            };
            if start == 0 || !rest[..start].ends_with('%') {
                let name = after[..end].trim();
                if !name.is_empty() {
                    result.insert(name.to_owned());
                }
            }
            rest = &after[end + 1..];
        }
    }
    result
}

/// A bounded editing context for the second argument of a literal `I18n.t`
/// call. Parsing can fail while the table is being typed, so this uses the
/// existing Lua-aware call scanner and accepts only an unambiguous table head.
pub(crate) fn argument_field_context(
    source: &str,
    position: SourcePosition,
) -> Option<(String, String, BTreeSet<String>)> {
    let cursor = crate::source_text::byte_for_position(source, position)?;
    let open = crate::completion::context::active_call_open(source, cursor)?;
    if crate::completion::context::callee_parts_before(source, open)? != [MODULE_NAME, "t"]
        || crate::completion::context::active_parameter_index(source, open + 1, cursor) != 1
    {
        return None;
    }
    let before = source.get(open + 1..cursor)?.trim_start();
    let quote = before.chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let mut end = None;
    let mut escaped = false;
    for (offset, character) in before[quote.len_utf8()..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == quote {
            end = Some(quote.len_utf8() + offset);
            break;
        }
    }
    let end = end?;
    let key = before.get(quote.len_utf8()..end)?.to_owned();
    let after_key = before.get(end + quote.len_utf8()..)?.trim_start();
    let after_comma = after_key.strip_prefix(',')?.trim_start();
    let fields = after_comma.strip_prefix('{')?;
    if fields.contains(['{', '}']) {
        return None;
    }
    let mut used = BTreeSet::new();
    let mut segments = fields.split(',').collect::<Vec<_>>();
    let active = segments.pop()?.trim_start();
    for segment in segments {
        if let Some((name, _)) = segment.split_once('=') {
            used.insert(name.trim().to_owned());
        }
    }
    if !active
        .chars()
        .all(|c| c == '_' || c.is_ascii_alphanumeric())
    {
        return None;
    }
    Some((key, active.to_owned(), used))
}

pub(crate) fn function_accepts_key(parsed: &ParsedSource, name: &str, parameter: usize) -> bool {
    if parsed
        .type_aliases
        .iter()
        .any(|alias| alias.name == KEY_TYPE)
    {
        return false;
    }
    parsed.function_annotations.iter().any(|function| {
        function
            .name
            .as_deref()
            .is_some_and(|candidate| candidate == name || candidate.ends_with(&format!(".{name}")))
            && function
                .signature_parameters
                .get(parameter)
                .and_then(|parameter| parameter.ty.as_ref())
                .is_some_and(|ty| matches!(ty, crate::TuaType::Named(alias) if alias == KEY_TYPE))
    })
}

pub(crate) fn imported_wrapper(
    parsed: &ParsedSource,
    callee_parts: &[String],
) -> Option<(String, String)> {
    if callee_parts.len() == 2 {
        let binding = crate::module_index::require_bindings_for_parsed_source(parsed)
            .into_iter()
            .find(|binding| binding.alias == callee_parts[0])?;
        return Some((binding.module, callee_parts[1].clone()));
    }
    let [callee] = callee_parts else {
        return None;
    };
    for stmt in parsed.ast.nodes().stmts() {
        let full_moon::ast::Stmt::LocalAssignment(local) = stmt else {
            continue;
        };
        for (name, expression) in local.names().iter().zip(local.expressions()) {
            if name.token().to_string().trim() != callee {
                continue;
            }
            let (prefix, suffixes) = match expression {
                Expression::FunctionCall(call) => {
                    (call.prefix(), call.suffixes().collect::<Vec<_>>())
                }
                Expression::Var(Var::Expression(var)) => {
                    (var.prefix(), var.suffixes().collect::<Vec<_>>())
                }
                _ => continue,
            };
            let Prefix::Name(receiver) = prefix else {
                continue;
            };
            if !matches!(receiver.token().to_string().trim(), "require" | "import") {
                continue;
            }
            let mut suffixes = suffixes.into_iter();
            let Some(Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses {
                arguments,
                ..
            }))) = suffixes.next()
            else {
                continue;
            };
            let Some(Expression::String(module)) = arguments.iter().next() else {
                continue;
            };
            let Some(Suffix::Index(Index::Dot { name: field, .. })) = suffixes.next() else {
                continue;
            };
            if suffixes.next().is_some() {
                continue;
            }
            let module = string_value(module)?;
            return Some((module, field.token().to_string().trim().to_owned()));
        }
    }
    None
}

#[derive(Clone, Debug)]
pub(crate) struct KeyUse {
    pub key: String,
    pub span: SourceSpan,
    pub locale: Option<String>,
    /// `None` means the caller passed an expression whose fields are unknown.
    pub argument_fields: Option<BTreeSet<String>>,
}

#[derive(Clone, Debug)]
pub(crate) struct WrapperKeyUse {
    pub usage: KeyUse,
    pub callee_parts: Vec<String>,
}

pub(crate) fn wrapper_key_uses(parsed: &ParsedSource) -> Vec<WrapperKeyUse> {
    #[derive(Default)]
    struct Collector(Vec<WrapperKeyUse>);
    impl Visitor for Collector {
        fn visit_function_call(&mut self, call: &FunctionCall) {
            let Some(callee_parts) = crate::ast_utils::field_chain_before_first_call(call) else {
                return;
            };
            if callee_parts == [MODULE_NAME, "t"] {
                return;
            }
            let Some(Expression::String(token)) = crate::lua::first_call_argument(call) else {
                return;
            };
            let (Some(key), Some((start, end))) = (string_value(token), token.range()) else {
                return;
            };
            self.0.push(WrapperKeyUse {
                usage: KeyUse {
                    key,
                    span: SourceSpan::from_range(start, end),
                    locale: None,
                    argument_fields: None,
                },
                callee_parts,
            });
        }
    }
    let mut collector = Collector::default();
    collector.visit_ast(&parsed.ast);
    collector.0
}

#[derive(Clone, Debug)]
pub(crate) struct LocaleUse {
    pub locale: String,
    pub span: SourceSpan,
}

pub(crate) fn locale_uses(parsed: &ParsedSource) -> Vec<LocaleUse> {
    #[derive(Default)]
    struct Collector(Vec<(LocaleUse, SourcePosition)>);
    impl Visitor for Collector {
        fn visit_function_call(&mut self, call: &FunctionCall) {
            let Prefix::Name(receiver) = call.prefix() else {
                return;
            };
            if receiver.token().to_string().trim() != MODULE_NAME {
                return;
            }
            let mut suffixes = call.suffixes();
            let Some(Suffix::Index(Index::Dot { name, .. })) = suffixes.next() else {
                return;
            };
            let argument_index = match name.token().to_string().trim() {
                "setLocale" => 0,
                "t" => 2,
                _ => return,
            };
            let Some(Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses {
                arguments,
                ..
            }))) = suffixes.next()
            else {
                return;
            };
            let Some(Expression::String(token)) = arguments.iter().nth(argument_index) else {
                return;
            };
            let (Some(locale), Some((start, end)), Some(position)) = (
                string_value(token),
                token.range(),
                receiver.start_position(),
            ) else {
                return;
            };
            self.0.push((
                LocaleUse {
                    locale,
                    span: SourceSpan::from_range(start, end),
                },
                SourcePosition::from_full_moon(position),
            ));
        }
    }
    let mut collector = Collector::default();
    collector.visit_ast(&parsed.ast);
    let scope = crate::scope_map_parsed_source(parsed);
    collector
        .0
        .into_iter()
        .filter_map(|(usage, receiver)| {
            (!scope.has_non_builtin_binding_at(MODULE_NAME, receiver)).then_some(usage)
        })
        .collect()
}

pub(crate) fn key_uses(parsed: &ParsedSource) -> Vec<KeyUse> {
    #[derive(Default)]
    struct Collector {
        uses: Vec<(KeyUse, SourcePosition)>,
    }
    impl Visitor for Collector {
        fn visit_function_call(&mut self, call: &FunctionCall) {
            let Prefix::Name(receiver) = call.prefix() else {
                return;
            };
            if receiver.token().to_string().trim() != MODULE_NAME {
                return;
            }
            let mut suffixes = call.suffixes();
            let Some(Suffix::Index(Index::Dot { name, .. })) = suffixes.next() else {
                return;
            };
            if name.token().to_string().trim() != "t" {
                return;
            }
            let Some(Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses {
                arguments,
                ..
            }))) = suffixes.next()
            else {
                return;
            };
            let Some(Expression::String(token)) = arguments.iter().next() else {
                return;
            };
            let Some(key) = string_value(token) else {
                return;
            };
            let Some((start, end)) = token.range() else {
                return;
            };
            let locale = arguments.iter().nth(2).and_then(|arg| match arg {
                Expression::String(token) => string_value(token),
                _ => None,
            });
            let argument_fields = match arguments.iter().nth(1) {
                None => Some(BTreeSet::new()),
                Some(Expression::TableConstructor(table)) => Some(
                    table
                        .fields()
                        .iter()
                        .filter_map(|field| field_key(field).map(|(name, _, _)| name))
                        .collect(),
                ),
                Some(_) => None,
            };
            let Some(receiver_position) = receiver.start_position() else {
                return;
            };
            self.uses.push((
                KeyUse {
                    key,
                    span: SourceSpan::from_range(start, end),
                    locale,
                    argument_fields,
                },
                SourcePosition::from_full_moon(receiver_position),
            ));
        }
    }
    let mut collector = Collector::default();
    collector.visit_ast(&parsed.ast);
    let scope = crate::scope_map_parsed_source(parsed);
    collector
        .uses
        .into_iter()
        .filter_map(|(usage, receiver)| {
            (!scope.has_non_builtin_binding_at(MODULE_NAME, receiver)).then_some(usage)
        })
        .collect()
}

pub(crate) fn use_diagnostics(
    parsed: &ParsedSource,
    uses: &[KeyUse],
    locales: &[LocaleUse],
    index: &CatalogIndex,
    config: &I18nConfig,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for usage in locales {
        if !locale_has_catalog_or_parent(&usage.locale, config) {
            diagnostics.push(
                Diagnostic::new(
                    &parsed.path,
                    usage.span.start.line,
                    usage.span.start.column,
                    DiagnosticSeverity::Warning,
                    format!("i18n locale `{}` has no configured catalog", usage.locale),
                )
                .with_code(DiagnosticCode::DOMAIN_I18N)
                .with_range(
                    usage.span.start.line,
                    usage.span.start.column,
                    usage.span.end.line,
                    usage.span.end.column,
                ),
            );
        }
    }
    for usage in uses {
        let at = usage.span.start;
        if !index.entries.contains_key(&usage.key) {
            diagnostics.push(
                Diagnostic::error(
                    &parsed.path,
                    at.line,
                    at.column,
                    format!("unknown i18n key `{}`", usage.key),
                )
                .with_code(DiagnosticCode::DOMAIN_I18N)
                .with_range(
                    at.line,
                    at.column,
                    usage.span.end.line,
                    usage.span.end.column,
                ),
            );
            continue;
        }
        if let Some(fields) = &usage.argument_fields {
            let required = index.entries[&usage.key]
                .get(&config.fallback_locale)
                .or_else(|| index.entries[&usage.key].values().next());
            if let Some(entry) = required {
                if !matches!(entry.value, CatalogValue::Dynamic) {
                    let missing = entry
                        .placeholders
                        .difference(fields)
                        .cloned()
                        .collect::<Vec<_>>();
                    if !missing.is_empty() {
                        diagnostics.push(
                            Diagnostic::error(
                                &parsed.path,
                                at.line,
                                at.column,
                                format!(
                                    "i18n key `{}` needs argument{} {}",
                                    usage.key,
                                    if missing.len() == 1 { "" } else { "s" },
                                    missing
                                        .iter()
                                        .map(|name| format!("`{name}`"))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                ),
                            )
                            .with_code(DiagnosticCode::DOMAIN_I18N)
                            .with_range(
                                at.line,
                                at.column,
                                usage.span.end.line,
                                usage.span.end.column,
                            ),
                        );
                    }
                }
            }
        }
    }
    diagnostics
}

fn locale_has_catalog_or_parent(locale: &str, config: &I18nConfig) -> bool {
    let mut candidate = locale;
    loop {
        if config
            .catalogs
            .iter()
            .any(|catalog| catalog.locale == candidate)
        {
            return true;
        }
        let Some((parent, _)) = candidate.rsplit_once('-') else {
            return false;
        };
        candidate = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_literal_keys_plural_forms_and_dialogue_without_executing_lua() {
        let source = r#"return { en = {
  menu = { play = "Play %{name}" },
  coins = { one = "%{count} coin", other = "%{count} coins" },
  dialogue = { "Hello", "Goodbye" },
  partial = { "Hello", tostring(42) },
  empty = {},
  calculated = os.execute("echo should-not-run"),
} }"#;
        let parsed = crate::parse_lua_source("src/locales/en.lua", source).unwrap();
        let mut index = CatalogIndex::default();
        index.add_catalog("en", PathBuf::from("src/locales/en.lua"), Some(&parsed));
        assert!(matches!(
            index.entries["menu.play"]["en"].value,
            CatalogValue::Text(_)
        ));
        assert_eq!(
            index.entries["menu.play"]["en"].placeholders,
            BTreeSet::from(["name".to_owned()])
        );
        assert!(matches!(
            index.entries["coins"]["en"].value,
            CatalogValue::Plural(_)
        ));
        assert!(matches!(
            index.entries["dialogue"]["en"].value,
            CatalogValue::Dialogue(_)
        ));
        assert!(matches!(
            index.entries["partial"]["en"].value,
            CatalogValue::Dynamic
        ));
        assert!(matches!(
            index.entries["empty"]["en"].value,
            CatalogValue::Dynamic
        ));
        assert!(matches!(
            index.entries["calculated"]["en"].value,
            CatalogValue::Dynamic
        ));
        assert!(!index.entries.contains_key("coins.one"));
    }

    #[test]
    fn catalog_resolution_uses_every_locale_ancestor_before_fallback() {
        let mut index = CatalogIndex::default();
        for (locale, text) in [("en", "Default"), ("zh", "Chinese")] {
            let path = PathBuf::from(format!("src/locales/{locale}.lua"));
            let source = format!("return {{ {locale} = {{ title = '{text}' }} }}");
            let parsed = crate::parse_lua_source(&path, &source).unwrap();
            index.add_catalog(locale, path, Some(&parsed));
        }
        assert_eq!(
            index
                .resolved_entry("title", "zh-Hant-TW", "en")
                .unwrap()
                .locale,
            "zh"
        );
        assert_eq!(
            index.resolved_entry("title", "fr-CA", "en").unwrap().locale,
            "en"
        );
    }

    #[test]
    fn reports_mismatched_root_and_missing_plural_other() {
        let parsed = crate::parse_lua_source(
            "src/locales/en.lua",
            "return { fr = { count = { one = 'one' } } }",
        )
        .unwrap();
        let mut index = CatalogIndex::default();
        index.add_catalog("en", PathBuf::from("src/locales/en.lua"), Some(&parsed));
        assert!(index.diagnostics[&PathBuf::from("src/locales/en.lua")][0]
            .message
            .contains("root"));
        index.add_catalog("fr", PathBuf::from("src/locales/fr.lua"), Some(&parsed));
        assert!(index.diagnostics[&PathBuf::from("src/locales/fr.lua")][0]
            .message
            .contains("other"));
    }

    #[test]
    fn missing_locale_coverage_is_summarized_once_per_catalog() {
        let mut index = CatalogIndex::default();
        let en = crate::parse_lua_source(
            "src/locales/en.lua",
            "return { en = { first = 'First', second = 'Second' } }",
        )
        .unwrap();
        let fr = crate::parse_lua_source(
            "src/locales/fr.lua",
            "return { fr = { first = 'Premier' } }",
        )
        .unwrap();
        index.add_catalog("en", PathBuf::from("src/locales/en.lua"), Some(&en));
        index.add_catalog("fr", PathBuf::from("src/locales/fr.lua"), Some(&fr));
        let config = I18nConfig {
            enabled: true,
            catalogs: vec![
                crate::I18nCatalogConfig {
                    locale: "en".to_owned(),
                    path: PathBuf::from("locales/en.lua"),
                },
                crate::I18nCatalogConfig {
                    locale: "fr".to_owned(),
                    path: PathBuf::from("locales/fr.lua"),
                },
            ],
            ..I18nConfig::default()
        };
        index.finish(&config);
        let warnings = &index.diagnostics[&PathBuf::from("src/locales/fr.lua")];
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("missing 1 key"));
    }

    #[test]
    fn snapshot_completion_hover_definition_and_overlay_follow_catalog_revision() {
        use crate::{AnalysisHost, AnalysisSourceKind};
        let mut config = crate::ProjectConfig::default();
        config.i18n.enabled = true;
        config.i18n.catalogs.push(crate::I18nCatalogConfig {
            locale: "en".to_owned(),
            path: PathBuf::from("locales/en.lua"),
        });
        let mut host = AnalysisHost::with_config(config);
        host.set_source_roots([PathBuf::from("src")]);
        host.set_file_with_kind(
            "src/locales/en.lua",
            "return { en = { menu = { play = 'Play %{name}' } } }",
            AnalysisSourceKind::Lua,
        );
        let source = "local text = I18n.t(\"menu.pl\")\n";
        host.set_file_with_kind("src/main.tua", source, AnalysisSourceKind::Tua);
        let cursor = SourcePosition {
            line: 1,
            column: source.find("menu.pl").unwrap() + "menu.pl".len() + 1,
        };
        let snapshot = host.snapshot();
        assert!(snapshot
            .completion("src/main.tua", cursor)
            .iter()
            .any(|item| item.label == "menu.play"));

        let editing = "local text = I18n.t(\"menu.play\", { na\n";
        host.set_file_with_kind("src/main.tua", editing, AnalysisSourceKind::Tua);
        let cursor = SourcePosition {
            line: 1,
            column: editing.trim_end().len() + 1,
        };
        assert!(host
            .snapshot()
            .completion("src/main.tua", cursor)
            .iter()
            .any(|item| item.label == "name"));

        let source = "local text = I18n.t(\"menu.play\")\n";
        host.set_file_with_kind("src/main.tua", source, AnalysisSourceKind::Tua);
        let position = SourcePosition {
            line: 1,
            column: source.find("menu.play").unwrap() + 1,
        };
        let snapshot = host.snapshot();
        assert!(snapshot
            .hover("src/main.tua", position)
            .unwrap()
            .contents
            .contains("Play"));
        assert_eq!(
            snapshot.definition("src/main.tua", position).unwrap().path,
            PathBuf::from("src/locales/en.lua")
        );
        let references = snapshot.references("src/main.tua", position, true);
        assert_eq!(references.len(), 2);
        let rename = snapshot.rename("src/main.tua", position, "start").unwrap();
        assert_eq!(rename.edits.len(), 2);
        assert!(rename
            .edits
            .iter()
            .any(|edit| edit.path == std::path::Path::new("src/main.tua")
                && edit.new_text == "\"menu.start\""));
        assert!(rename.edits.iter().any(|edit| edit.path
            == std::path::Path::new("src/locales/en.lua")
            && edit.new_text == "start"));

        host.set_file_with_kind(
            "src/locales/en.lua",
            "return { en = { menu = { start = 'Start' } } }",
            AnalysisSourceKind::Lua,
        );
        let editing = "local text = I18n.t(\"menu.\")\n";
        host.set_file_with_kind("src/main.tua", editing, AnalysisSourceKind::Tua);
        let snapshot = host.snapshot();
        assert!(snapshot
            .completion(
                "src/main.tua",
                SourcePosition {
                    line: 1,
                    column: editing.find("menu.").unwrap() + "menu.".len() + 1
                }
            )
            .iter()
            .any(|item| item.label == "menu.start"));
        host.set_file_with_kind("src/main.tua", source, AnalysisSourceKind::Tua);
        let snapshot = host.snapshot();
        let diagnostics = snapshot.diagnostics("src/main.tua");
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unknown i18n key")));
        let actions = snapshot.code_actions(
            "src/main.tua",
            crate::AssistRange {
                start: position,
                end: position,
            },
            &diagnostics,
            &[crate::AssistKind::QuickFix],
        );
        assert!(actions.iter().any(
            |action| action.title == "Add `menu.play` to fallback catalog"
                && action.edit.changes[&PathBuf::from("src/locales/en.lua")][0]
                    .new_text
                    .contains("play = \"TODO\"")
        ));
    }

    #[test]
    fn typed_wrapper_preserves_key_completion_across_files() {
        use crate::{AnalysisHost, AnalysisSourceKind};
        let mut config = crate::ProjectConfig::default();
        config.i18n.enabled = true;
        config.i18n.catalogs.push(crate::I18nCatalogConfig {
            locale: "en".to_owned(),
            path: PathBuf::from("locales/en.lua"),
        });
        let mut host = AnalysisHost::with_config(config);
        host.set_source_roots([PathBuf::from("src")]);
        host.set_file_with_kind(
            "src/locales/en.lua",
            "return { en = { menu = { play = 'Play' } } }",
            AnalysisSourceKind::Lua,
        );
        host.set_file_with_kind("src/translate.tua", "local function translate(key: I18nKey)\n  return I18n.t(key)\nend\nreturn { translate = translate }\n", AnalysisSourceKind::Tua);
        let source = "local translate = require('translate').translate\nlocal text = translate(\"menu.pl\")\n";
        host.set_file_with_kind("src/main.tua", source, AnalysisSourceKind::Tua);
        let position = SourcePosition {
            line: 2,
            column: source.lines().nth(1).unwrap().find("menu.pl").unwrap() + "menu.pl".len() + 1,
        };
        let snapshot = host.snapshot();
        assert!(snapshot
            .completion("src/main.tua", position)
            .iter()
            .any(|item| item.label == "menu.play"));

        let source = "local translate = require('translate').translate\nlocal text = translate('menu.play')\n";
        host.set_file_with_kind("src/main.tua", source, AnalysisSourceKind::Tua);
        let position = SourcePosition {
            line: 2,
            column: source.lines().nth(1).unwrap().find("menu.play").unwrap() + 2,
        };
        let snapshot = host.snapshot();
        assert!(snapshot
            .hover("src/main.tua", position)
            .unwrap()
            .contents
            .contains("Play"));
        assert_eq!(
            snapshot.definition("src/main.tua", position).unwrap().path,
            PathBuf::from("src/locales/en.lua")
        );
        assert_eq!(snapshot.references("src/main.tua", position, true).len(), 2);
        assert_eq!(
            snapshot
                .rename("src/main.tua", position, "start")
                .unwrap()
                .edits
                .len(),
            2
        );
        host.set_file_with_kind(
            "src/main.tua",
            "local translate = require('translate').translate\ntranslate('menu.bad')\n",
            AnalysisSourceKind::Tua,
        );
        assert!(host
            .snapshot()
            .diagnostics("src/main.tua")
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unknown i18n key `menu.bad`")));
    }

    #[test]
    fn literal_text_result_narrows_checker_and_project_inference() {
        use crate::{AnalysisHost, AnalysisSourceKind};
        let mut config = crate::ProjectConfig::default();
        config.i18n.enabled = true;
        config.i18n.catalogs.push(crate::I18nCatalogConfig {
            locale: "en".to_owned(),
            path: PathBuf::from("locales/en.lua"),
        });
        let mut host = AnalysisHost::with_config(config);
        host.set_source_roots([PathBuf::from("src")]);
        host.set_file_with_kind(
            "src/locales/en.lua",
            "return { en = { title = 'Welcome' } }",
            AnalysisSourceKind::Lua,
        );
        host.set_file_with_kind(
            "src/main.tua",
            "local title: string = I18n.t('title')\nlocal inferred = I18n.t('title')\nlocal confirmed: string = inferred\nlocal dynamic_key: string = 'title'\nI18n.t(dynamic_key)\nlocal bad: number = I18n.t('title')\n",
            AnalysisSourceKind::Tua,
        );
        let snapshot = host.snapshot();
        let diagnostics = snapshot.diagnostics("src/main.tua");
        assert!(
            !diagnostics.iter().any(|diagnostic| diagnostic.line <= 5
                && diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
            "{diagnostics:?}"
        );
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.line == 6
                && diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn plural_results_are_strings_but_dialogue_results_remain_arrays() {
        use crate::{AnalysisHost, AnalysisSourceKind};
        let mut config = crate::ProjectConfig::default();
        config.i18n.enabled = true;
        config.i18n.catalogs.push(crate::I18nCatalogConfig {
            locale: "en".to_owned(),
            path: PathBuf::from("locales/en.lua"),
        });
        let mut host = AnalysisHost::with_config(config);
        host.set_source_roots([PathBuf::from("src")]);
        host.set_file_with_kind(
            "src/locales/en.lua",
            "return { en = { coins = { one = '%{count} coin', other = '%{count} coins' }, dialogue = { 'Hello', 'Bye' } } }",
            AnalysisSourceKind::Lua,
        );
        host.set_file_with_kind(
            "src/main.tua",
            "local coins: string = I18n.t('coins', {count = 2})\nlocal bad: string = I18n.t('dialogue')\n",
            AnalysisSourceKind::Tua,
        );
        let diagnostics = host.snapshot().diagnostics("src/main.tua");
        assert!(
            !diagnostics.iter().any(|diagnostic| diagnostic.line == 1
                && diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
            "{diagnostics:?}"
        );
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.line == 2
                && diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn reports_literal_locale_at_the_locale_argument() {
        use crate::{AnalysisHost, AnalysisSourceKind};
        let mut config = crate::ProjectConfig::default();
        config.i18n.enabled = true;
        config.i18n.catalogs.push(crate::I18nCatalogConfig {
            locale: "en".to_owned(),
            path: PathBuf::from("locales/en.lua"),
        });
        let mut host = AnalysisHost::with_config(config);
        host.set_source_roots([PathBuf::from("src")]);
        host.set_file_with_kind(
            "src/locales/en.lua",
            "return { en = { title = 'Hi' } }",
            AnalysisSourceKind::Lua,
        );
        host.set_file_with_kind(
            "src/main.tua",
            "I18n.setLocale('xx')\nI18n.t('title', {}, 'yy')\nI18n.t('title', {}, 'en-US')\n",
            AnalysisSourceKind::Tua,
        );
        let diagnostics = host.snapshot().diagnostics("src/main.tua");
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.message.contains("no configured catalog"))
                .count(),
            2,
            "{diagnostics:?}"
        );
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.line == 1 && diagnostic.column > 15));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.line == 2 && diagnostic.column > 20));
    }

    #[test]
    fn catalog_deletion_config_change_and_other_workspace_do_not_leak_keys() {
        use crate::{AnalysisHost, AnalysisSourceKind};
        let mut config = crate::ProjectConfig::default();
        config.i18n.enabled = true;
        config.i18n.catalogs.push(crate::I18nCatalogConfig {
            locale: "en".to_owned(),
            path: PathBuf::from("locales/en.lua"),
        });
        let source = "local title = I18n.t('menu.')\n";
        let position = SourcePosition {
            line: 1,
            column: source.find("menu.").unwrap() + 6,
        };
        let mut first = AnalysisHost::with_config(config.clone());
        first.set_source_roots([PathBuf::from("first/src")]);
        first.set_file_with_kind(
            "first/src/locales/en.lua",
            "return { en = { menu = { play = 'Play' } } }",
            AnalysisSourceKind::Lua,
        );
        first.set_file_with_kind("first/src/main.tua", source, AnalysisSourceKind::Tua);
        let mut second = AnalysisHost::with_config(config);
        second.set_source_roots([PathBuf::from("second/src")]);
        second.set_file_with_kind(
            "second/src/locales/en.lua",
            "return { en = { menu = { quit = 'Quit' } } }",
            AnalysisSourceKind::Lua,
        );
        second.set_file_with_kind("second/src/main.tua", source, AnalysisSourceKind::Tua);
        let first_labels = first.snapshot().completion("first/src/main.tua", position);
        let second_labels = second
            .snapshot()
            .completion("second/src/main.tua", position);
        assert!(first_labels.iter().any(|item| item.label == "menu.play"));
        assert!(!second_labels.iter().any(|item| item.label == "menu.play"));
        first.remove_file("first/src/locales/en.lua");
        let snapshot = first.snapshot();
        assert!(snapshot
            .i18n_catalog_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("missing")));
        assert!(!snapshot
            .completion("first/src/main.tua", position)
            .iter()
            .any(|item| item.label == "menu.play"));
        first.set_config(crate::ProjectConfig::default());
        assert!(first.snapshot().i18n_catalog_diagnostics().is_empty());
    }
}
