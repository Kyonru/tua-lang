use std::path::Path;

use full_moon::ast::{self, Call, Expression, Field, FunctionArgs, FunctionCall, Suffix};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};

use crate::asset::{self, AssetIndex, AssetLoaderKind, AssetMetadata, AssetValidationStatus};
use crate::ast_utils::{
    assignment_expression_for_local, field_chain_before_first_call, token_text,
};
use crate::emit::lowering::{LoweringDocument, LoweringOperation};
use crate::emit::LoweringKind;
use crate::plugin::{PluginContext, TuaPlugin};
use crate::types::{FunctionType, TableField, Type};
use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity, SourceSpan, TypedParameter};

pub(crate) const HELPER_NAME: &str = "spriteSheet";
pub(crate) const TYPE_NAME: &str = "SpriteSheet";
pub(crate) const RUNTIME_MODULE: &str = "anim8";

const BUILTIN_TYPE_NAMES: &[&str] = &[TYPE_NAME];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SpriteDeclaration {
    pub(crate) name: String,
    pub(crate) image_path: Option<String>,
    pub(crate) frame: Option<SpriteFrame>,
    pub(crate) animations: Vec<SpriteAnimation>,
    pub(crate) span: SourceSpan,
    pub(crate) name_span: SourceSpan,
    pub(crate) image_span: Option<SourceSpan>,
    statement_text: String,
    erased_statement_text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpriteFrame {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SpriteAnimation {
    pub(crate) name: String,
    pub(crate) start_frame: u32,
    pub(crate) end_frame: u32,
    pub(crate) fps: String,
    pub(crate) looping: bool,
    pub(crate) span: SourceSpan,
    pub(crate) name_span: SourceSpan,
    pub(crate) fps_span: SourceSpan,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SpritePlugin;

impl TuaPlugin for SpritePlugin {
    fn name(&self) -> &'static str {
        "tua.sprite"
    }

    fn diagnostics(&self, context: PluginContext<'_>) -> Vec<Diagnostic> {
        let declarations = context
            .parsed
            .map(|parsed| parsed.sprite_declarations.as_slice())
            .unwrap_or(&[]);
        let mut diagnostics = Vec::new();

        for declaration in declarations {
            diagnostics.extend(sprite_declaration_diagnostics(context, declaration));
        }

        diagnostics
    }
}

pub(crate) fn parse_sprite_declarations(
    ast: &ast::Ast,
) -> (Vec<SpriteDeclaration>, Vec<Diagnostic>) {
    let mut declarations = Vec::new();

    collect_block_sprite_declarations(ast.nodes(), &mut declarations);

    (declarations, Vec::new())
}

pub(crate) fn lowering_operations(
    document: &LoweringDocument,
    parsed: &crate::ParsedSource,
) -> Vec<LoweringOperation> {
    if parsed.sprite_declarations.is_empty() {
        return Vec::new();
    }

    let output = document.code();
    let mut operations = Vec::new();
    let mut output_search_start = 0usize;
    let mut source_search_start = 0usize;

    for declaration in &parsed.sprite_declarations {
        let Some((relative_start, statement_len)) =
            find_sprite_statement(&output[output_search_start..], declaration)
        else {
            continue;
        };
        let start = output_search_start + relative_start;
        let end = start + statement_len;
        let (source_span, next_source_search_start) =
            original_sprite_statement_span(&parsed.source, declaration, source_search_start)
                .unwrap_or((declaration.span, source_search_start));

        operations.push(LoweringOperation::replace(
            start..end,
            lower_sprite_declaration(declaration),
            source_span,
            LoweringKind::Sprite,
        ));
        output_search_start = end;
        source_search_start = next_source_search_start;
    }

    operations
}

fn original_sprite_statement_span(
    source: &str,
    declaration: &SpriteDeclaration,
    search_start: usize,
) -> Option<(SourceSpan, usize)> {
    let statement = declaration.statement_text.trim_start();
    let erased_statement = declaration.erased_statement_text.trim_start();
    let remaining = source.get(search_start..)?;
    let (relative_start, length) = remaining
        .find(statement)
        .map(|start| (start, statement.len()))
        .or_else(|| {
            remaining
                .find(erased_statement)
                .map(|start| (start, erased_statement.len()))
        })?;
    let start = search_start + relative_start;

    Some((
        SourceSpan {
            start: source_position_from_byte(source, start),
            end: source_position_from_byte(source, start + length),
        },
        start + length,
    ))
}

fn source_position_from_byte(source: &str, byte: usize) -> crate::SourcePosition {
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

    crate::SourcePosition { line, column }
}

pub(crate) fn asset_metadata_for_declaration(
    declaration: &SpriteDeclaration,
    asset_roots: &[std::path::PathBuf],
    asset_index: Option<&AssetIndex>,
) -> Option<AssetMetadata> {
    let image_path = declaration.image_path.as_deref()?;
    let image_span = declaration.image_span?;

    Some(asset::validate_asset_with_index(
        AssetLoaderKind::Image,
        image_path,
        image_span,
        asset_roots,
        asset_index,
    ))
}

pub(crate) fn runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/anim8.lua")
}

pub(crate) fn builtin_type_names() -> &'static [&'static str] {
    BUILTIN_TYPE_NAMES
}

