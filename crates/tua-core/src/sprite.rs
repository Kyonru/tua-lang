use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use full_moon::ast::{self, Call, Expression, Field, FunctionArgs, FunctionCall, Suffix};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};

use crate::asset::{self, AssetIndex, AssetLoaderKind, AssetMetadata, AssetValidationStatus};
use crate::ast_utils::{
    assignment_expression_for_local, field_chain_before_first_call, token_text,
};
use crate::emit::lowering::{LoweringDocument, LoweringOperation};
use crate::emit::LoweringKind;
use crate::inspector::{
    InspectorItem, SpriteAnimationInspection, SpriteAssetInspection, SpriteFrameInspection,
    SpriteSheetInspection,
};
use crate::types::{FunctionType, TableField, Type};
use crate::{
    Diagnostic, DiagnosticCode, DiagnosticSeverity, SourcePosition, SourceSpan, TypedParameter,
};

pub(crate) const HELPER_NAME: &str = "spriteSheet";
pub(crate) const TYPE_NAME: &str = "SpriteSheet";
pub(crate) const CONFIG_TYPE_NAME: &str = "SpriteSheetConfig";
pub(crate) const FRAME_CONFIG_TYPE_NAME: &str = "SpriteFrameConfig";
pub(crate) const ANIMATION_CONFIG_TYPE_NAME: &str = "SpriteAnimationConfig";
pub(crate) const RUNTIME_MODULE: &str = "anim8";

const BUILTIN_TYPE_NAMES: &[&str] = &[
    TYPE_NAME,
    CONFIG_TYPE_NAME,
    FRAME_CONFIG_TYPE_NAME,
    ANIMATION_CONFIG_TYPE_NAME,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SpriteDeclaration {
    pub(crate) name: String,
    pub(crate) image_path: Option<String>,
    pub(crate) frame: Option<SpriteFrame>,
    pub(crate) animations: Vec<SpriteAnimation>,
    pub(crate) span: SourceSpan,
    pub(crate) name_span: SourceSpan,
    pub(crate) image_span: Option<SourceSpan>,
    config_span: SourceSpan,
    frame_config_spans: Vec<SourceSpan>,
    animations_span: Option<SourceSpan>,
    animation_config_spans: Vec<SourceSpan>,
    statement_text: String,
    erased_statement_text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpriteFrame {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) left: u32,
    pub(crate) top: u32,
    pub(crate) border: u32,
    pub(crate) span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SpriteAnimation {
    pub(crate) name: String,
    pub(crate) start_frame: u32,
    pub(crate) end_frame: u32,
    pub(crate) row: u32,
    pub(crate) column: Option<u32>,
    pub(crate) frame: Option<SpriteFrame>,
    pub(crate) fps: Option<String>,
    pub(crate) durations: Option<String>,
    duration_preview: Option<String>,
    pub(crate) on_loop: Option<String>,
    pub(crate) looping: bool,
    pub(crate) span: SourceSpan,
    pub(crate) name_span: SourceSpan,
    pub(crate) fps_span: Option<SourceSpan>,
    pub(crate) durations_span: Option<SourceSpan>,
    has_selector_conflict: bool,
    has_timing_conflict: bool,
    has_loop_conflict: bool,
}

pub(crate) fn diagnostics(
    path: &Path,
    declarations: &[SpriteDeclaration],
    assets: &[AssetMetadata],
) -> Vec<Diagnostic> {
    declarations
        .iter()
        .flat_map(|declaration| sprite_declaration_diagnostics(path, assets, declaration))
        .collect()
}

pub(crate) fn inspections(
    path: &Path,
    declarations: &[SpriteDeclaration],
    assets: &[AssetMetadata],
    asset_roots: &[PathBuf],
    asset_index: Option<&AssetIndex>,
) -> Vec<InspectorItem> {
    declarations
        .iter()
        .map(|declaration| {
            let image =
                asset_for_declaration(assets, declaration).map(|asset| SpriteAssetInspection {
                    path: asset.relative_path.clone(),
                    resolved_path: asset::resolved_asset_path(asset, asset_roots, asset_index),
                    status: asset.status,
                    dimensions: asset.dimensions,
                    span: asset.span,
                });
            let frame = declaration.frame.map(sprite_frame_inspection);
            let animations = declaration
                .animations
                .iter()
                .map(|animation| SpriteAnimationInspection {
                    name: animation.name.clone(),
                    from: animation.start_frame,
                    to: animation.end_frame,
                    row: animation.row,
                    column: animation.column,
                    frame: animation.frame.map(sprite_frame_inspection),
                    fps: animation
                        .fps
                        .as_deref()
                        .and_then(|fps| fps.parse().ok())
                        .or_else(|| uniform_duration_fps(animation.duration_preview.as_deref()))
                        .unwrap_or(0.0),
                    durations: animation.durations.clone(),
                    on_loop: animation.on_loop.as_ref().map(|on_loop| {
                        quoted_expression_value(on_loop)
                            .unwrap_or(on_loop)
                            .to_owned()
                    }),
                    looping: animation.looping,
                    span: animation.span,
                    name_span: animation.name_span,
                })
                .collect();

            InspectorItem::SpriteSheet(SpriteSheetInspection {
                name: declaration.name.clone(),
                span: declaration.span,
                name_span: declaration.name_span,
                image,
                frame,
                animations,
                diagnostics: sprite_declaration_diagnostics(path, assets, declaration),
            })
        })
        .collect()
}

fn sprite_frame_inspection(frame: SpriteFrame) -> SpriteFrameInspection {
    SpriteFrameInspection {
        width: frame.width,
        height: frame.height,
        left: frame.left,
        top: frame.top,
        border: frame.border,
        span: frame.span,
    }
}

fn uniform_duration_fps(durations: Option<&str>) -> Option<f64> {
    let duration = durations?.parse::<f64>().ok()?;

    (duration > 0.0).then_some(1.0 / duration)
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
    let emitted_declarations = crate::parse_raw_tua_source(&parsed.path, output.as_str())
        .ok()
        .map(|raw| parse_sprite_declarations(&raw.ast).0);
    let lowered_declarations = emitted_declarations
        .as_ref()
        .filter(|declarations| declarations.len() == parsed.sprite_declarations.len())
        .unwrap_or(&parsed.sprite_declarations);
    let mut operations = Vec::new();
    let mut output_search_start = 0usize;
    let mut source_search_start = 0usize;

    for (declaration, lowered_declaration) in
        parsed.sprite_declarations.iter().zip(lowered_declarations)
    {
        let Some((relative_start, statement_len)) =
            find_sprite_statement(&output[output_search_start..], lowered_declaration)
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
            lower_sprite_declaration(lowered_declaration),
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
            TableField::required(Type::alias(FRAME_CONFIG_TYPE_NAME)),
        ),
        ("animations", TableField::required(Type::Any)),
    ])
}

