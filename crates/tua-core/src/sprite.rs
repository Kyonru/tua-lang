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
    InspectorItem, SpriteAnimationInspection, SpriteAssetInspection, SpriteChildInspection,
    SpriteFrameInspection, SpriteFrameMetadataInspection, SpriteFrameRangeInspection,
    SpriteMarkerInspection, SpriteOriginInspection, SpriteSheetInspection,
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
pub(crate) const ANIMATION_TYPE_NAME: &str = "SpriteAnimation";
pub(crate) const PLAYER_TYPE_NAME: &str = "SpritePlayer";
pub(crate) const ORIGIN_CONFIG_TYPE_NAME: &str = "SpriteOriginConfig";
pub(crate) const RANGE_CONFIG_TYPE_NAME: &str = "SpriteFrameRangeConfig";
pub(crate) const MARKER_CONFIG_TYPE_NAME: &str = "SpriteMarkerConfig";
pub(crate) const FRAME_METADATA_CONFIG_TYPE_NAME: &str = "SpriteFrameMetadataConfig";
pub(crate) const CHILD_CONFIG_TYPE_NAME: &str = "SpriteChildConfig";
pub(crate) const RUNTIME_MODULE: &str = "anim8";
pub(crate) const PLAYER_RUNTIME_MODULE: &str = "tuasprite";

const BUILTIN_TYPE_NAMES: &[&str] = &[
    TYPE_NAME,
    CONFIG_TYPE_NAME,
    FRAME_CONFIG_TYPE_NAME,
    ANIMATION_CONFIG_TYPE_NAME,
    ANIMATION_TYPE_NAME,
    PLAYER_TYPE_NAME,
    ORIGIN_CONFIG_TYPE_NAME,
    RANGE_CONFIG_TYPE_NAME,
    MARKER_CONFIG_TYPE_NAME,
    FRAME_METADATA_CONFIG_TYPE_NAME,
    CHILD_CONFIG_TYPE_NAME,
];

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SpriteSpec {
    animations: Vec<String>,
    markers: Vec<String>,
    children: Vec<String>,
    exact: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SpriteDeclaration {
    pub(crate) name: String,
    pub(crate) image_path: Option<String>,
    pub(crate) frame: Option<SpriteFrame>,
    pub(crate) origin: SpriteOrigin,
    pub(crate) animations: Vec<SpriteAnimation>,
    pub(crate) children: Vec<SpriteChild>,
    pub(crate) span: SourceSpan,
    pub(crate) name_span: SourceSpan,
    pub(crate) image_span: Option<SourceSpan>,
    config_span: SourceSpan,
    frame_config_spans: Vec<SourceSpan>,
    origin_config_spans: Vec<SourceSpan>,
    range_config_spans: Vec<SourceSpan>,
    marker_config_spans: Vec<SourceSpan>,
    metadata_config_spans: Vec<SourceSpan>,
    children_span: Option<SourceSpan>,
    child_config_spans: Vec<SourceSpan>,
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SpriteOrigin {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpriteFrameRange {
    pub(crate) start_frame: u32,
    pub(crate) end_frame: u32,
    pub(crate) row: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SpriteMarker {
    pub(crate) frame: u32,
    pub(crate) name: String,
    pub(crate) span: SourceSpan,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SpriteFrameMetadata {
    pub(crate) origin: Option<SpriteOrigin>,
    pub(crate) trim_x: i32,
    pub(crate) trim_y: i32,
    pub(crate) source_width: Option<u32>,
    pub(crate) source_height: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SpriteChild {
    pub(crate) name: String,
    pub(crate) sheet: Option<String>,
    pub(crate) initial: Option<String>,
    pub(crate) x: String,
    pub(crate) y: String,
    pub(crate) rotation: String,
    pub(crate) scale_x: String,
    pub(crate) scale_y: String,
    pub(crate) span: SourceSpan,
    pub(crate) name_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SpriteAnimation {
    pub(crate) name: String,
    pub(crate) start_frame: u32,
    pub(crate) end_frame: u32,
    pub(crate) row: u32,
    pub(crate) column: Option<u32>,
    pub(crate) ranges: Vec<SpriteFrameRange>,
    pub(crate) frame: Option<SpriteFrame>,
    pub(crate) origin: Option<SpriteOrigin>,
    pub(crate) markers: Vec<SpriteMarker>,
    pub(crate) metadata: Vec<SpriteFrameMetadata>,
    pub(crate) fps: Option<String>,
    pub(crate) durations: Option<String>,
    duration_preview: Option<String>,
    duration_values: Vec<String>,
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
                    ranges: animation
                        .ranges
                        .iter()
                        .map(|range| SpriteFrameRangeInspection {
                            from: range.start_frame,
                            to: range.end_frame,
                            row: range.row,
                        })
                        .collect(),
                    frame: animation.frame.map(sprite_frame_inspection),
                    origin: sprite_origin_inspection(
                        animation.origin.unwrap_or(declaration.origin),
                    ),
                    fps: animation
                        .fps
                        .as_deref()
                        .and_then(|fps| fps.parse().ok())
                        .or_else(|| uniform_duration_fps(animation.duration_preview.as_deref()))
                        .unwrap_or(0.0),
                    durations: animation.durations.clone(),
                    duration_values: animation
                        .duration_values
                        .iter()
                        .filter_map(|duration| duration.parse().ok())
                        .collect(),
                    markers: animation
                        .markers
                        .iter()
                        .map(|marker| SpriteMarkerInspection {
                            frame: marker.frame,
                            name: marker.name.clone(),
                        })
                        .collect(),
                    metadata: animation
                        .metadata
                        .iter()
                        .map(|metadata| {
                            let frame =
                                animation
                                    .frame
                                    .or(declaration.frame)
                                    .unwrap_or(SpriteFrame {
                                        width: 0,
                                        height: 0,
                                        left: 0,
                                        top: 0,
                                        border: 0,
                                        span: animation.span,
                                    });
                            SpriteFrameMetadataInspection {
                                origin: sprite_origin_inspection(
                                    metadata
                                        .origin
                                        .or(animation.origin)
                                        .unwrap_or(declaration.origin),
                                ),
                                trim_x: metadata.trim_x,
                                trim_y: metadata.trim_y,
                                source_width: metadata.source_width.unwrap_or(frame.width),
                                source_height: metadata.source_height.unwrap_or(frame.height),
                            }
                        })
                        .collect(),
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
            let children = declaration
                .children
                .iter()
                .map(|child| SpriteChildInspection {
                    name: child.name.clone(),
                    sheet: child.sheet.clone(),
                    initial: child.initial.clone(),
                    x: child.x.clone(),
                    y: child.y.clone(),
                    rotation: child.rotation.clone(),
                    scale_x: child.scale_x.clone(),
                    scale_y: child.scale_y.clone(),
                    span: child.span,
                    name_span: child.name_span,
                })
                .collect();

            InspectorItem::SpriteSheet(SpriteSheetInspection {
                name: declaration.name.clone(),
                span: declaration.span,
                name_span: declaration.name_span,
                image,
                frame,
                origin: sprite_origin_inspection(declaration.origin),
                animations,
                children,
                diagnostics: sprite_declaration_diagnostics(path, assets, declaration),
            })
        })
        .collect()
}

fn sprite_origin_inspection(origin: SpriteOrigin) -> SpriteOriginInspection {
    SpriteOriginInspection {
        x: origin.x,
        y: origin.y,
    }
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

pub(crate) fn player_runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/tuasprite.lua")
}

pub(crate) fn builtin_type_names() -> &'static [&'static str] {
    BUILTIN_TYPE_NAMES
}

pub(crate) fn sprite_sheet_type() -> Type {
    sprite_sheet_type_for_spec(&SpriteSpec::default())
}

fn sprite_sheet_type_for_spec(spec: &SpriteSpec) -> Type {
    let animation_names = literal_union_or_string(&spec.animations);
    let child_names = literal_union_or_string(&spec.children);
    let animations = if spec.exact {
        Type::table(spec.animations.iter().map(|name| {
            (
                name.clone(),
                TableField::required(Type::alias(ANIMATION_TYPE_NAME)),
            )
        }))
    } else {
        Type::dictionary(Type::string(), Type::alias(ANIMATION_TYPE_NAME))
    };
    let player = sprite_player_type(spec);

    Type::table([
        ("image", TableField::required(Type::alias("Image"))),
        ("grid", TableField::required(Type::Any)),
        (
            "frame",
            TableField::required(Type::alias(FRAME_CONFIG_TYPE_NAME)),
        ),
        (
            "origin",
            TableField::required(Type::alias(ORIGIN_CONFIG_TYPE_NAME)),
        ),
        ("animations", TableField::required(animations)),
        (
            "animationOrder",
            TableField::required(Type::array(animation_names.clone())),
        ),
        ("markers", TableField::required(Type::Any)),
        ("frameMetadata", TableField::required(Type::Any)),
        ("children", TableField::required(Type::Any)),
        (
            "childOrder",
            TableField::required(Type::array(child_names.clone())),
        ),
        ("origins", TableField::required(Type::Any)),
        ("looping", TableField::required(Type::Any)),
        (
            "player",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("initial", animation_names),
                ],
                1,
                [player],
            ),
        ),
    ])
}

fn sprite_animation_type() -> Type {
    Type::table([
        ("frames", TableField::required(Type::array(Type::Any))),
        (
            "durations",
            TableField::required(Type::array(Type::number())),
        ),
        (
            "intervals",
            TableField::required(Type::array(Type::number())),
        ),
        ("totalDuration", TableField::required(Type::number())),
        ("timer", TableField::required(Type::number())),
        ("position", TableField::required(Type::number())),
        ("status", TableField::required(Type::string())),
        ("flippedH", TableField::required(Type::boolean())),
        ("flippedV", TableField::required(Type::boolean())),
        (
            "update",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("dt", Type::number()),
                ],
                2,
                [Type::nil()],
            ),
        ),
        (
            "pause",
            method_field(
                [crate::FunctionParameter::named("self", Type::Any)],
                1,
                [Type::nil()],
            ),
        ),
        (
            "resume",
            method_field(
                [crate::FunctionParameter::named("self", Type::Any)],
                1,
                [Type::nil()],
            ),
        ),
        (
            "gotoFrame",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("frame", Type::number()),
                ],
                2,
                [Type::nil()],
            ),
        ),
        (
            "getDimensions",
            method_field(
                [crate::FunctionParameter::named("self", Type::Any)],
                1,
                [Type::number(), Type::number()],
            ),
        ),
    ])
}