pub(crate) fn sprite_sheet_type() -> Type {
    Type::table([
        ("image", TableField::required(Type::alias("Image"))),
        ("grid", TableField::required(Type::Any)),
        (
            "frame",
            TableField::required(Type::table([
                ("width", TableField::required(Type::number())),
                ("height", TableField::required(Type::number())),
            ])),
        ),
        ("animations", TableField::required(Type::Any)),
    ])
}

pub(crate) fn constructor_type() -> FunctionType {
    FunctionType::new([Type::Any], Type::alias(TYPE_NAME))
}

pub(crate) fn typed_function() -> crate::TypedFunction {
    crate::TypedFunction {
        name: Some(HELPER_NAME.to_owned()),
        parameters: vec![TypedParameter {
            name: "config".to_owned(),
            ty: Type::Any,
            documentation: Some("Sprite sheet configuration table.".to_owned()),
        }],
        parameter_count: 1,
        required_parameter_count: 1,
        has_varargs: false,
        vararg_type: None,
        return_type: Some(Type::alias(TYPE_NAME)),
        return_types: vec![Type::alias(TYPE_NAME)],
        documentation: Some(documentation().to_owned()),
        overloads: Vec::new(),
    }
}

pub(crate) fn is_helper(name: &str) -> bool {
    name == HELPER_NAME
}

pub(crate) const fn detail() -> &'static str {
    "function spriteSheet(config: table): SpriteSheet"
}

pub(crate) const fn documentation() -> &'static str {
    "Creates a LOVE sprite sheet structure from a typed table literal and lowers to bundled anim8-backed Lua."
}

pub(crate) const fn sprite_sheet_snippet() -> &'static str {
    "local ${1:Player} = spriteSheet({\n  image = \"${2:assets/player.png}\",\n  frame = { width = ${3:16}, height = ${4:16} },\n  animations = {\n    ${5:idle} = { from = ${6:0}, to = ${7:3}, fps = ${8:8} },\n  },\n})"
}

fn collect_block_sprite_declarations(
    block: &ast::Block,
    declarations: &mut Vec<SpriteDeclaration>,
) {
    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::LocalAssignment(local) => {
                if let Some(declaration) = sprite_declaration_from_local_assignment(local) {
                    declarations.push(declaration);
                }
            }
            ast::Stmt::Do(block) => collect_block_sprite_declarations(block.block(), declarations),
            ast::Stmt::FunctionDeclaration(function) => {
                collect_block_sprite_declarations(function.body().block(), declarations);
            }
            ast::Stmt::LocalFunction(function) => {
                collect_block_sprite_declarations(function.body().block(), declarations);
            }
            ast::Stmt::GenericFor(for_stmt) => {
                collect_block_sprite_declarations(for_stmt.block(), declarations);
            }
            ast::Stmt::If(if_stmt) => {
                collect_block_sprite_declarations(if_stmt.block(), declarations);

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        collect_block_sprite_declarations(else_if.block(), declarations);
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    collect_block_sprite_declarations(else_block, declarations);
                }
            }
            ast::Stmt::NumericFor(for_stmt) => {
                collect_block_sprite_declarations(for_stmt.block(), declarations);
            }
            ast::Stmt::Repeat(repeat) => {
                collect_block_sprite_declarations(repeat.block(), declarations);
            }
            ast::Stmt::While(while_stmt) => {
                collect_block_sprite_declarations(while_stmt.block(), declarations);
            }
            _ => {}
        }
    }
}