pub(crate) fn builtin_aliases() -> Vec<(String, Type)> {
    vec![
        (TYPE_NAME.to_owned(), sprite_sheet_type()),
        (CONFIG_TYPE_NAME.to_owned(), sprite_sheet_config_type()),
        (
            FRAME_CONFIG_TYPE_NAME.to_owned(),
            sprite_frame_config_type(),
        ),
        (
            ANIMATION_CONFIG_TYPE_NAME.to_owned(),
            sprite_animation_config_type(),
        ),
    ]
}

fn sprite_sheet_config_type() -> Type {
    Type::table([
        ("image", TableField::required(Type::string())),
        (
            "frame",
            TableField::required(Type::alias(FRAME_CONFIG_TYPE_NAME)),
        ),
        ("animations", TableField::required(Type::Any)),
    ])
}

fn sprite_frame_config_type() -> Type {
    Type::table([
        ("width", TableField::required(Type::number())),
        ("height", TableField::required(Type::number())),
        ("left", TableField::optional(Type::number())),
        ("top", TableField::optional(Type::number())),
        ("border", TableField::optional(Type::number())),
    ])
}

fn sprite_animation_config_type() -> Type {
    Type::table([
        ("from", TableField::optional(Type::number())),
        ("to", TableField::optional(Type::number())),
        ("column", TableField::optional(Type::number())),
        ("row", TableField::optional(Type::number())),
        (
            "frame",
            TableField::optional(Type::alias(FRAME_CONFIG_TYPE_NAME)),
        ),
        ("fps", TableField::optional(Type::number())),
        ("durations", TableField::optional(Type::Any)),
        ("onLoop", TableField::optional(Type::Any)),
        ("loop", TableField::optional(Type::boolean())),
    ])
}

