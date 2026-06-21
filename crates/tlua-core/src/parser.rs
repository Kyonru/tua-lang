use std::path::{Path, PathBuf};

use full_moon::ast::luau::{TypeDeclaration, TypeField, TypeFieldKey, TypeInfo};
use full_moon::ast::{self, LuaVersion};
use full_moon::node::Node;
use full_moon::tokenizer::Position;

use crate::{Diagnostic, DiagnosticCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

impl SourcePosition {
    fn from_full_moon(position: Position) -> Self {
        Self {
            line: position.line().max(1),
            column: position.character().max(1),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

impl SourceSpan {
    fn from_range(start: Position, end: Position) -> Self {
        Self {
            start: SourcePosition::from_full_moon(start),
            end: SourcePosition::from_full_moon(end),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParsedSource {
    pub path: PathBuf,
    pub source: String,
    pub ast: full_moon::ast::Ast,
    pub span: Option<SourceSpan>,
    pub type_aliases: Vec<TypeAlias>,
    pub local_annotations: Vec<LocalAnnotation>,
    pub function_annotations: Vec<FunctionAnnotation>,
}

impl ParsedSource {
    pub fn statement_count(&self) -> usize {
        self.ast.nodes().stmts().count()
    }

    pub fn has_return(&self) -> bool {
        self.ast.nodes().last_stmt().is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAlias {
    pub name: String,
    pub ty: TluaType,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalAnnotation {
    pub name: String,
    pub ty: TluaType,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionAnnotation {
    pub name: Option<String>,
    pub parameters: Vec<ParameterAnnotation>,
    pub return_type: Option<TluaType>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterAnnotation {
    pub name: String,
    pub ty: TluaType,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TluaType {
    Named(String),
    Union(Vec<TluaType>),
    Table(Vec<TableFieldType>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableFieldType {
    pub name: String,
    pub optional: bool,
    pub ty: TluaType,
    pub span: Option<SourceSpan>,
}

pub fn parse_lua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    parse_source_with_version(path, source, LuaVersion::lua51(), false)
}

pub fn parse_tlua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    parse_source_with_version(path, source, LuaVersion::luau(), true)
}

fn parse_source_with_version(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    lua_version: LuaVersion,
    collect_types: bool,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    let path = path.as_ref().to_path_buf();
    let source = source.into();

    let parser_source = if collect_types {
        normalize_tlua_source_for_luau_parser(&source)
    } else {
        source.clone()
    };

    match full_moon::parse_fallible(&parser_source, lua_version).into_result() {
        Ok(ast) => {
            let span = ast
                .range()
                .map(|(start, end)| SourceSpan::from_range(start, end));
            let type_syntax = if collect_types {
                collect_type_syntax(&ast)
            } else {
                TypeSyntax::default()
            };

            Ok(ParsedSource {
                path,
                source,
                ast,
                span,
                type_aliases: type_syntax.type_aliases,
                local_annotations: type_syntax.local_annotations,
                function_annotations: type_syntax.function_annotations,
            })
        }
        Err(errors) => Err(errors
            .into_iter()
            .map(|error| parser_error_to_diagnostic(&path, error))
            .collect()),
    }
}

#[derive(Default)]
struct TypeSyntax {
    type_aliases: Vec<TypeAlias>,
    local_annotations: Vec<LocalAnnotation>,
    function_annotations: Vec<FunctionAnnotation>,
}

fn collect_type_syntax(ast: &ast::Ast) -> TypeSyntax {
    let mut syntax = TypeSyntax::default();

    collect_block_type_syntax(ast.nodes(), &mut syntax);

    syntax
}

fn collect_block_type_syntax(block: &ast::Block, syntax: &mut TypeSyntax) {
    for stmt in block.stmts() {
        collect_stmt_type_syntax(stmt, syntax);
    }
}

fn collect_stmt_type_syntax(stmt: &ast::Stmt, syntax: &mut TypeSyntax) {
    match stmt {
        ast::Stmt::TypeDeclaration(declaration) => {
            syntax
                .type_aliases
                .push(type_alias_from_declaration(declaration));
        }
        ast::Stmt::LocalAssignment(local) => {
            for (name, specifier) in local.names().iter().zip(local.type_specifiers()) {
                let Some(specifier) = specifier else {
                    continue;
                };

                syntax.local_annotations.push(LocalAnnotation {
                    name: token_text(name),
                    ty: tlua_type_from_type_info(specifier.type_info()),
                    span: node_span(specifier),
                });
            }
        }
        ast::Stmt::LocalFunction(function) => {
            syntax.function_annotations.push(function_annotation(
                Some(token_text(function.name())),
                function.body(),
            ));
            collect_block_type_syntax(function.body().block(), syntax);
        }
        ast::Stmt::FunctionDeclaration(function) => {
            syntax.function_annotations.push(function_annotation(
                Some(function.name().to_string()),
                function.body(),
            ));
            collect_block_type_syntax(function.body().block(), syntax);
        }
        ast::Stmt::Do(block) => collect_block_type_syntax(block.block(), syntax),
        ast::Stmt::If(if_stmt) => {
            collect_block_type_syntax(if_stmt.block(), syntax);
            if let Some(else_ifs) = if_stmt.else_if() {
                for else_if in else_ifs {
                    collect_block_type_syntax(else_if.block(), syntax);
                }
            }
            if let Some(else_block) = if_stmt.else_block() {
                collect_block_type_syntax(else_block, syntax);
            }
        }
        ast::Stmt::GenericFor(for_stmt) => collect_block_type_syntax(for_stmt.block(), syntax),
        ast::Stmt::NumericFor(for_stmt) => collect_block_type_syntax(for_stmt.block(), syntax),
        ast::Stmt::Repeat(repeat) => collect_block_type_syntax(repeat.block(), syntax),
        ast::Stmt::While(while_stmt) => collect_block_type_syntax(while_stmt.block(), syntax),
        _ => {}
    }
}

fn type_alias_from_declaration(declaration: &TypeDeclaration) -> TypeAlias {
    TypeAlias {
        name: token_text(declaration.type_name()),
        ty: tlua_type_from_type_info(declaration.type_definition()),
        span: node_span(declaration),
    }
}

fn function_annotation(name: Option<String>, body: &ast::FunctionBody) -> FunctionAnnotation {
    let parameters = body
        .parameters()
        .iter()
        .zip(body.type_specifiers())
        .filter_map(|(parameter, specifier)| {
            let specifier = specifier?;
            let ast::Parameter::Name(name) = parameter else {
                return None;
            };

            Some(ParameterAnnotation {
                name: token_text(name),
                ty: tlua_type_from_type_info(specifier.type_info()),
                span: node_span(specifier),
            })
        })
        .collect();

    FunctionAnnotation {
        name,
        parameters,
        return_type: body
            .return_type()
            .map(|specifier| tlua_type_from_type_info(specifier.type_info())),
        span: node_span(body),
    }
}

fn tlua_type_from_type_info(type_info: &TypeInfo) -> TluaType {
    match type_info {
        TypeInfo::Basic(token) => TluaType::Named(token_text(token)),
        TypeInfo::Optional { base, .. } => TluaType::Union(vec![
            tlua_type_from_type_info(base),
            TluaType::Named("nil".to_owned()),
        ]),
        TypeInfo::Union(union) => {
            TluaType::Union(union.types().iter().map(tlua_type_from_type_info).collect())
        }
        TypeInfo::Table { fields, .. } => TluaType::Table(
            fields
                .iter()
                .filter_map(table_field_type_from_type_field)
                .collect(),
        ),
        other => TluaType::Named(other.to_string()),
    }
}

fn table_field_type_from_type_field(field: &TypeField) -> Option<TableFieldType> {
    let key = match field.key() {
        TypeFieldKey::Name(key) => key,
        _ => return None,
    };

    let mut name = token_text(key);
    let key_optional = name.ends_with('?');

    if key_optional {
        name.pop();
    }

    let (value_optional, ty) = table_field_value_type(field.value());

    Some(TableFieldType {
        name,
        optional: key_optional || value_optional,
        ty,
        span: node_span(field),
    })
}

fn table_field_value_type(type_info: &TypeInfo) -> (bool, TluaType) {
    match type_info {
        TypeInfo::Optional { base, .. } => (true, tlua_type_from_type_info(base)),
        _ => (false, tlua_type_from_type_info(type_info)),
    }
}

fn normalize_tlua_source_for_luau_parser(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut normalized = String::with_capacity(source.len());
    let mut index = 0;

    while index < chars.len() {
        if starts_line_comment(&chars, index) {
            index = push_until_line_end(&chars, index, &mut normalized);
            continue;
        }

        if chars[index] == '"' || chars[index] == '\'' {
            index = push_short_string(&chars, index, &mut normalized);
            continue;
        }

        if chars[index] == '[' && chars.get(index + 1) == Some(&'[') {
            index = push_long_bracket(&chars, index, &mut normalized);
            continue;
        }

        if is_optional_field_marker(&chars, index) {
            normalized.push(':');
            index += 2;
            index = push_type_with_trailing_optional(&chars, index, &mut normalized);
            continue;
        }

        normalized.push(chars[index]);
        index += 1;
    }

    normalized
}

fn is_optional_field_marker(chars: &[char], index: usize) -> bool {
    chars[index] == '?'
        && chars.get(index + 1) == Some(&':')
        && index > 0
        && is_identifier_char(chars[index - 1])
}

fn is_identifier_char(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn starts_line_comment(chars: &[char], index: usize) -> bool {
    chars[index] == '-' && chars.get(index + 1) == Some(&'-')
}

fn push_until_line_end(chars: &[char], mut index: usize, output: &mut String) -> usize {
    while index < chars.len() {
        let character = chars[index];
        output.push(character);
        index += 1;

        if character == '\n' {
            break;
        }
    }

    index
}

fn push_short_string(chars: &[char], mut index: usize, output: &mut String) -> usize {
    let quote = chars[index];
    output.push(quote);
    index += 1;

    while index < chars.len() {
        let character = chars[index];
        output.push(character);
        index += 1;

        if character == '\\' && index < chars.len() {
            output.push(chars[index]);
            index += 1;
            continue;
        }

        if character == quote {
            break;
        }
    }

    index
}

fn push_long_bracket(chars: &[char], mut index: usize, output: &mut String) -> usize {
    output.push(chars[index]);
    output.push(chars[index + 1]);
    index += 2;

    while index < chars.len() {
        let character = chars[index];
        output.push(character);
        index += 1;

        if character == ']' && chars.get(index) == Some(&']') {
            output.push(chars[index]);
            index += 1;
            break;
        }
    }

    index
}

fn push_type_with_trailing_optional(
    chars: &[char],
    mut index: usize,
    output: &mut String,
) -> usize {
    let mut depth = 0usize;

    while index < chars.len() {
        if starts_line_comment(chars, index) {
            index = push_until_line_end(chars, index, output);
            continue;
        }

        if chars[index] == '"' || chars[index] == '\'' {
            index = push_short_string(chars, index, output);
            continue;
        }

        if chars[index] == '[' && chars.get(index + 1) == Some(&'[') {
            index = push_long_bracket(chars, index, output);
            continue;
        }

        let character = chars[index];

        match character {
            '{' | '(' | '[' => {
                depth += 1;
                output.push(character);
                index += 1;
            }
            '}' if depth == 0 => {
                insert_optional_before_trailing_whitespace(output);
                output.push(character);
                return index + 1;
            }
            ',' if depth == 0 => {
                insert_optional_before_trailing_whitespace(output);
                output.push(character);
                return index + 1;
            }
            '}' | ')' | ']' => {
                depth = depth.saturating_sub(1);
                output.push(character);
                index += 1;
            }
            _ => {
                output.push(character);
                index += 1;
            }
        }
    }

    insert_optional_before_trailing_whitespace(output);
    index
}

fn insert_optional_before_trailing_whitespace(output: &mut String) {
    let insert_at = output.trim_end_matches(char::is_whitespace).len();
    output.insert(insert_at, '?');
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn parser_error_to_diagnostic(path: &Path, error: full_moon::Error) -> Diagnostic {
    let (start, _) = error.range();
    let position = SourcePosition::from_full_moon(start);

    Diagnostic::error(
        path,
        position.line,
        position.column,
        error.error_message().into_owned(),
    )
    .with_code(DiagnosticCode::SYNTAX_PARSE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_assignments() {
        let parsed = parse_lua_source("src/local.tlua", "local x = 1\nlocal y = x + 2\n")
            .expect("locals should parse");

        assert_eq!(parsed.path, PathBuf::from("src/local.tlua"));
        assert_eq!(parsed.statement_count(), 2);
        assert!(parsed.span.is_some());
    }

    #[test]
    fn parses_functions_and_returns() {
        let parsed = parse_lua_source(
            "src/function.tlua",
            r#"
local function add(a, b)
  return a + b
end
"#,
        )
        .expect("function should parse");

        assert_eq!(parsed.statement_count(), 1);
        assert!(parsed.source.contains("return a + b"));
    }

    #[test]
    fn parses_table_constructors() {
        let parsed = parse_lua_source(
            "src/table.tlua",
            r#"
local player = {
  hp = 100,
  alive = true,
}
"#,
        )
        .expect("table should parse");

        assert_eq!(parsed.statement_count(), 1);
        assert!(parsed.source.contains("alive"));
    }

    #[test]
    fn parses_return_statements() {
        let parsed = parse_lua_source("src/return.tlua", "return { ok = true }\n")
            .expect("return should parse");

        assert_eq!(parsed.statement_count(), 0);
        assert!(parsed.has_return());
    }

    #[test]
    fn parses_require_calls() {
        let parsed = parse_lua_source(
            "src/require.tlua",
            r#"
local Player = require("player")
return Player
"#,
        )
        .expect("require should parse");

        assert_eq!(parsed.statement_count(), 1);
        assert!(parsed.has_return());
        assert!(parsed.source.contains("require"));
    }

    #[test]
    fn parses_type_aliases_and_table_shapes() {
        let parsed = parse_tlua_source(
            "src/types.tlua",
            r#"
type Vec2 = {
  x: number,
  y: number,
}

type Player = {
  pos: Vec2,
  hp: number,
  alive: boolean,
}
"#,
        )
        .expect("type aliases and table shapes should parse");

        assert_eq!(parsed.type_aliases.len(), 2);

        let vec2 = &parsed.type_aliases[0];
        assert_eq!(vec2.name, "Vec2");

        let TluaType::Table(fields) = &vec2.ty else {
            panic!("Vec2 should be represented as a table shape");
        };

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "x");
        assert_eq!(fields[0].ty, TluaType::Named("number".to_owned()));
        assert_eq!(fields[1].name, "y");
        assert_eq!(fields[1].ty, TluaType::Named("number".to_owned()));

        let player = &parsed.type_aliases[1];
        assert_eq!(player.name, "Player");

        let TluaType::Table(fields) = &player.ty else {
            panic!("Player should be represented as a table shape");
        };

        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].name, "pos");
        assert_eq!(fields[0].ty, TluaType::Named("Vec2".to_owned()));
        assert_eq!(fields[1].name, "hp");
        assert_eq!(fields[1].ty, TluaType::Named("number".to_owned()));
        assert_eq!(fields[2].name, "alive");
        assert_eq!(fields[2].ty, TluaType::Named("boolean".to_owned()));
    }

    #[test]
    fn parses_optional_field_syntax_from_spec() {
        let parsed = parse_tlua_source(
            "src/save_slot.tlua",
            r#"
type SaveSlot = {
  name?: string,
  level: number,
  checkpoint?: string
}
"#,
        )
        .expect("optional table fields should parse");

        let alias = &parsed.type_aliases[0];
        assert_eq!(alias.name, "SaveSlot");

        let TluaType::Table(fields) = &alias.ty else {
            panic!("SaveSlot should be represented as a table shape");
        };

        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].name, "name");
        assert!(fields[0].optional);
        assert_eq!(fields[0].ty, TluaType::Named("string".to_owned()));
        assert_eq!(fields[1].name, "level");
        assert!(!fields[1].optional);
        assert_eq!(fields[1].ty, TluaType::Named("number".to_owned()));
        assert_eq!(fields[2].name, "checkpoint");
        assert!(fields[2].optional);
        assert_eq!(fields[2].ty, TluaType::Named("string".to_owned()));
    }

    #[test]
    fn parses_union_type_aliases() {
        let parsed = parse_tlua_source("src/maybe.tlua", "type MaybeNumber = number | nil\n")
            .expect("union type aliases should parse");

        assert_eq!(parsed.type_aliases.len(), 1);
        assert_eq!(parsed.type_aliases[0].name, "MaybeNumber");
        assert_eq!(
            parsed.type_aliases[0].ty,
            TluaType::Union(vec![
                TluaType::Named("number".to_owned()),
                TluaType::Named("nil".to_owned())
            ])
        );
    }

    #[test]
    fn parses_local_annotations() {
        let parsed = parse_tlua_source(
            "src/locals.tlua",
            r#"
local score: number = 0
local title: string = "Arena"
local current: Player | nil = nil
local payload: any = nil
"#,
        )
        .expect("local annotations should parse");

        assert_eq!(parsed.local_annotations.len(), 4);
        assert_eq!(parsed.local_annotations[0].name, "score");
        assert_eq!(
            parsed.local_annotations[0].ty,
            TluaType::Named("number".to_owned())
        );
        assert_eq!(parsed.local_annotations[1].name, "title");
        assert_eq!(
            parsed.local_annotations[1].ty,
            TluaType::Named("string".to_owned())
        );
        assert_eq!(parsed.local_annotations[2].name, "current");
        assert_eq!(
            parsed.local_annotations[2].ty,
            TluaType::Union(vec![
                TluaType::Named("Player".to_owned()),
                TluaType::Named("nil".to_owned())
            ])
        );
        assert_eq!(parsed.local_annotations[3].name, "payload");
        assert_eq!(
            parsed.local_annotations[3].ty,
            TluaType::Named("any".to_owned())
        );
    }

    #[test]
    fn parses_function_parameter_and_return_annotations() {
        let parsed = parse_tlua_source(
            "src/damage.tlua",
            r#"
local function damage(player: Player, amount: number): Player
  return player
end
"#,
        )
        .expect("function annotations should parse");

        assert_eq!(parsed.function_annotations.len(), 1);

        let function = &parsed.function_annotations[0];
        assert_eq!(function.name.as_deref(), Some("damage"));
        assert_eq!(function.parameters.len(), 2);
        assert_eq!(function.parameters[0].name, "player");
        assert_eq!(
            function.parameters[0].ty,
            TluaType::Named("Player".to_owned())
        );
        assert_eq!(function.parameters[1].name, "amount");
        assert_eq!(
            function.parameters[1].ty,
            TluaType::Named("number".to_owned())
        );
        assert_eq!(
            function.return_type,
            Some(TluaType::Named("Player".to_owned()))
        );
    }

    #[test]
    fn keeps_type_syntax_out_of_lua_compatible_parser() {
        let diagnostics = parse_lua_source("src/typed.tlua", "local score: number = 0\n")
            .expect_err("plain Lua parser should reject erased type syntax");

        assert_eq!(diagnostics[0].code, Some(DiagnosticCode::SYNTAX_PARSE));
    }

    #[test]
    fn syntax_errors_produce_tl000x_diagnostics_with_locations() {
        let diagnostics = parse_lua_source("src/broken.tlua", "local x = \n")
            .expect_err("invalid Lua should fail");

        assert!(!diagnostics.is_empty());

        let first = &diagnostics[0];

        assert_eq!(first.file, PathBuf::from("src/broken.tlua"));
        assert_eq!(first.severity, crate::DiagnosticSeverity::Error);
        assert_eq!(first.code, Some(DiagnosticCode::SYNTAX_PARSE));
        assert!(first.line >= 1);
        assert!(first.column >= 1);
        assert!(first.format_cli().contains("error TL0001"));
    }
}