fn sprite_declaration_from_local_assignment(
    local: &ast::LocalAssignment,
) -> Option<SpriteDeclaration> {
    let names = local.names().iter().collect::<Vec<_>>();
    let expressions = local.expressions().iter().collect::<Vec<_>>();

    if names.len() != 1 || expressions.len() != 1 {
        return None;
    }

    let name = names[0];
    let (expression, _) = assignment_expression_for_local(&expressions, 0)?;
    let table = sprite_sheet_call_table(expression)?;
    let span = node_span(local)?;
    let name_span = node_span(name)?;

    Some(sprite_declaration_from_config(
        token_text(name),
        name_span,
        span,
        local.to_string(),
        format!("local {} = {}", token_text(name), expression),
        table,
    ))
}

fn sprite_sheet_call_table(expression: &Expression) -> Option<&ast::TableConstructor> {
    match expression {
        Expression::FunctionCall(function_call) if is_sprite_sheet_call(function_call) => {
            first_table_argument(function_call)
        }
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => sprite_sheet_call_table(expression),
        _ => None,
    }
}

fn is_sprite_sheet_call(function_call: &FunctionCall) -> bool {
    field_chain_before_first_call(function_call)
        .is_some_and(|parts| parts.len() == 1 && parts[0] == HELPER_NAME)
}

fn first_table_argument(function_call: &FunctionCall) -> Option<&ast::TableConstructor> {
    function_call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })) => {
            arguments.iter().next().and_then(table_expression)
        }
        Suffix::Call(Call::AnonymousCall(FunctionArgs::TableConstructor(table))) => Some(table),
        _ => None,
    })
}

fn sprite_declaration_from_config(
    name: String,
    name_span: SourceSpan,
    span: SourceSpan,
    statement_text: String,
    erased_statement_text: String,
    table: &ast::TableConstructor,
) -> SpriteDeclaration {
    let (image_path, image_span) = named_field(table, "image")
        .and_then(string_expression)
        .map(|(path, span)| (Some(path), Some(span)))
        .unwrap_or((None, None));
    let frame = named_field(table, "frame")
        .and_then(table_expression)
        .and_then(parse_frame_table);
    let animations = named_field(table, "animations")
        .and_then(table_expression)
        .map(parse_animation_table)
        .unwrap_or_default();

    SpriteDeclaration {
        name,
        image_path,
        frame,
        animations,
        span,
        name_span,
        image_span,
        statement_text,
        erased_statement_text,
    }
}

fn find_sprite_statement(output: &str, declaration: &SpriteDeclaration) -> Option<(usize, usize)> {
    output
        .find(&declaration.statement_text)
        .map(|start| (start, declaration.statement_text.len()))
        .or_else(|| {
            output
                .find(&declaration.erased_statement_text)
                .map(|start| (start, declaration.erased_statement_text.len()))
        })
}

fn parse_frame_table(table: &ast::TableConstructor) -> Option<SpriteFrame> {
    let (width, width_span) = named_field(table, "width").and_then(u32_expression)?;
    let (height, height_span) = named_field(table, "height").and_then(u32_expression)?;

    Some(SpriteFrame {
        width,
        height,
        span: SourceSpan {
            start: width_span.start,
            end: height_span.end,
        },
    })
}

fn parse_animation_table(table: &ast::TableConstructor) -> Vec<SpriteAnimation> {
    table
        .fields()
        .iter()
        .filter_map(parse_animation_field)
        .collect()
}

fn parse_animation_field(field: &Field) -> Option<SpriteAnimation> {
    let Field::NameKey { key, value, .. } = field else {
        return None;
    };
    let table = table_expression(value)?;
    let (start_frame, start_span) = named_field(table, "from").and_then(u32_expression)?;
    let (end_frame, _) = named_field(table, "to").and_then(u32_expression)?;
    let (fps, fps_span) = named_field(table, "fps").and_then(number_string_expression)?;
    let looping = named_field(table, "loop")
        .and_then(bool_expression)
        .unwrap_or(true);
    let name_span = node_span(key)?;

    Some(SpriteAnimation {
        name: token_text(key),
        start_frame,
        end_frame,
        fps,
        looping,
        span: node_span(value).unwrap_or(SourceSpan {
            start: start_span.start,
            end: fps_span.end,
        }),
        name_span,
        fps_span,
    })
}