pub(crate) fn constructor_type() -> FunctionType {
    FunctionType::new([Type::alias(CONFIG_TYPE_NAME)], Type::alias(TYPE_NAME))
}

pub(crate) fn typed_function() -> crate::TypedFunction {
    crate::TypedFunction {
        name: Some(HELPER_NAME.to_owned()),
        parameters: vec![TypedParameter {
            name: "config".to_owned(),
            ty: Type::alias(CONFIG_TYPE_NAME),
            documentation: Some(
                "Sprite sheet image, grid geometry, and named animation configuration.".to_owned(),
            ),
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
    "function spriteSheet(config: SpriteSheetConfig): SpriteSheet"
}

pub(crate) const fn documentation() -> &'static str {
    "Creates a LOVE sprite sheet structure from a typed table literal and lowers to bundled anim8-backed Lua."
}

pub(crate) const fn sprite_sheet_snippet() -> &'static str {
    "local ${1:Player} = spriteSheet({\n  image = \"${2:assets/player.png}\",\n  frame = { width = ${3:16}, height = ${4:16}, left = ${5:0}, top = ${6:0}, border = ${7:0} },\n  animations = {\n    ${8:idle} = { from = ${9:0}, to = ${10:3}, row = ${11:0}, fps = ${12:8} },\n  },\n})"
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpriteConfigContext {
    Sheet,
    Frame,
    Animations,
    Animation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpriteConfigCompletionItem {
    pub(crate) label: &'static str,
    pub(crate) detail: &'static str,
    pub(crate) documentation: &'static str,
    pub(crate) insert_text: &'static str,
}

pub(crate) fn config_context_at(
    parsed: &crate::ParsedSource,
    position: SourcePosition,
) -> Option<SpriteConfigContext> {
    parsed.sprite_declarations.iter().find_map(|declaration| {
        if !span_contains_position(declaration.config_span, position) {
            return None;
        }
        if declaration
            .frame_config_spans
            .iter()
            .any(|span| span_contains_position(*span, position))
        {
            return Some(SpriteConfigContext::Frame);
        }
        if declaration
            .animation_config_spans
            .iter()
            .any(|span| span_contains_position(*span, position))
        {
            return Some(SpriteConfigContext::Animation);
        }
        if declaration
            .animations_span
            .is_some_and(|span| span_contains_position(span, position))
        {
            return Some(SpriteConfigContext::Animations);
        }

        Some(SpriteConfigContext::Sheet)
    })
}

pub(crate) fn config_completion_items(
    context: SpriteConfigContext,
) -> &'static [SpriteConfigCompletionItem] {
    match context {
        SpriteConfigContext::Sheet => &[
            SpriteConfigCompletionItem {
                label: "image",
                detail: "string",
                documentation: "Project-relative sprite sheet image path.",
                insert_text: "image = \"${1:assets/sprites.png}\",",
            },
            SpriteConfigCompletionItem {
                label: "frame",
                detail: FRAME_CONFIG_TYPE_NAME,
                documentation: "Default anim8 grid cell size, origin, and border spacing.",
                insert_text: "frame = { width = ${1:16}, height = ${2:16}, left = ${3:0}, top = ${4:0}, border = ${5:0} },",
            },
            SpriteConfigCompletionItem {
                label: "animations",
                detail: "{ [string]: SpriteAnimationConfig }",
                documentation: "Named animation definitions built from the sprite grid.",
                insert_text: "animations = {\n  ${1:idle} = { from = ${2:0}, to = ${3:3}, row = ${4:0}, fps = ${5:8} },\n},",
            },
        ],
        SpriteConfigContext::Frame => &[
            SpriteConfigCompletionItem {
                label: "width",
                detail: "number",
                documentation: "Grid cell width in pixels.",
                insert_text: "width = ${1:16},",
            },
            SpriteConfigCompletionItem {
                label: "height",
                detail: "number",
                documentation: "Grid cell height in pixels.",
                insert_text: "height = ${1:16},",
            },
            SpriteConfigCompletionItem {
                label: "left",
                detail: "number",
                documentation: "Pixels skipped from the image's left edge before the grid.",
                insert_text: "left = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "top",
                detail: "number",
                documentation: "Pixels skipped from the image's top edge before the grid.",
                insert_text: "top = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "border",
                detail: "number",
                documentation: "Pixel spacing around adjacent grid cells.",
                insert_text: "border = ${1:0},",
            },
        ],
        SpriteConfigContext::Animations => &[SpriteConfigCompletionItem {
            label: "animation",
            detail: ANIMATION_CONFIG_TYPE_NAME,
            documentation: "Named zero-based row/column animation range.",
            insert_text: "${1:name} = { from = ${2:0}, to = ${3:3}, row = ${4:0}, fps = ${5:8} },",
        }],
        SpriteConfigContext::Animation => &[
            SpriteConfigCompletionItem {
                label: "from",
                detail: "number",
                documentation: "Zero-based first column in the animation range.",
                insert_text: "from = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "to",
                detail: "number",
                documentation: "Zero-based last column in the animation range.",
                insert_text: "to = ${1:3},",
            },
            SpriteConfigCompletionItem {
                label: "column",
                detail: "number",
                documentation: "Zero-based single-column selector; replaces `from` and `to`.",
                insert_text: "column = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "row",
                detail: "number",
                documentation: "Zero-based grid row; defaults to 0.",
                insert_text: "row = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "frame",
                detail: FRAME_CONFIG_TYPE_NAME,
                documentation: "Optional per-animation grid size and origin override.",
                insert_text: "frame = { width = ${1:16}, height = ${2:16}, left = ${3:0}, top = ${4:0}, border = ${5:0} },",
            },
            SpriteConfigCompletionItem {
                label: "fps",
                detail: "number",
                documentation: "Frames per second shorthand; cannot be combined with `durations`.",
                insert_text: "fps = ${1:8},",
            },
            SpriteConfigCompletionItem {
                label: "durations",
                detail: "number | table",
                documentation: "anim8 duration in seconds, per-frame values, or ranges such as `{ ['3-5'] = 0.2 }`.",
                insert_text: "durations = ${1:0.125},",
            },
            SpriteConfigCompletionItem {
                label: "onLoop",
                detail: "function | string",
                documentation: "anim8 loop callback or animation method name such as `pauseAtEnd`.",
                insert_text: "onLoop = \"${1:pauseAtEnd}\",",
            },
            SpriteConfigCompletionItem {
                label: "loop",
                detail: "boolean",
                documentation: "Legacy shorthand; `false` maps to `onLoop = \"pauseAtEnd\"`.",
                insert_text: "loop = ${1:false},",
            },
        ],
    }
}

pub(crate) const fn on_loop_method_values() -> &'static [&'static str] {
    &["pauseAtEnd", "pauseAtStart", "pause", "resume"]
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
    let frame_table = named_field(table, "frame").and_then(table_expression);
    let frame = frame_table.and_then(parse_frame_table);
    let animations_table = named_field(table, "animations").and_then(table_expression);
    let animations = animations_table
        .map(parse_animation_table)
        .unwrap_or_default();
    let animation_config_spans = animations_table
        .map(animation_table_spans)
        .unwrap_or_default();
    let mut frame_config_spans = frame_table
        .and_then(node_span)
        .into_iter()
        .collect::<Vec<_>>();
    if let Some(animations_table) = animations_table {
        frame_config_spans.extend(animation_frame_table_spans(animations_table));
    }

    SpriteDeclaration {
        name,
        image_path,
        frame,
        animations,
        span,
        name_span,
        image_span,
        config_span: node_span(table).unwrap_or(span),
        frame_config_spans,
        animations_span: animations_table.and_then(node_span),
        animation_config_spans,
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
    let (width, _) = named_field(table, "width").and_then(u32_expression)?;
    let (height, _) = named_field(table, "height").and_then(u32_expression)?;

    Some(SpriteFrame {
        width,
        height,
        left: named_field(table, "left")
            .and_then(u32_expression)
            .map(|(value, _)| value)
            .unwrap_or(0),
        top: named_field(table, "top")
            .and_then(u32_expression)
            .map(|(value, _)| value)
            .unwrap_or(0),
        border: named_field(table, "border")
            .and_then(u32_expression)
            .map(|(value, _)| value)
            .unwrap_or(0),
        span: node_span(table)?,
    })
}

fn parse_animation_table(table: &ast::TableConstructor) -> Vec<SpriteAnimation> {
    table
        .fields()
        .iter()
        .filter_map(parse_animation_field)
        .collect()
}

fn animation_table_spans(table: &ast::TableConstructor) -> Vec<SourceSpan> {
    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NameKey { value, .. } = field else {
                return None;
            };

            table_expression(value).and_then(node_span)
        })
        .collect()
}