fn sprite_player_type(spec: &SpriteSpec) -> Type {
    let animation_name = literal_union_or_string(&spec.animations);
    let marker_name = literal_union_or_string(&spec.markers);
    let child_name = literal_union_or_string(&spec.children);
    let optional_animation_name = Type::union([animation_name.clone(), Type::nil()]);
    let marker_callback = Type::Function(FunctionType::new_signature(
        [
            crate::FunctionParameter::named("player", Type::Any),
            crate::FunctionParameter::named("marker", marker_name.clone()),
            crate::FunctionParameter::named("animation", animation_name.clone()),
            crate::FunctionParameter::named("frame", Type::number()),
        ],
        4,
        None,
        [Type::nil()],
    ));

    Type::table([
        ("name", TableField::required(optional_animation_name)),
        ("playing", TableField::required(Type::boolean())),
        ("speed", TableField::required(Type::number())),
        ("children", TableField::required(Type::Any)),
        (
            "play",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("name", animation_name.clone()),
                    crate::FunctionParameter::named("restart", Type::boolean()),
                ],
                2,
                [Type::nil()],
            ),
        ),
        (
            "stop",
            method_field(
                [crate::FunctionParameter::named("self", Type::Any)],
                1,
                [Type::nil()],
            ),
        ),
        (
            "resume",
            method_field(
                [crate::FunctionParameter::named("self", Type::Any)],
                1,
                [Type::nil()],
            ),
        ),
        (
            "gotoAndPlay",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("name", animation_name.clone()),
                    crate::FunctionParameter::named("frame", Type::number()),
                ],
                2,
                [Type::nil()],
            ),
        ),
        (
            "gotoAndStop",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("name", animation_name),
                    crate::FunctionParameter::named("frame", Type::number()),
                ],
                2,
                [Type::nil()],
            ),
        ),
        (
            "seekFrame",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("frame", Type::number()),
                    crate::FunctionParameter::named("emitMarkers", Type::boolean()),
                ],
                2,
                [Type::nil()],
            ),
        ),
        (
            "seek",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("progress", Type::number()),
                    crate::FunctionParameter::named("emitMarkers", Type::boolean()),
                ],
                2,
                [Type::nil()],
            ),
        ),
        (
            "update",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("dt", Type::number()),
                ],
                2,
                [Type::nil()],
            ),
        ),
        (
            "draw",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("x", Type::number()),
                    crate::FunctionParameter::named("y", Type::number()),
                    crate::FunctionParameter::named("rotation", Type::number()),
                    crate::FunctionParameter::named("scaleX", Type::number()),
                    crate::FunctionParameter::named("scaleY", Type::number()),
                    crate::FunctionParameter::named("originX", Type::number()),
                    crate::FunctionParameter::named("originY", Type::number()),
                    crate::FunctionParameter::named("shearX", Type::number()),
                    crate::FunctionParameter::named("shearY", Type::number()),
                ],
                3,
                [Type::nil()],
            ),
        ),
        (
            "addToBatch",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("batch", Type::alias("SpriteBatch")),
                    crate::FunctionParameter::named("x", Type::number()),
                    crate::FunctionParameter::named("y", Type::number()),
                    crate::FunctionParameter::named("rotation", Type::number()),
                    crate::FunctionParameter::named("scaleX", Type::number()),
                    crate::FunctionParameter::named("scaleY", Type::number()),
                    crate::FunctionParameter::named("originX", Type::number()),
                    crate::FunctionParameter::named("originY", Type::number()),
                    crate::FunctionParameter::named("shearX", Type::number()),
                    crate::FunctionParameter::named("shearY", Type::number()),
                ],
                4,
                [Type::union([Type::number(), Type::nil()])],
            ),
        ),
        (
            "on",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("marker", marker_name.clone()),
                    crate::FunctionParameter::named("callback", marker_callback),
                ],
                3,
                [Type::nil()],
            ),
        ),
        (
            "off",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("marker", marker_name),
                ],
                2,
                [Type::nil()],
            ),
        ),
        (
            "child",
            method_field(
                [
                    crate::FunctionParameter::named("self", Type::Any),
                    crate::FunctionParameter::named("name", child_name),
                ],
                2,
                [Type::alias(PLAYER_TYPE_NAME)],
            ),
        ),
    ])
}