fn named_field<'a>(table: &'a ast::TableConstructor, name: &str) -> Option<&'a Expression> {
    table.fields().iter().find_map(|field| {
        let Field::NameKey { key, value, .. } = field else {
            return None;
        };

        (token_text(key) == name).then_some(value)
    })
}

fn table_expression(expression: &Expression) -> Option<&ast::TableConstructor> {
    match expression {
        Expression::TableConstructor(table) => Some(table),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => table_expression(expression),
        _ => None,
    }
}

fn string_expression(expression: &Expression) -> Option<(String, SourceSpan)> {
    match expression {
        Expression::String(token) => Some((string_literal_value(token)?, node_span(token)?)),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => string_expression(expression),
        _ => None,
    }
}

fn u32_expression(expression: &Expression) -> Option<(u32, SourceSpan)> {
    let (number, span) = number_string_expression(expression)?;

    Some((number.parse().ok()?, span))
}

fn number_string_expression(expression: &Expression) -> Option<(String, SourceSpan)> {
    match expression {
        Expression::Number(token) => Some((token_text(token), node_span(token)?)),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => number_string_expression(expression),
        _ => None,
    }
}

fn bool_expression(expression: &Expression) -> Option<bool> {
    match expression {
        Expression::Symbol(token) => match token_text(token).as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => bool_expression(expression),
        _ => None,
    }
}

fn sprite_declaration_diagnostics(
    context: PluginContext<'_>,
    declaration: &SpriteDeclaration,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if let Some(image_span) = declaration.image_span {
        if let Some(asset) = context.assets.iter().find(|asset| {
            asset.kind == AssetLoaderKind::Image
                && asset.span == image_span
                && Some(asset.relative_path.as_str()) == declaration.image_path.as_deref()
        }) {
            match asset.status {
                AssetValidationStatus::Missing => diagnostics.push(sprite_diagnostic(
                    context.path,
                    image_span,
                    format!("missing sprite image asset `{}`", asset.relative_path),
                )),
                AssetValidationStatus::UnsupportedExtension => diagnostics.push(sprite_diagnostic(
                    context.path,
                    image_span,
                    format!(
                        "unsupported sprite image extension for `{}`",
                        asset.relative_path
                    ),
                )),
                AssetValidationStatus::WrongKind => diagnostics.push(sprite_diagnostic(
                    context.path,
                    image_span,
                    format!(
                        "sprite image `{}` is not an image asset",
                        asset.relative_path
                    ),
                )),
                AssetValidationStatus::Valid | AssetValidationStatus::Unchecked => {}
            }
        }
    }

    if let Some(frame) = declaration.frame {
        if frame.width == 0 || frame.height == 0 {
            diagnostics.push(sprite_diagnostic(
                context.path,
                frame.span,
                "sprite frame size must be greater than zero",
            ));
        }
    }

    for animation in &declaration.animations {
        if animation.end_frame < animation.start_frame {
            diagnostics.push(sprite_diagnostic(
                context.path,
                animation.span,
                format!(
                    "sprite animation `{}` has invalid frame range {}..{}",
                    animation.name, animation.start_frame, animation.end_frame
                ),
            ));
        }

        if animation
            .fps
            .parse::<f64>()
            .ok()
            .is_none_or(|fps| fps <= 0.0)
        {
            diagnostics.push(sprite_diagnostic(
                context.path,
                animation.fps_span,
                format!(
                    "sprite animation `{}` fps must be greater than zero",
                    animation.name
                ),
            ));
        }
    }

    diagnostics
}

fn sprite_diagnostic(path: &Path, span: SourceSpan, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(
        path,
        span.start.line,
        span.start.column,
        DiagnosticSeverity::Error,
        message,
    )
    .with_code(DiagnosticCode::PLUGIN_SPRITE)
}