fn animation_frame_table_spans(table: &ast::TableConstructor) -> Vec<SourceSpan> {
    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NameKey { value, .. } = field else {
                return None;
            };
            let animation = table_expression(value)?;

            named_field(animation, "frame")
                .and_then(table_expression)
                .and_then(node_span)
        })
        .collect()
}

fn parse_animation_field(field: &Field) -> Option<SpriteAnimation> {
    let Field::NameKey { key, value, .. } = field else {
        return None;
    };
    let table = table_expression(value)?;
    let column = named_field(table, "column")
        .and_then(u32_expression)
        .map(|(value, _)| value);
    let range = named_field(table, "from")
        .and_then(u32_expression)
        .zip(named_field(table, "to").and_then(u32_expression));
    let (start_frame, end_frame) = match (column, range) {
        (Some(column), _) => (column, column),
        (None, Some(((start, _), (end, _)))) => (start, end),
        (None, None) => return None,
    };
    let fps = named_field(table, "fps").and_then(number_string_expression);
    let durations_expression = named_field(table, "durations");
    let durations = durations_expression.and_then(expression_text);
    if fps.is_none() && durations.is_none() {
        return None;
    }
    let on_loop = named_field(table, "onLoop").and_then(expression_text);
    let legacy_looping = named_field(table, "loop")
        .and_then(bool_expression)
        .unwrap_or(true);
    let looping = on_loop
        .as_ref()
        .and_then(|(text, _)| quoted_expression_value(text))
        .map(|method| !matches!(method, "pauseAtEnd" | "pauseAtStart" | "pause"))
        .unwrap_or(legacy_looping);
    let name_span = node_span(key)?;

    Some(SpriteAnimation {
        name: token_text(key),
        start_frame,
        end_frame,
        row: named_field(table, "row")
            .and_then(u32_expression)
            .map(|(value, _)| value)
            .unwrap_or(0),
        column,
        frame: named_field(table, "frame")
            .and_then(table_expression)
            .and_then(parse_frame_table),
        fps: fps.as_ref().map(|(value, _)| value.clone()),
        durations: durations.as_ref().map(|(value, _)| value.clone()),
        duration_preview: durations_expression.and_then(duration_preview_value),
        on_loop: on_loop.as_ref().map(|(value, _)| value.clone()),
        looping,
        span: node_span(value)?,
        name_span,
        fps_span: fps.map(|(_, span)| span),
        durations_span: durations.map(|(_, span)| span),
        has_selector_conflict: column.is_some() && range.is_some(),
        has_timing_conflict: named_field(table, "fps").is_some()
            && named_field(table, "durations").is_some(),
        has_loop_conflict: named_field(table, "onLoop").is_some()
            && named_field(table, "loop").is_some(),
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

fn expression_text(expression: &Expression) -> Option<(String, SourceSpan)> {
    Some((
        expression.to_string().trim().to_owned(),
        node_span(expression)?,
    ))
}

fn duration_preview_value(expression: &Expression) -> Option<String> {
    if let Some((duration, _)) = number_string_expression(expression) {
        return Some(duration);
    }
    let table = table_expression(expression)?;

    table.fields().iter().find_map(|field| {
        let value = match field {
            Field::ExpressionKey { value, .. } | Field::NameKey { value, .. } => value,
            Field::NoKey(value) => value,
            _ => return None,
        };

        number_string_expression(value).map(|(duration, _)| duration)
    })
}

fn quoted_expression_value(expression: &str) -> Option<&str> {
    let expression = expression.trim();
    let quote = expression.chars().next()?;

    matches!(quote, '\'' | '"')
        .then(|| expression.strip_prefix(quote)?.strip_suffix(quote))
        .flatten()
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
    path: &Path,
    assets: &[AssetMetadata],
    declaration: &SpriteDeclaration,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if declaration.image_path.is_none() {
        diagnostics.push(sprite_diagnostic(
            path,
            declaration.span,
            "sprite sheet requires an image string path",
        ));
    } else if let Some(image_span) = declaration.image_span {
        if let Some(asset) = asset_for_declaration(assets, declaration) {
            match asset.status {
                AssetValidationStatus::Missing => diagnostics.push(sprite_diagnostic(
                    path,
                    image_span,
                    format!("missing sprite image asset `{}`", asset.relative_path),
                )),
                AssetValidationStatus::UnsupportedExtension => diagnostics.push(sprite_diagnostic(
                    path,
                    image_span,
                    format!(
                        "unsupported sprite image extension for `{}`",
                        asset.relative_path
                    ),
                )),
                AssetValidationStatus::WrongKind => diagnostics.push(sprite_diagnostic(
                    path,
                    image_span,
                    format!(
                        "sprite image `{}` is not an image asset",
                        asset.relative_path
                    ),
                )),
                AssetValidationStatus::PathCaseMismatch => diagnostics.push(
                    Diagnostic::new(
                        path,
                        image_span.start.line,
                        image_span.start.column,
                        DiagnosticSeverity::Warning,
                        format!(
                            "asset path casing `{}` does not match the filesystem path `{}`",
                            asset.relative_path,
                            asset
                                .canonical_path
                                .as_deref()
                                .unwrap_or(&asset.relative_path)
                        ),
                    )
                    .with_code(DiagnosticCode::ASSET_PATH_CASE),
                ),
                AssetValidationStatus::Valid | AssetValidationStatus::Unchecked => {}
            }
        }
    }

    if let Some(frame) = declaration.frame {
        validate_frame(path, frame, "sprite frame", &mut diagnostics);
    } else {
        diagnostics.push(sprite_diagnostic(
            path,
            declaration.span,
            "sprite sheet requires frame width and height",
        ));
    }

    if declaration.animations.is_empty() {
        diagnostics.push(sprite_diagnostic(
            path,
            declaration.span,
            "sprite sheet requires at least one animation",
        ));
    }

    let mut animation_names = BTreeSet::new();

    for animation in &declaration.animations {
        if !animation_names.insert(animation.name.as_str()) {
            diagnostics.push(sprite_diagnostic(
                path,
                animation.name_span,
                format!(
                    "sprite animation `{}` is defined more than once",
                    animation.name
                ),
            ));
        }

        if animation.end_frame < animation.start_frame {
            diagnostics.push(sprite_diagnostic(
                path,
                animation.span,
                format!(
                    "sprite animation `{}` has invalid frame range {}..{}",
                    animation.name, animation.start_frame, animation.end_frame
                ),
            ));
        }

        if animation.has_selector_conflict {
            diagnostics.push(sprite_diagnostic(
                path,
                animation.span,
                format!(
                    "sprite animation `{}` cannot combine `column` with `from`/`to`",
                    animation.name
                ),
            ));
        }

        if animation.has_timing_conflict {
            diagnostics.push(sprite_diagnostic(
                path,
                animation.span,
                format!(
                    "sprite animation `{}` cannot combine `fps` with `durations`",
                    animation.name
                ),
            ));
        }

        if animation.has_loop_conflict {
            diagnostics.push(sprite_diagnostic(
                path,
                animation.span,
                format!(
                    "sprite animation `{}` cannot combine `loop` with `onLoop`",
                    animation.name
                ),
            ));
        }

        if let (Some(fps), Some(fps_span)) = (&animation.fps, animation.fps_span) {
            if fps.parse::<f64>().ok().is_none_or(|fps| fps <= 0.0) {
                diagnostics.push(sprite_diagnostic(
                    path,
                    fps_span,
                    format!(
                        "sprite animation `{}` fps must be greater than zero",
                        animation.name
                    ),
                ));
            }
        }

        if let (Some(durations), Some(durations_span)) =
            (&animation.durations, animation.durations_span)
        {
            if durations
                .parse::<f64>()
                .ok()
                .is_some_and(|duration| duration <= 0.0)
            {
                diagnostics.push(sprite_diagnostic(
                    path,
                    durations_span,
                    format!(
                        "sprite animation `{}` durations must be greater than zero",
                        animation.name
                    ),
                ));
            }
        }

        if let Some(frame) = animation.frame {
            validate_frame(
                path,
                frame,
                &format!("sprite animation `{}` frame", animation.name),
                &mut diagnostics,
            );
        }
    }

    diagnostics
}

fn validate_frame(path: &Path, frame: SpriteFrame, label: &str, diagnostics: &mut Vec<Diagnostic>) {
    if frame.width == 0 || frame.height == 0 {
        diagnostics.push(sprite_diagnostic(
            path,
            frame.span,
            format!("{label} size must be greater than zero"),
        ));
    }
}

fn asset_for_declaration<'a>(
    assets: &'a [AssetMetadata],
    declaration: &SpriteDeclaration,
) -> Option<&'a AssetMetadata> {
    let image_span = declaration.image_span?;

    assets.iter().find(|asset| {
        asset.kind == AssetLoaderKind::Image
            && asset.span == image_span
            && Some(asset.relative_path.as_str()) == declaration.image_path.as_deref()
    })
}

fn sprite_diagnostic(path: &Path, span: SourceSpan, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(
        path,
        span.start.line,
        span.start.column,
        DiagnosticSeverity::Error,
        message,
    )
    .with_code(DiagnosticCode::DOMAIN_SPRITE)
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
        left: 0,
        top: 0,
        border: 0,
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
        "local {grid_local} = {}\n",
        grid_constructor(frame, &image_local)
    ));
    for animation in &declaration.animations {
        let Some(animation_frame) = animation.frame else {
            continue;
        };
        let animation_grid = animation_grid_local(declaration, animation);
        output.push_str(&format!(
            "local {animation_grid} = {}\n",
            grid_constructor(animation_frame, &image_local)
        ));
    }
    output.push_str(&format!("local {} = {{\n", declaration.name));
    output.push_str(&format!("  image = {image_local},\n"));
    output.push_str(&format!("  grid = {grid_local},\n"));
    output.push_str(&format!(
        "  frame = {{ width = {}, height = {}, left = {}, top = {}, border = {} }},\n",
        frame.width, frame.height, frame.left, frame.top, frame.border
    ));
    output.push_str("  animations = {\n");

    for animation in &declaration.animations {
        let animation_grid = animation
            .frame
            .map(|_| animation_grid_local(declaration, animation))
            .unwrap_or_else(|| grid_local.clone());
        output.push_str(&format!(
            "    {} = {},\n",
            animation.name,
            lower_animation(animation, &animation_grid)
        ));
    }

    output.push_str("  },\n");
    output.push_str("}\n");
    output
}