fn method_field(
    parameters: impl IntoIterator<Item = crate::FunctionParameter>,
    required_parameter_count: usize,
    returns: impl IntoIterator<Item = Type>,
) -> TableField {
    TableField::required(Type::Function(FunctionType::new_signature(
        parameters,
        required_parameter_count,
        None,
        returns,
    )))
}

fn literal_union_or_string(values: &[String]) -> Type {
    match values {
        [] => Type::string(),
        [value] => Type::string_literal(value),
        _ => Type::union(values.iter().map(Type::string_literal)),
    }
}

pub(crate) fn builtin_aliases() -> Vec<(String, Type)> {
    vec![
        (TYPE_NAME.to_owned(), sprite_sheet_type()),
        (ANIMATION_TYPE_NAME.to_owned(), sprite_animation_type()),
        (
            PLAYER_TYPE_NAME.to_owned(),
            sprite_player_type(&SpriteSpec::default()),
        ),
        (CONFIG_TYPE_NAME.to_owned(), sprite_sheet_config_type()),
        (
            FRAME_CONFIG_TYPE_NAME.to_owned(),
            sprite_frame_config_type(),
        ),
        (
            ANIMATION_CONFIG_TYPE_NAME.to_owned(),
            sprite_animation_config_type(),
        ),
        (
            ORIGIN_CONFIG_TYPE_NAME.to_owned(),
            sprite_origin_config_type(),
        ),
        (
            RANGE_CONFIG_TYPE_NAME.to_owned(),
            sprite_frame_range_config_type(),
        ),
        (
            MARKER_CONFIG_TYPE_NAME.to_owned(),
            sprite_marker_config_type(),
        ),
        (
            FRAME_METADATA_CONFIG_TYPE_NAME.to_owned(),
            sprite_frame_metadata_config_type(),
        ),
        (
            CHILD_CONFIG_TYPE_NAME.to_owned(),
            sprite_child_config_type(),
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
        (
            "origin",
            TableField::optional(Type::alias(ORIGIN_CONFIG_TYPE_NAME)),
        ),
        (
            "animations",
            TableField::required(Type::dictionary(
                Type::string(),
                Type::alias(ANIMATION_CONFIG_TYPE_NAME),
            )),
        ),
        (
            "children",
            TableField::optional(Type::dictionary(
                Type::string(),
                Type::alias(CHILD_CONFIG_TYPE_NAME),
            )),
        ),
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

fn sprite_origin_config_type() -> Type {
    Type::table([
        ("x", TableField::optional(Type::number())),
        ("y", TableField::optional(Type::number())),
    ])
}

fn sprite_frame_range_config_type() -> Type {
    Type::table([
        ("from", TableField::optional(Type::number())),
        ("to", TableField::optional(Type::number())),
        ("column", TableField::optional(Type::number())),
        ("row", TableField::optional(Type::number())),
    ])
}

fn sprite_marker_config_type() -> Type {
    Type::table([
        ("frame", TableField::required(Type::number())),
        ("name", TableField::required(Type::string())),
    ])
}

fn sprite_frame_metadata_config_type() -> Type {
    Type::table([
        (
            "origin",
            TableField::optional(Type::alias(ORIGIN_CONFIG_TYPE_NAME)),
        ),
        ("trimX", TableField::optional(Type::number())),
        ("trimY", TableField::optional(Type::number())),
        ("sourceWidth", TableField::optional(Type::number())),
        ("sourceHeight", TableField::optional(Type::number())),
    ])
}

fn sprite_child_config_type() -> Type {
    Type::table([
        ("sheet", TableField::required(Type::alias(TYPE_NAME))),
        ("initial", TableField::optional(Type::string())),
        ("x", TableField::optional(Type::number())),
        ("y", TableField::optional(Type::number())),
        ("rotation", TableField::optional(Type::number())),
        ("scaleX", TableField::optional(Type::number())),
        ("scaleY", TableField::optional(Type::number())),
    ])
}

fn sprite_animation_config_type() -> Type {
    Type::table([
        ("from", TableField::optional(Type::number())),
        ("to", TableField::optional(Type::number())),
        ("column", TableField::optional(Type::number())),
        ("row", TableField::optional(Type::number())),
        (
            "frames",
            TableField::optional(Type::array(Type::alias(RANGE_CONFIG_TYPE_NAME))),
        ),
        (
            "frame",
            TableField::optional(Type::alias(FRAME_CONFIG_TYPE_NAME)),
        ),
        (
            "origin",
            TableField::optional(Type::alias(ORIGIN_CONFIG_TYPE_NAME)),
        ),
        (
            "markers",
            TableField::optional(Type::array(Type::alias(MARKER_CONFIG_TYPE_NAME))),
        ),
        (
            "metadata",
            TableField::optional(Type::array(Type::alias(FRAME_METADATA_CONFIG_TYPE_NAME))),
        ),
        ("fps", TableField::optional(Type::number())),
        ("durations", TableField::optional(Type::Any)),
        (
            "onLoop",
            TableField::optional(Type::union([
                Type::string(),
                Type::Function(FunctionType::new_signature(
                    [
                        crate::FunctionParameter::named("animation", Type::Any),
                        crate::FunctionParameter::named("loops", Type::number()),
                    ],
                    2,
                    None,
                    [Type::Any],
                )),
            ])),
        ),
        ("loop", TableField::optional(Type::boolean())),
    ])
}

pub(crate) fn constructor_type() -> FunctionType {
    FunctionType::new([Type::alias(CONFIG_TYPE_NAME)], Type::alias(TYPE_NAME))
}

pub(crate) fn sprite_sheet_type_from_call(call: &FunctionCall) -> Option<Type> {
    let config = first_table_argument(call)?;
    let animations = named_field(config, "animations").and_then(table_expression)?;
    let mut spec = SpriteSpec {
        exact: true,
        ..SpriteSpec::default()
    };
    if let Some(children) = named_field(config, "children").and_then(table_expression) {
        for field in children.fields().iter() {
            let Field::NameKey { key, .. } = field else {
                return Some(Type::alias(TYPE_NAME));
            };
            spec.children.push(token_text(key));
        }
    }

    for field in animations.fields().iter() {
        let Field::NameKey { key, value, .. } = field else {
            return Some(Type::alias(TYPE_NAME));
        };
        spec.animations.push(token_text(key));

        let Some(animation) = table_expression(value) else {
            continue;
        };
        let Some(markers) = named_field(animation, "markers").and_then(table_expression) else {
            continue;
        };
        for marker in markers.fields().iter() {
            let marker = match marker {
                Field::NoKey(value) => table_expression(value),
                _ => None,
            };
            let Some(name) = marker
                .and_then(|marker| named_field(marker, "name"))
                .and_then(string_expression)
                .map(|(name, _)| name)
            else {
                continue;
            };
            if !spec.markers.contains(&name) {
                spec.markers.push(name);
            }
        }
    }

    (!spec.animations.is_empty()).then(|| sprite_sheet_type_for_spec(&spec))
}

pub(crate) fn direct_type_is_sprite_player(candidate: &Type) -> bool {
    let Type::Table(table) = candidate else {
        return false;
    };

    table.field("play").is_some()
        && table.field("gotoAndPlay").is_some()
        && table.field("seekFrame").is_some()
        && table.field("draw").is_some()
}

pub(crate) fn type_is_sprite_sheet(environment: &crate::TypeEnvironment, candidate: &Type) -> bool {
    let Type::Table(table) = environment.resolve_aliases(candidate) else {
        return false;
    };

    table.field("animations").is_some()
        && table.field("animationOrder").is_some()
        && table.field("player").is_some()
}

pub(crate) fn type_is_sprite_player(
    environment: &crate::TypeEnvironment,
    candidate: &Type,
) -> bool {
    direct_type_is_sprite_player(&environment.resolve_aliases(candidate))
}

pub(crate) fn typed_member(
    environment: &crate::TypeEnvironment,
    receiver_type: &Type,
    member: &str,
) -> Option<crate::TypedFunction> {
    let Type::Table(player) = environment.resolve_aliases(receiver_type) else {
        return None;
    };
    if !direct_type_is_sprite_player(&Type::Table(player.clone())) {
        return None;
    }
    let Type::Function(function) = &player.field(member)?.ty else {
        return None;
    };

    Some(crate::TypedFunction {
        name: Some(format!("{PLAYER_TYPE_NAME}:{member}")),
        parameters: function
            .parameters
            .iter()
            .map(|parameter| TypedParameter {
                name: parameter.name.clone().unwrap_or_else(|| "value".to_owned()),
                ty: parameter.ty.clone(),
                documentation: None,
            })
            .collect(),
        parameter_count: function.parameters.len(),
        required_parameter_count: function.required_parameter_count,
        has_varargs: function.vararg_type.is_some(),
        vararg_type: function.vararg_type.as_deref().cloned(),
        return_type: function.return_types_or_any().first().cloned(),
        return_types: function.return_types_or_any().to_vec(),
        documentation: Some(player_method_documentation(member).to_owned()),
        overloads: Vec::new(),
    })
}

fn player_method_documentation(member: &str) -> &'static str {
    match member {
        "play" => "Play a configured animation, optionally restarting it.",
        "stop" => "Pause playback on the current frame.",
        "resume" => "Resume the current animation.",
        "gotoAndPlay" => "Select an animation and continue from a chosen frame.",
        "gotoAndStop" => "Select an animation and pause on a chosen frame.",
        "seekFrame" => "Seek to a one-based frame, optionally emitting its markers.",
        "seek" => "Seek by normalized animation progress.",
        "update" => "Advance the player using elapsed time and its signed speed.",
        "draw" => "Draw the current frame using its configured registration point.",
        "addToBatch" => "Add the current leaf frame and transform to a caller-owned SpriteBatch.",
        "on" => "Subscribe to a configured frame-marker event.",
        "off" => "Remove callbacks for a configured frame-marker event.",
        "child" => "Return a configured nested sprite player's controller.",
        _ => "Sprite player operation.",
    }
}

pub(crate) fn method_string_completion_items(
    environment: &crate::TypeEnvironment,
    receiver_type: &Type,
    method: &str,
    active_parameter: usize,
) -> Option<Vec<SpriteMethodCompletionItem>> {
    if active_parameter != 0 {
        return None;
    }
    let field = match method {
        "play" | "gotoAndPlay" | "gotoAndStop" => "play",
        "on" | "off" => "on",
        "child" => "child",
        _ => return None,
    };
    let Type::Table(player) = environment.resolve_aliases(receiver_type) else {
        return None;
    };
    if !direct_type_is_sprite_player(&Type::Table(player.clone())) {
        return None;
    }
    let Type::Function(function) = &player.field(field)?.ty else {
        return None;
    };
    let values = string_literal_values(&function.parameters.get(1)?.ty);
    if values.is_empty() {
        return None;
    }
    let detail = match field {
        "play" => "sprite animation",
        "on" => "sprite marker",
        _ => "nested sprite clip",
    };

    Some(
        values
            .into_iter()
            .map(|value| SpriteMethodCompletionItem {
                label: value.clone(),
                detail: detail.to_owned(),
                documentation: format!("Configured {detail} `{value}`."),
            })
            .collect(),
    )
}

fn string_literal_values(ty: &Type) -> Vec<String> {
    match ty {
        Type::StringLiteral(value) => vec![value.clone()],
        Type::Union(types) => types.iter().flat_map(string_literal_values).collect(),
        _ => Vec::new(),
    }
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
    Origin,
    Animations,
    Animation,
    Range,
    Marker,
    Metadata,
    Children,
    Child,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpriteConfigCompletionItem {
    pub(crate) label: &'static str,
    pub(crate) detail: &'static str,
    pub(crate) documentation: &'static str,
    pub(crate) insert_text: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SpriteMethodCompletionItem {
    pub(crate) label: String,
    pub(crate) detail: String,
    pub(crate) documentation: String,
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
            .origin_config_spans
            .iter()
            .any(|span| span_contains_position(*span, position))
        {
            return Some(SpriteConfigContext::Origin);
        }
        if declaration
            .range_config_spans
            .iter()
            .any(|span| span_contains_position(*span, position))
        {
            return Some(SpriteConfigContext::Range);
        }
        if declaration
            .marker_config_spans
            .iter()
            .any(|span| span_contains_position(*span, position))
        {
            return Some(SpriteConfigContext::Marker);
        }
        if declaration
            .metadata_config_spans
            .iter()
            .any(|span| span_contains_position(*span, position))
        {
            return Some(SpriteConfigContext::Metadata);
        }
        if declaration
            .child_config_spans
            .iter()
            .any(|span| span_contains_position(*span, position))
        {
            return Some(SpriteConfigContext::Child);
        }
        if declaration
            .children_span
            .is_some_and(|span| span_contains_position(span, position))
        {
            return Some(SpriteConfigContext::Children);
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
                label: "origin",
                detail: ORIGIN_CONFIG_TYPE_NAME,
                documentation: "Default registration point for player drawing.",
                insert_text: "origin = { x = ${1:0}, y = ${2:0} },",
            },
            SpriteConfigCompletionItem {
                label: "animations",
                detail: "{ [string]: SpriteAnimationConfig }",
                documentation: "Named animation definitions built from the sprite grid.",
                insert_text: "animations = {\n  ${1:idle} = { from = ${2:0}, to = ${3:3}, row = ${4:0}, fps = ${5:8} },\n},",
            },
            SpriteConfigCompletionItem {
                label: "children",
                detail: "{ [string]: SpriteChildConfig }",
                documentation: "Optional nested sprite sheets with local display transforms.",
                insert_text: "children = {\n  ${1:clip} = { sheet = ${2:ChildSheet}, initial = \"${3:idle}\", x = ${4:0}, y = ${5:0} },\n},",
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
        SpriteConfigContext::Origin => &[
            SpriteConfigCompletionItem {
                label: "x",
                detail: "number",
                documentation: "Horizontal registration point measured from the untrimmed frame's left edge.",
                insert_text: "x = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "y",
                detail: "number",
                documentation: "Vertical registration point measured from the untrimmed frame's top edge.",
                insert_text: "y = ${1:0},",
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
                label: "frames",
                detail: "SpriteFrameRangeConfig[]",
                documentation: "Ordered row/column ranges for animations that cross grid rows.",
                insert_text: "frames = {\n  { from = ${1:0}, to = ${2:7}, row = ${3:0} },\n  { from = ${4:0}, to = ${5:7}, row = ${6:1} },\n},",
            },
            SpriteConfigCompletionItem {
                label: "frame",
                detail: FRAME_CONFIG_TYPE_NAME,
                documentation: "Optional per-animation grid size and origin override.",
                insert_text: "frame = { width = ${1:16}, height = ${2:16}, left = ${3:0}, top = ${4:0}, border = ${5:0} },",
            },
            SpriteConfigCompletionItem {
                label: "origin",
                detail: ORIGIN_CONFIG_TYPE_NAME,
                documentation: "Optional registration-point override for this animation.",
                insert_text: "origin = { x = ${1:0}, y = ${2:0} },",
            },
            SpriteConfigCompletionItem {
                label: "markers",
                detail: "SpriteMarkerConfig[]",
                documentation: "Named events attached to zero-based animation-local frame indexes.",
                insert_text: "markers = {\n  { frame = ${1:0}, name = \"${2:event}\" },\n},",
            },
            SpriteConfigCompletionItem {
                label: "metadata",
                detail: "SpriteFrameMetadataConfig[]",
                documentation: "Optional per-frame registration, trim offset, and untrimmed source size metadata.",
                insert_text: "metadata = {\n  { origin = { x = ${1:0}, y = ${2:0} }, trimX = ${3:0}, trimY = ${4:0}, sourceWidth = ${5:16}, sourceHeight = ${6:16} },\n},",
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
        SpriteConfigContext::Range => &[
            SpriteConfigCompletionItem {
                label: "from",
                detail: "number",
                documentation: "Zero-based first column in this ordered frame range.",
                insert_text: "from = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "to",
                detail: "number",
                documentation: "Zero-based last column in this ordered frame range.",
                insert_text: "to = ${1:7},",
            },
            SpriteConfigCompletionItem {
                label: "column",
                detail: "number",
                documentation: "Zero-based single-column selector.",
                insert_text: "column = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "row",
                detail: "number",
                documentation: "Zero-based grid row for this range.",
                insert_text: "row = ${1:0},",
            },
        ],
        SpriteConfigContext::Marker => &[
            SpriteConfigCompletionItem {
                label: "frame",
                detail: "number",
                documentation: "Zero-based animation-local frame index that emits the marker.",
                insert_text: "frame = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "name",
                detail: "string",
                documentation: "Marker event name checked by SpritePlayer:on and off.",
                insert_text: "name = \"${1:event}\",",
            },
        ],
        SpriteConfigContext::Metadata => &[
            SpriteConfigCompletionItem {
                label: "origin",
                detail: ORIGIN_CONFIG_TYPE_NAME,
                documentation: "Optional registration-point override for this frame.",
                insert_text: "origin = { x = ${1:0}, y = ${2:0} },",
            },
            SpriteConfigCompletionItem {
                label: "trimX",
                detail: "number",
                documentation: "Horizontal offset of the trimmed atlas rectangle in the untrimmed source frame.",
                insert_text: "trimX = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "trimY",
                detail: "number",
                documentation: "Vertical offset of the trimmed atlas rectangle in the untrimmed source frame.",
                insert_text: "trimY = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "sourceWidth",
                detail: "number",
                documentation: "Width of the frame before atlas trimming.",
                insert_text: "sourceWidth = ${1:16},",
            },
            SpriteConfigCompletionItem {
                label: "sourceHeight",
                detail: "number",
                documentation: "Height of the frame before atlas trimming.",
                insert_text: "sourceHeight = ${1:16},",
            },
        ],
        SpriteConfigContext::Children => &[SpriteConfigCompletionItem {
            label: "child",
            detail: CHILD_CONFIG_TYPE_NAME,
            documentation: "Named nested sprite sheet and its local transform.",
            insert_text: "${1:name} = { sheet = ${2:ChildSheet}, initial = \"${3:idle}\", x = ${4:0}, y = ${5:0} },",
        }],
        SpriteConfigContext::Child => &[
            SpriteConfigCompletionItem {
                label: "sheet",
                detail: TYPE_NAME,
                documentation: "Sprite sheet used by this nested clip.",
                insert_text: "sheet = ${1:ChildSheet},",
            },
            SpriteConfigCompletionItem {
                label: "initial",
                detail: "string",
                documentation: "Optional initial animation for the child player.",
                insert_text: "initial = \"${1:idle}\",",
            },
            SpriteConfigCompletionItem {
                label: "x",
                detail: "number",
                documentation: "Horizontal child offset in parent-local coordinates.",
                insert_text: "x = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "y",
                detail: "number",
                documentation: "Vertical child offset in parent-local coordinates.",
                insert_text: "y = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "rotation",
                detail: "number",
                documentation: "Child rotation in radians.",
                insert_text: "rotation = ${1:0},",
            },
            SpriteConfigCompletionItem {
                label: "scaleX",
                detail: "number",
                documentation: "Horizontal child scale.",
                insert_text: "scaleX = ${1:1},",
            },
            SpriteConfigCompletionItem {
                label: "scaleY",
                detail: "number",
                documentation: "Vertical child scale.",
                insert_text: "scaleY = ${1:1},",
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
    let origin_table = named_field(table, "origin").and_then(table_expression);
    let origin = origin_table
        .and_then(parse_origin_table)
        .unwrap_or_default();
    let animations_table = named_field(table, "animations").and_then(table_expression);
    let animations = animations_table
        .map(parse_animation_table)
        .unwrap_or_default();
    let children_table = named_field(table, "children").and_then(table_expression);
    let children = children_table.map(parse_children).unwrap_or_default();
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
    let mut origin_config_spans = origin_table
        .and_then(node_span)
        .into_iter()
        .collect::<Vec<_>>();
    let mut range_config_spans = Vec::new();
    let mut marker_config_spans = Vec::new();
    let mut metadata_config_spans = Vec::new();
    if let Some(animations_table) = animations_table {
        origin_config_spans.extend(animation_nested_table_spans(animations_table, "origin"));
        origin_config_spans.extend(animation_list_nested_table_spans(
            animations_table,
            "metadata",
            "origin",
        ));
        range_config_spans.extend(animation_list_item_spans(animations_table, "frames"));
        marker_config_spans.extend(animation_list_item_spans(animations_table, "markers"));
        metadata_config_spans.extend(animation_list_item_spans(animations_table, "metadata"));
    }

    SpriteDeclaration {
        name,
        image_path,
        frame,
        origin,
        animations,
        children,
        span,
        name_span,
        image_span,
        config_span: node_span(table).unwrap_or(span),
        frame_config_spans,
        origin_config_spans,
        range_config_spans,
        marker_config_spans,
        metadata_config_spans,
        children_span: children_table.and_then(node_span),
        child_config_spans: children_table
            .map(animation_table_spans)
            .unwrap_or_default(),
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

fn parse_origin_table(table: &ast::TableConstructor) -> Option<SpriteOrigin> {
    Some(SpriteOrigin {
        x: named_field(table, "x")
            .and_then(i32_expression)
            .map(|(value, _)| value)
            .unwrap_or(0),
        y: named_field(table, "y")
            .and_then(i32_expression)
            .map(|(value, _)| value)
            .unwrap_or(0),
    })
}

fn parse_animation_table(table: &ast::TableConstructor) -> Vec<SpriteAnimation> {
    table
        .fields()
        .iter()
        .filter_map(parse_animation_field)
        .collect()
}

fn parse_children(table: &ast::TableConstructor) -> Vec<SpriteChild> {
    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NameKey { key, value, .. } = field else {
                return None;
            };
            let child = table_expression(value)?;

            Some(SpriteChild {
                name: token_text(key),
                sheet: named_field(child, "sheet")
                    .and_then(expression_text)
                    .map(|(value, _)| value),
                initial: named_field(child, "initial")
                    .and_then(expression_text)
                    .map(|(value, _)| value),
                x: child_expression_or(child, "x", "0"),
                y: child_expression_or(child, "y", "0"),
                rotation: child_expression_or(child, "rotation", "0"),
                scale_x: child_expression_or(child, "scaleX", "1"),
                scale_y: child_expression_or(child, "scaleY", "1"),
                span: node_span(value)?,
                name_span: node_span(key)?,
            })
        })
        .collect()
}

fn child_expression_or(table: &ast::TableConstructor, name: &str, fallback: &str) -> String {
    named_field(table, name)
        .and_then(expression_text)
        .map(|(value, _)| value)
        .unwrap_or_else(|| fallback.to_owned())
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
    animation_nested_table_spans(table, "frame")
}

fn animation_nested_table_spans(
    table: &ast::TableConstructor,
    field_name: &str,
) -> Vec<SourceSpan> {
    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NameKey { value, .. } = field else {
                return None;
            };
            let animation = table_expression(value)?;

            named_field(animation, field_name)
                .and_then(table_expression)
                .and_then(node_span)
        })
        .collect()
}

fn animation_list_item_spans(table: &ast::TableConstructor, field_name: &str) -> Vec<SourceSpan> {
    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NameKey { value, .. } = field else {
                return None;
            };
            table_expression(value)
        })
        .filter_map(|animation| named_field(animation, field_name).and_then(table_expression))
        .flat_map(|items| {
            items.fields().iter().filter_map(|field| match field {
                Field::NoKey(value) => table_expression(value).and_then(node_span),
                _ => None,
            })
        })
        .collect()
}

fn animation_list_nested_table_spans(
    table: &ast::TableConstructor,
    list_name: &str,
    field_name: &str,
) -> Vec<SourceSpan> {
    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NameKey { value, .. } = field else {
                return None;
            };
            table_expression(value)
        })
        .filter_map(|animation| named_field(animation, list_name).and_then(table_expression))
        .flat_map(|items| {
            items.fields().iter().filter_map(|field| {
                let Field::NoKey(value) = field else {
                    return None;
                };
                table_expression(value)
                    .and_then(|item| named_field(item, field_name))
                    .and_then(table_expression)
                    .and_then(node_span)
            })
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
    let row = named_field(table, "row")
        .and_then(u32_expression)
        .map(|(value, _)| value)
        .unwrap_or(0);
    let ranges_expression = named_field(table, "frames");
    let explicit_ranges = ranges_expression
        .and_then(table_expression)
        .map(parse_frame_ranges)
        .unwrap_or_default();
    let top_level_range = match (column, range) {
        (Some(column), _) => Some(SpriteFrameRange {
            start_frame: column,
            end_frame: column,
            row,
        }),
        (None, Some(((start_frame, _), (end_frame, _)))) => Some(SpriteFrameRange {
            start_frame,
            end_frame,
            row,
        }),
        (None, None) => None,
    };
    let ranges = if explicit_ranges.is_empty() {
        vec![top_level_range?]
    } else {
        explicit_ranges
    };
    let first_range = ranges.first().copied()?;
    let (start_frame, end_frame, row) = (
        first_range.start_frame,
        first_range.end_frame,
        first_range.row,
    );
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
    let frame_count = ranges.iter().map(frame_range_len).sum::<usize>();
    let duration_values = durations_expression
        .and_then(|durations| static_duration_values(durations, frame_count))
        .or_else(|| {
            fps.as_ref().and_then(|(fps, _)| {
                let fps = fps.parse::<f64>().ok()?;
                (fps > 0.0).then(|| vec![(1.0 / fps).to_string(); frame_count])
            })
        })
        .unwrap_or_default();
    let markers = named_field(table, "markers")
        .and_then(table_expression)
        .map(parse_markers)
        .unwrap_or_default();
    let metadata = named_field(table, "metadata")
        .and_then(table_expression)
        .map(parse_frame_metadata_list)
        .unwrap_or_default();

    Some(SpriteAnimation {
        name: token_text(key),
        start_frame,
        end_frame,
        row,
        column,
        ranges,
        frame: named_field(table, "frame")
            .and_then(table_expression)
            .and_then(parse_frame_table),
        origin: named_field(table, "origin")
            .and_then(table_expression)
            .and_then(parse_origin_table),
        markers,
        metadata,
        fps: fps.as_ref().map(|(value, _)| value.clone()),
        durations: durations.as_ref().map(|(value, _)| value.clone()),
        duration_preview: durations_expression.and_then(duration_preview_value),
        duration_values,
        on_loop: on_loop.as_ref().map(|(value, _)| value.clone()),
        looping,
        span: node_span(value)?,
        name_span,
        fps_span: fps.map(|(_, span)| span),
        durations_span: durations.map(|(_, span)| span),
        has_selector_conflict: (column.is_some() && range.is_some())
            || (ranges_expression.is_some() && top_level_range.is_some()),
        has_timing_conflict: named_field(table, "fps").is_some()
            && named_field(table, "durations").is_some(),
        has_loop_conflict: named_field(table, "onLoop").is_some()
            && named_field(table, "loop").is_some(),
    })
}

fn parse_frame_ranges(table: &ast::TableConstructor) -> Vec<SpriteFrameRange> {
    table
        .fields()
        .iter()
        .filter_map(|field| match field {
            Field::NoKey(value) => table_expression(value).and_then(parse_frame_range),
            _ => None,
        })
        .collect()
}

fn parse_frame_range(table: &ast::TableConstructor) -> Option<SpriteFrameRange> {
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

    Some(SpriteFrameRange {
        start_frame,
        end_frame,
        row: named_field(table, "row")
            .and_then(u32_expression)
            .map(|(value, _)| value)
            .unwrap_or(0),
    })
}

fn frame_range_len(range: &SpriteFrameRange) -> usize {
    range.start_frame.abs_diff(range.end_frame) as usize + 1
}

fn parse_markers(table: &ast::TableConstructor) -> Vec<SpriteMarker> {
    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NoKey(value) = field else {
                return None;
            };
            let marker = table_expression(value)?;
            let (frame, _) = named_field(marker, "frame").and_then(u32_expression)?;
            let (name, span) = named_field(marker, "name").and_then(string_expression)?;

            Some(SpriteMarker { frame, name, span })
        })
        .collect()
}

fn parse_frame_metadata_list(table: &ast::TableConstructor) -> Vec<SpriteFrameMetadata> {
    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NoKey(value) = field else {
                return None;
            };
            let metadata = table_expression(value)?;

            Some(SpriteFrameMetadata {
                origin: named_field(metadata, "origin")
                    .and_then(table_expression)
                    .and_then(parse_origin_table),
                trim_x: named_field(metadata, "trimX")
                    .and_then(i32_expression)
                    .map(|(value, _)| value)
                    .unwrap_or(0),
                trim_y: named_field(metadata, "trimY")
                    .and_then(i32_expression)
                    .map(|(value, _)| value)
                    .unwrap_or(0),
                source_width: named_field(metadata, "sourceWidth")
                    .and_then(u32_expression)
                    .map(|(value, _)| value),
                source_height: named_field(metadata, "sourceHeight")
                    .and_then(u32_expression)
                    .map(|(value, _)| value),
            })
        })
        .collect()
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

fn i32_expression(expression: &Expression) -> Option<(i32, SourceSpan)> {
    let (text, span) = expression_text(expression)?;
    let number = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();

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

fn static_duration_values(expression: &Expression, frame_count: usize) -> Option<Vec<String>> {
    if let Some((duration, _)) = number_string_expression(expression) {
        let parsed = duration.parse::<f64>().ok()?;
        return (parsed > 0.0).then(|| vec![duration; frame_count]);
    }

    let table = table_expression(expression)?;
    let mut values = vec![None; frame_count];
    let mut sequential_index = 0usize;

    for field in table.fields().iter() {
        match field {
            Field::NoKey(value) => {
                let duration = number_string_expression(value)?.0;
                if duration
                    .parse::<f64>()
                    .ok()
                    .is_none_or(|value| value <= 0.0)
                    || sequential_index >= values.len()
                {
                    return None;
                }
                values[sequential_index] = Some(duration);
                sequential_index += 1;
            }
            Field::ExpressionKey { key, value, .. } => {
                let (interval, _) = string_expression(key)?;
                let duration = number_string_expression(value)?.0;
                if duration
                    .parse::<f64>()
                    .ok()
                    .is_none_or(|value| value <= 0.0)
                {
                    return None;
                }
                let (start, end) = parse_duration_interval(&interval)?;
                let step = if start <= end { 1 } else { -1 };
                let mut index = start;
                loop {
                    let zero_based = usize::try_from(index.checked_sub(1)?).ok()?;
                    *values.get_mut(zero_based)? = Some(duration.clone());
                    if index == end {
                        break;
                    }
                    index += step;
                }
            }
            _ => return None,
        }
    }

    values.into_iter().collect()
}

fn parse_duration_interval(value: &str) -> Option<(i64, i64)> {
    let value = value.trim();
    if let Some((start, end)) = value.split_once('-') {
        return Some((start.trim().parse().ok()?, end.trim().parse().ok()?));
    }
    let index = value.parse().ok()?;
    Some((index, index))
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

        for range in &animation.ranges {
            if range.end_frame < range.start_frame {
                diagnostics.push(sprite_diagnostic(
                    path,
                    animation.span,
                    format!(
                        "sprite animation `{}` has invalid frame range {}..{}",
                        animation.name, range.start_frame, range.end_frame
                    ),
                ));
            }
        }

        if animation.has_selector_conflict {
            diagnostics.push(sprite_diagnostic(
                path,
                animation.span,
                format!(
                    "sprite animation `{}` cannot combine `column` with `from`/`to`, or `frames` with top-level selectors",
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

        let frame_count = animation.ranges.iter().map(frame_range_len).sum::<usize>();
        if !animation.metadata.is_empty() && animation.metadata.len() != frame_count {
            diagnostics.push(sprite_diagnostic(
                path,
                animation.span,
                format!(
                    "sprite animation `{}` has {} metadata entries, but the animation has {} frame(s)",
                    animation.name,
                    animation.metadata.len(),
                    frame_count
                ),
            ));
        }
        if animation
            .metadata
            .iter()
            .any(|metadata| metadata.source_width == Some(0) || metadata.source_height == Some(0))
        {
            diagnostics.push(sprite_diagnostic(
                path,
                animation.span,
                format!(
                    "sprite animation `{}` metadata source size must be greater than zero",
                    animation.name
                ),
            ));
        }
        for marker in &animation.markers {
            if marker.frame as usize >= frame_count {
                diagnostics.push(sprite_diagnostic(
                    path,
                    marker.span,
                    format!(
                        "sprite animation `{}` marker `{}` targets frame {}, but the animation has {} frame(s)",
                        animation.name, marker.name, marker.frame, frame_count
                    ),
                ));
            }
        }
    }

    let mut child_names = BTreeSet::new();
    for child in &declaration.children {
        if !child_names.insert(child.name.as_str()) {
            diagnostics.push(sprite_diagnostic(
                path,
                child.name_span,
                format!("sprite child `{}` is defined more than once", child.name),
            ));
        }
        if child.sheet.is_none() {
            diagnostics.push(sprite_diagnostic(
                path,
                child.span,
                format!("sprite child `{}` requires a `sheet`", child.name),
            ));
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
        "local tuasprite = require(\"{PLAYER_RUNTIME_MODULE}\")\n"
    ));
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
    output.push_str("  __tuaStaticAnimations = true,\n");
    output.push_str(&format!("  image = {image_local},\n"));
    output.push_str(&format!("  grid = {grid_local},\n"));
    output.push_str(&format!(
        "  frame = {{ width = {}, height = {}, left = {}, top = {}, border = {} }},\n",
        frame.width, frame.height, frame.left, frame.top, frame.border
    ));
    output.push_str(&format!(
        "  origin = {{ x = {}, y = {} }},\n",
        declaration.origin.x, declaration.origin.y
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
    output.push_str("  animationOrder = {\n");
    for animation in &declaration.animations {
        output.push_str(&format!("    {},\n", lua_string_literal(&animation.name)));
    }
    output.push_str("  },\n");
    output.push_str("  origins = {\n");
    for animation in &declaration.animations {
        let origin = animation.origin.unwrap_or(declaration.origin);
        output.push_str(&format!(
            "    {} = {{ x = {}, y = {} }},\n",
            animation.name, origin.x, origin.y
        ));
    }
    output.push_str("  },\n");
    output.push_str("  markers = {\n");
    for animation in &declaration.animations {
        output.push_str(&format!("    {} = {{\n", animation.name));
        for marker in &animation.markers {
            output.push_str(&format!(
                "      {{ frame = {}, name = {} }},\n",
                marker.frame + 1,
                lua_string_literal(&marker.name)
            ));
        }
        output.push_str("    },\n");
    }
    output.push_str("  },\n");
    output.push_str("  __tuaMarkerFrames = {\n");
    for animation in &declaration.animations {
        if animation.markers.is_empty() {
            continue;
        }
        output.push_str(&format!("    {} = {{\n", animation.name));
        let mut marker_index = std::collections::BTreeMap::<u32, Vec<&str>>::new();
        for marker in &animation.markers {
            marker_index
                .entry(marker.frame + 1)
                .or_default()
                .push(&marker.name);
        }
        for (frame, names) in marker_index {
            output.push_str(&format!("      [{frame}] = {{"));
            for name in names {
                output.push_str(&format!(" {},", lua_string_literal(name)));
            }
            output.push_str(" },\n");
        }
        output.push_str("    },\n");
    }
    output.push_str("  },\n");
    output.push_str("  frameMetadata = {\n");
    for animation in &declaration.animations {
        let animation_frame = animation.frame.unwrap_or(frame);
        let animation_origin = animation.origin.unwrap_or(declaration.origin);
        output.push_str(&format!("    {} = {{\n", animation.name));
        for metadata in &animation.metadata {
            let origin = metadata.origin.unwrap_or(animation_origin);
            output.push_str(&format!(
                "      {{ originX = {}, originY = {}, trimX = {}, trimY = {}, sourceWidth = {}, sourceHeight = {} }},\n",
                origin.x,
                origin.y,
                metadata.trim_x,
                metadata.trim_y,
                metadata.source_width.unwrap_or(animation_frame.width),
                metadata.source_height.unwrap_or(animation_frame.height),
            ));
        }
        output.push_str("    },\n");
    }
    output.push_str("  },\n");
    output.push_str("  childOrder = {\n");
    for child in &declaration.children {
        output.push_str(&format!("    {},\n", lua_string_literal(&child.name)));
    }
    output.push_str("  },\n");
    output.push_str("  children = {\n");
    for child in &declaration.children {
        output.push_str(&format!(
            "    {} = {{ sheet = {}, initial = {}, x = {}, y = {}, rotation = {}, scaleX = {}, scaleY = {} }},\n",
            child.name,
            child.sheet.as_deref().unwrap_or("nil"),
            child.initial.as_deref().unwrap_or("nil"),
            child.x,
            child.y,
            child.rotation,
            child.scale_x,
            child.scale_y,
        ));
    }
    output.push_str("  },\n");
    output.push_str("  looping = {\n");
    for animation in &declaration.animations {
        output.push_str(&format!(
            "    {} = {},\n",
            animation.name,
            if animation.looping { "true" } else { "false" }
        ));
    }
    output.push_str("  },\n");
    output.push_str("  player = function(self, initial)\n");
    output.push_str("    return tuasprite.new(self, initial)\n");
    output.push_str("  end,\n");
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
    let selectors = animation
        .ranges
        .iter()
        .flat_map(|range| {
            let start = range.start_frame + 1;
            let end = range.end_frame + 1;
            let row = range.row + 1;
            let columns = if start == end {
                start.to_string()
            } else {
                format!("{start}-{end}")
            };
            [lua_string_or_number(&columns), row.to_string()]
        })
        .collect::<Vec<_>>()
        .join(", ");
    let frames = format!("{grid_local}({selectors})");
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

fn lua_string_or_number(value: &str) -> String {
    if value.parse::<u32>().is_ok() {
        value.to_owned()
    } else {
        lua_string_literal(value)
    }
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

    #[test]
    fn lowers_multi_row_ranges_origins_and_markers() {
        let source = r#"local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 180, height = 140 },
  origin = { x = 90, y = 140 },
  animations = {
    run = {
      frames = {
        { from = 0, to = 1, row = 0 },
        { column = 0, row = 1 },
      },
      fps = 12,
      markers = {
        { frame = 1, name = "step" },
      },
      metadata = {
        { origin = { x = 90, y = 140 }, sourceWidth = 180, sourceHeight = 140 },
        { origin = { x = 92, y = 138 }, trimX = 2, trimY = 4, sourceWidth = 184, sourceHeight = 144 },
        { origin = { x = 90, y = 140 }, sourceWidth = 180, sourceHeight = 140 },
      },
    },
  },
})
"#;
        let parsed = parse_tua_source("src/sprites.tua", source).expect("sprite should parse");
        let animation = &parsed.sprite_declarations[0].animations[0];

        assert_eq!(animation.ranges.len(), 2);
        assert_eq!(
            animation
                .ranges
                .iter()
                .map(super::frame_range_len)
                .sum::<usize>(),
            3
        );
        assert_eq!(animation.markers[0].name, "step");

        let lowered = crate::emit_tua_source("src/sprites.tua", source)
            .expect("multi-row sprite should lower");
        assert!(lowered.contains("PlayerGrid(\"1-2\", 1, 1, 2)"));
        assert!(lowered.contains("origin = { x = 90, y = 140 }"));
        assert!(lowered.contains("{ frame = 2, name = \"step\" }"));
        assert!(lowered.contains("[2] = { \"step\","));
        assert!(lowered.contains(
            "{ originX = 92, originY = 138, trimX = 2, trimY = 4, sourceWidth = 184, sourceHeight = 144 }"
        ));
        assert!(lowered.contains("return tuasprite.new(self, initial)"));
    }

    #[test]
    fn infers_exact_sprite_sheet_and_player_types() {
        let parsed =
            parse_tua_source("src/sprites.tua", SPRITE_SOURCE).expect("sprite should parse");
        let environment =
            crate::TypeEnvironment::from_parsed(&parsed).expect("sprite environment should build");
        assert!(environment.function(super::HELPER_NAME).is_none());
        let call = parsed
            .ast
            .nodes()
            .stmts()
            .filter_map(|stmt| match stmt {
                full_moon::ast::Stmt::LocalAssignment(local) => local
                    .expressions()
                    .iter()
                    .find_map(|expression| match expression {
                        full_moon::ast::Expression::FunctionCall(call) => Some(call),
                        _ => None,
                    }),
                _ => None,
            })
            .next()
            .expect("sprite call");
        let player = super::sprite_sheet_type_from_call(call).expect("sprite should be typed");
        let crate::Type::Table(sheet) = player else {
            panic!("expected specialized sprite table, got {player}");
        };
        let animations = &sheet.field("animations").expect("animations field").ty;
        let crate::Type::Table(animations) = animations else {
            panic!("expected exact animation table, got {animations}");
        };

        assert!(animations.field("idle").is_some());
        assert!(animations.field("run").is_some());
        assert!(sheet.field("player").is_some());

        let resolved = environment.resolve_aliases(&crate::Type::Table(sheet.clone()));
        let crate::Type::Table(resolved) = resolved else {
            unreachable!()
        };
        let crate::Type::Function(factory) = &resolved.field("player").expect("player field").ty
        else {
            unreachable!()
        };
        let crate::Type::Table(player) = &factory.return_types_or_any()[0] else {
            unreachable!()
        };
        assert!(player.field("addToBatch").is_some());
        let crate::Type::Function(play) = &player.field("play").expect("play field").ty else {
            unreachable!()
        };
        assert_eq!(
            super::string_literal_values(&play.parameters[1].ty),
            vec!["idle".to_owned(), "run".to_owned()]
        );
    }

    #[test]
    fn lowers_optional_nested_sprite_children() {
        let source = r#"local Hat = spriteSheet({
  image = "assets/hat.png",
  frame = { width = 16, height = 16 },
  animations = { idle = { column = 0, fps = 1 } },
})

local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 32, height = 32 },
  animations = { idle = { column = 0, fps = 1 } },
  children = {
    hat = { sheet = Hat, initial = "idle", x = 4, y = -8, scaleX = 2, scaleY = 2 },
  },
})
"#;
        let parsed = parse_tua_source("src/sprites.tua", source).expect("sprites should parse");
        let player = &parsed.sprite_declarations[1];

        assert_eq!(player.children.len(), 1);
        assert_eq!(player.children[0].name, "hat");
        assert_eq!(player.children[0].sheet.as_deref(), Some("Hat"));
        assert_eq!(player.children[0].y, "-8");

        let lowered =
            crate::emit_tua_source("src/sprites.tua", source).expect("nested sprites should lower");
        assert!(lowered.contains("childOrder = {\n    \"hat\","));
        assert!(lowered.contains(
            "hat = { sheet = Hat, initial = \"idle\", x = 4, y = -8, rotation = 0, scaleX = 2, scaleY = 2 }"
        ));
    }
}