fn lower_sprite_declaration(declaration: &SpriteDeclaration) -> String {
    let image_local = format!("{}Image", declaration.name);
    let grid_local = format!("{}Grid", declaration.name);
    let image_value = declaration
        .image_path
        .as_deref()
        .map(lua_string_literal)
        .unwrap_or_else(|| "nil".to_owned());
    let frame = declaration.frame.unwrap_or(SpriteFrame {
        width: 0,
        height: 0,
        span: declaration.span,
    });
    let mut output = String::new();

    output.push_str(&format!(
        "-- Sprite helper: {} lowers to anim8-backed Lua.\n",
        declaration.name
    ));
    output.push_str(&format!("local anim8 = require(\"{RUNTIME_MODULE}\")\n"));
    output.push_str(&format!(
        "local {image_local} = love.graphics.newImage({image_value})\n"
    ));
    output.push_str(&format!(
        "local {grid_local} = anim8.newGrid({}, {}, {image_local}:getWidth(), {image_local}:getHeight())\n",
        frame.width, frame.height
    ));
    output.push_str(&format!("local {} = {{\n", declaration.name));
    output.push_str(&format!("  image = {image_local},\n"));
    output.push_str(&format!("  grid = {grid_local},\n"));
    output.push_str(&format!(
        "  frame = {{ width = {}, height = {} }},\n",
        frame.width, frame.height
    ));
    output.push_str("  animations = {\n");

    for animation in &declaration.animations {
        output.push_str(&format!(
            "    {} = {},\n",
            animation.name,
            lower_animation(animation, &grid_local)
        ));
    }

    output.push_str("  },\n");
    output.push_str("}\n");
    output
}

fn lower_animation(animation: &SpriteAnimation, grid_local: &str) -> String {
    let start = animation.start_frame + 1;
    let end = animation.end_frame + 1;
    let frames = if start == end {
        format!("{grid_local}({start}, 1)")
    } else {
        format!("{grid_local}({:?}, 1)", format!("{start}-{end}"))
    };
    let loop_mode = if animation.looping {
        String::new()
    } else {
        ", \"pauseAtEnd\"".to_owned()
    };

    format!(
        "anim8.newAnimation({frames}, 1 / {}{loop_mode})",
        animation.fps
    )
}

fn string_literal_value(token: &TokenReference) -> Option<String> {
    match token.token().token_type() {
        TokenType::StringLiteral { literal, .. } => Some(literal.to_string()),
        _ => None,
    }
}

fn lua_string_literal(value: &str) -> String {
    format!("{value:?}")
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

#[cfg(test)]
mod tests {
    use crate::parse_tua_source;

    const SPRITE_SOURCE: &str = r#"local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = { from = 0, to = 3, fps = 8 },
    run = { from = 4, to = 9, fps = 12, loop = false },
  },
})
"#;

    #[test]
    fn parses_sprite_sheet_helper_form() {
        let parsed =
            parse_tua_source("src/sprites.tua", SPRITE_SOURCE).expect("sprite should parse");
        let sprite = parsed
            .sprite_declarations
            .first()
            .expect("sprite helper should be recorded");

        assert_eq!(sprite.name, "Player");
        assert_eq!(sprite.image_path.as_deref(), Some("assets/player.png"));
        assert_eq!(
            sprite.frame.map(|frame| (frame.width, frame.height)),
            Some((16, 16))
        );
        assert_eq!(sprite.animations.len(), 2);
        assert_eq!(sprite.animations[0].name, "idle");
        assert_eq!(sprite.animations[0].start_frame, 0);
        assert_eq!(sprite.animations[0].end_frame, 3);
        assert!(sprite.animations[0].looping);
        assert!(!sprite.animations[1].looping);
    }

    #[test]
    fn lowers_sprite_sheet_helper_call() {
        let lowered =
            crate::emit_tua_source("src/sprites.tua", SPRITE_SOURCE).expect("sprite should lower");

        assert!(lowered.contains("-- Sprite helper: Player lowers to anim8-backed Lua."));
        assert!(lowered.contains("local anim8 = require(\"anim8\")"));
        assert!(lowered.contains(
            "local PlayerGrid = anim8.newGrid(16, 16, PlayerImage:getWidth(), PlayerImage:getHeight())"
        ));
        assert!(!lowered.contains("spriteSheet({"));
    }
}