fn grid_constructor(frame: SpriteFrame, image_local: &str) -> String {
    let mut arguments = format!(
        "{}, {}, {image_local}:getWidth(), {image_local}:getHeight()",
        frame.width, frame.height
    );

    if frame.left != 0 || frame.top != 0 || frame.border != 0 {
        arguments.push_str(&format!(
            ", {}, {}, {}",
            frame.left, frame.top, frame.border
        ));
    }

    format!("anim8.newGrid({arguments})")
}

fn animation_grid_local(declaration: &SpriteDeclaration, animation: &SpriteAnimation) -> String {
    format!("{}_{}Grid", declaration.name, animation.name)
}

fn lower_animation(animation: &SpriteAnimation, grid_local: &str) -> String {
    let start = animation.start_frame + 1;
    let end = animation.end_frame + 1;
    let row = animation.row + 1;
    let frames = if start == end {
        format!("{grid_local}({start}, {row})")
    } else {
        format!("{grid_local}({:?}, {row})", format!("{start}-{end}"))
    };
    let durations = animation
        .durations
        .clone()
        .or_else(|| animation.fps.as_ref().map(|fps| format!("1 / {fps}")))
        .unwrap_or_else(|| "0".to_owned());
    let on_loop = animation
        .on_loop
        .as_ref()
        .map(|on_loop| format!(", {on_loop}"))
        .unwrap_or_else(|| {
            if animation.looping {
                String::new()
            } else {
                ", \"pauseAtEnd\"".to_owned()
            }
        });

    format!("anim8.newAnimation({frames}, {durations}{on_loop})")
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

fn span_contains_position(span: SourceSpan, position: SourcePosition) -> bool {
    position_key(span.start) <= position_key(position)
        && position_key(position) <= position_key(span.end)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
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

    const ADVANCED_SPRITE_SOURCE: &str = r#"local Background = spriteSheet({
  image = "assets/background.png",
  frame = { width = 256, height = 256, left = 2, top = 4, border = 1 },
  animations = {
    orange = { column = 1, row = 0, durations = 1, onLoop = "pauseAtEnd" },
    blue = {
      from = 0,
      to = 3,
      row = 1,
      frame = { width = 128, height = 64, left = 8, top = 16, border = 2 },
      durations = { ['1-2'] = 0.1, ['3-4'] = 0.2 },
      onLoop = function(animation: any, loops: number)
        local completedLoops: number = loops
        completedLoops += 1
        animation:pause()
      end,
    },
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
        assert_eq!(sprite.animations[0].row, 0);
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

    #[test]
    fn parses_grid_geometry_selectors_durations_and_on_loop() {
        let parsed = parse_tua_source("src/sprites.tua", ADVANCED_SPRITE_SOURCE)
            .expect("advanced sprite should parse");
        let sprite = &parsed.sprite_declarations[0];
        let frame = sprite.frame.expect("default frame should parse");
        let orange = &sprite.animations[0];
        let blue = &sprite.animations[1];

        assert_eq!(
            (
                frame.width,
                frame.height,
                frame.left,
                frame.top,
                frame.border
            ),
            (256, 256, 2, 4, 1)
        );
        assert_eq!((orange.column, orange.row), (Some(1), 0));
        assert_eq!(orange.durations.as_deref(), Some("1"));
        assert_eq!(orange.on_loop.as_deref(), Some("\"pauseAtEnd\""));
        assert!(!orange.looping);
        assert_eq!((blue.start_frame, blue.end_frame, blue.row), (0, 3, 1));
        assert_eq!(
            blue.frame.map(|frame| (
                frame.width,
                frame.height,
                frame.left,
                frame.top,
                frame.border,
            )),
            Some((128, 64, 8, 16, 2))
        );
        assert!(blue
            .on_loop
            .as_deref()
            .is_some_and(|on_loop| on_loop.starts_with("function")));
    }

    #[test]
    fn lowers_advanced_sprite_configuration_to_anim8_arguments() {
        let lowered = crate::emit_tua_source("src/sprites.tua", ADVANCED_SPRITE_SOURCE)
            .expect("advanced sprite should lower");

        assert!(lowered.contains(
            "anim8.newGrid(256, 256, BackgroundImage:getWidth(), BackgroundImage:getHeight(), 2, 4, 1)"
        ));
        assert!(lowered.contains(
            "anim8.newGrid(128, 64, BackgroundImage:getWidth(), BackgroundImage:getHeight(), 8, 16, 2)"
        ));
        assert!(lowered
            .contains("orange = anim8.newAnimation(BackgroundGrid(2, 1), 1, \"pauseAtEnd\")"));
        assert!(lowered.contains(
            "blue = anim8.newAnimation(Background_blueGrid(\"1-4\", 2), { ['1-2'] = 0.1, ['3-4'] = 0.2 }, function(animation, loops)"
        ));
        assert!(lowered.contains("local completedLoops = loops"));
        assert!(lowered.contains("completedLoops = completedLoops + 1"));
        assert!(!lowered.contains("animation: any"));
        assert!(!lowered.contains("+="));
    }

    #[test]
    fn diagnoses_conflicting_sprite_animation_options() {
        let source = r#"local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = {
      column = 0,
      from = 0,
      to = 3,
      fps = 8,
      durations = 0.1,
      loop = false,
      onLoop = "pauseAtEnd",
    },
  },
})
"#;
        let parsed = parse_tua_source("src/sprites.tua", source).expect("sprite should parse");
        let diagnostics = super::sprite_declaration_diagnostics(
            std::path::Path::new("src/sprites.tua"),
            &[],
            &parsed.sprite_declarations[0],
        );
        let messages = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();

        assert!(messages
            .iter()
            .any(|message| message.contains("cannot combine `column`")));
        assert!(messages
            .iter()
            .any(|message| message.contains("cannot combine `fps`")));
        assert!(messages
            .iter()
            .any(|message| message.contains("cannot combine `loop`")));
    }
}
