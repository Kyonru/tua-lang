use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{json, Value};
use tua_core::{
    AssetInventory, CodeAction, CodeActionEdit, CodeActionKind, CodeLens, CodeLensReferenceKind,
    CodeLensReferenceTarget, CodeLensTarget, Completion, CompletionKind, Definition, Diagnostic,
    DiagnosticCode, DiagnosticSeverity, DocumentHighlight, DocumentHighlightKind, DocumentSymbol,
    DocumentSymbolKind, EcsAccessKind, EcsComponentUse, EcsGraph, EcsInsightKind, EcsQueryKind,
    FoldingRange, FoldingRangeKind, Hover, InlayHint, InlayHintKind, InlineCompletionCandidate,
    InspectorItem, ModuleDependencyGraph, ModuleExportOrigin, ModuleLink, ModuleSpecifierKind,
    ObjectPoolGraph, ObjectPoolInsightKind, ObjectPoolOperationKind, Reference, RenameEdit,
    RenameTarget, RequireResolution, SelectionRange, SemanticToken, SemanticTokenKind,
    SemanticTokenModifier, SignalGraph, SignalGraphCallRole, SignalGraphParameter, SignatureHelp,
    SignatureInformation, SourcePosition, SourceSpan, TextEdit, TypeExplanation,
    TypeExplanationStepKind, WorkspaceSymbol, WorkspaceSymbolKind,
};

use super::analysis::{
    AnalysisSnapshot, CallHierarchyIdentity, CallHierarchyIncomingCall, CallHierarchyItem,
    CallHierarchyItemKind, CallHierarchyOutgoingCall,
};
use super::uri::{path_from_uri, uri_from_path};

pub(super) fn response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

pub(super) fn error_response(id: Value, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

pub(super) fn publish_diagnostics(uri: &str, diagnostics: Vec<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {
            "uri": uri,
            "diagnostics": diagnostics
        }
    })
}

pub(super) fn document_diagnostic_report(diagnostics: Vec<Value>) -> Value {
    json!({
        "kind": "full",
        "items": diagnostics
    })
}

pub(super) fn workspace_diagnostic_refresh_request(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "workspace/diagnostic/refresh",
        "params": null
    })
}

pub(super) fn workspace_inlay_hint_refresh_request(id: u64) -> Value {
    workspace_refresh_request(id, "workspace/inlayHint/refresh")
}

pub(super) fn workspace_semantic_tokens_refresh_request(id: u64) -> Value {
    workspace_refresh_request(id, "workspace/semanticTokens/refresh")
}

fn workspace_refresh_request(id: u64, method: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": null
    })
}

pub(super) fn lsp_diagnostic_from_core(diagnostic: Diagnostic, source: &str) -> Value {
    lsp_diagnostic_from_core_with_snapshot(diagnostic, source, None)
}

pub(super) fn lsp_diagnostic_from_core_with_snapshot(
    diagnostic: Diagnostic,
    source: &str,
    snapshot: Option<&AnalysisSnapshot>,
) -> Value {
    let source = if diagnostic
        .file
        .file_name()
        .is_some_and(|name| name == tua_core::CONFIG_FILE)
    {
        ""
    } else {
        source
    };
    let start_line = diagnostic.line.saturating_sub(1);
    let start_character = utf16_column(source, diagnostic.line, diagnostic.column);
    let end_line = diagnostic.end_line.saturating_sub(1);
    let end_character = utf16_column(source, diagnostic.end_line, diagnostic.end_column);

    let related_information = diagnostic
        .related_information
        .iter()
        .filter_map(|information| {
            let related_source = if information.file == diagnostic.file {
                Some(source)
            } else {
                snapshot?.source(&information.file)
            }?;
            Some(json!({
                "location": {
                    "uri": uri_from_path(&information.file),
                    "range": lsp_range_for_source(related_source, information.span)
                },
                "message": information.message
            }))
        })
        .collect::<Vec<_>>();

    let mut result = json!({
        "range": {
            "start": {
                "line": start_line,
                "character": start_character
            },
            "end": {
                "line": end_line,
                "character": end_character
            }
        },
        "severity": lsp_severity(diagnostic.severity),
        "code": diagnostic.code.map(|code| code.to_string()),
        "source": "tua",
        "message": diagnostic.message
    });
    if !related_information.is_empty() {
        result["relatedInformation"] = Value::Array(related_information);
    }
    result
}

#[cfg(test)]
pub(super) fn core_diagnostic_from_lsp(
    path: &std::path::Path,
    diagnostic: &Value,
    source: &str,
) -> Option<Diagnostic> {
    core_diagnostic_from_lsp_with_snapshot(path, diagnostic, source, None)
}

pub(super) fn core_diagnostic_from_lsp_with_snapshot(
    path: &std::path::Path,
    diagnostic: &Value,
    source: &str,
    snapshot: Option<&AnalysisSnapshot>,
) -> Option<Diagnostic> {
    let start = diagnostic.get("range")?.get("start")?;
    let position = source_position_for_source(source, start)?;
    let end = diagnostic
        .get("range")
        .and_then(|range| range.get("end"))
        .and_then(|position| source_position_for_source(source, position))
        .unwrap_or(SourcePosition {
            line: position.line,
            column: position.column.saturating_add(1),
        });
    let code = diagnostic
        .get("code")
        .and_then(Value::as_str)
        .and_then(parse_diagnostic_code);
    let severity = diagnostic
        .get("severity")
        .and_then(Value::as_u64)
        .map(lsp_diagnostic_severity)
        .unwrap_or(DiagnosticSeverity::Warning);
    let message = diagnostic.get("message")?.as_str()?.to_owned();
    let mut output = Diagnostic::new(path, position.line, position.column, severity, message)
        .with_range(position.line, position.column, end.line, end.column);

    if let Some(code) = code {
        output = output.with_code(code);
    }

    if let Some(related_information) = diagnostic
        .get("relatedInformation")
        .and_then(Value::as_array)
    {
        for information in related_information {
            let Some(location) = information.get("location") else {
                continue;
            };
            let Some(uri) = location.get("uri").and_then(Value::as_str) else {
                continue;
            };
            let related_path = path_from_uri(uri);
            let related_source = if related_path == path {
                Some(source)
            } else {
                snapshot.and_then(|snapshot| snapshot.source(&related_path))
            };
            let Some(related_source) = related_source else {
                continue;
            };
            let Some(related_range) = location.get("range") else {
                continue;
            };
            let Some(related_start) = related_range
                .get("start")
                .and_then(|position| source_position_for_source(related_source, position))
            else {
                continue;
            };
            let Some(related_end) = related_range
                .get("end")
                .and_then(|position| source_position_for_source(related_source, position))
            else {
                continue;
            };
            let Some(message) = information.get("message").and_then(Value::as_str) else {
                continue;
            };
            output = output.with_related_information(tua_core::DiagnosticRelatedInformation {
                file: related_path,
                span: SourceSpan {
                    start: related_start,
                    end: related_end,
                },
                message: message.to_owned(),
            });
        }
    }

    Some(output)
}

fn parse_diagnostic_code(code: &str) -> Option<DiagnosticCode> {
    let number = code.strip_prefix("TL")?.parse::<u16>().ok()?;

    DiagnosticCode::new(number)
}

fn lsp_diagnostic_severity(severity: u64) -> DiagnosticSeverity {
    match severity {
        1 => DiagnosticSeverity::Error,
        2 => DiagnosticSeverity::Warning,
        3 => DiagnosticSeverity::Info,
        4 => DiagnosticSeverity::Hint,
        _ => DiagnosticSeverity::Warning,
    }
}

fn lsp_severity(severity: DiagnosticSeverity) -> u8 {
    match severity {
        DiagnosticSeverity::Error => 1,
        DiagnosticSeverity::Warning => 2,
        DiagnosticSeverity::Info => 3,
        DiagnosticSeverity::Hint => 4,
    }
}

pub(super) fn lsp_hover_from_core(hover: Hover, source: &str) -> Value {
    let mut markdown = if hover.is_markdown {
        hover.contents.clone()
    } else {
        format!("```tua\n{}\n```", hover.contents)
    };

    if let Some(documentation) = &hover.documentation {
        markdown.push_str("\n\n");
        markdown.push_str(documentation);
    }

    if !hover.context.is_empty() {
        markdown.push_str("\n\n---\n");

        for item in &hover.context {
            markdown.push_str("\n- ");
            markdown.push_str(item);
        }
    }

    let mut result = json!({
        "contents": {
            "kind": "markdown",
            "value": markdown
        }
    });

    if let Some(span) = hover.span {
        result["range"] = lsp_range_for_source(source, span);
    }

    result
}

pub(super) fn lsp_signal_graph_from_core(graph: SignalGraph, snapshot: &AnalysisSnapshot) -> Value {
    json!({
        "events": graph.events.into_iter().map(|event| {
            json!({
                "id": event.id,
                "name": event.name,
                "bus": event.bus,
                "busUri": uri_from_path(&event.bus_path),
                "resolvedPayload": lsp_signal_graph_parameters(event.resolved_payload),
                "schemaConflict": event.schema_conflict,
                "calls": event.calls.into_iter().map(|call| {
                    json!({
                        "uri": uri_from_path(&call.path),
                        "receiver": call.receiver,
                        "method": call.method,
                        "role": match call.role {
                            SignalGraphCallRole::Declaration => "declaration",
                            SignalGraphCallRole::Listener => "listener",
                            SignalGraphCallRole::Emit => "emit",
                            SignalGraphCallRole::Cleanup => "cleanup",
                            SignalGraphCallRole::Query => "query",
                        },
                        "range": lsp_range_for_source(snapshot.source(&call.path).unwrap_or(""), call.span),
                        "callRange": lsp_range_for_source(snapshot.source(&call.path).unwrap_or(""), call.call_span),
                        "insertionPosition": lsp_position_for_source(snapshot.source(&call.path).unwrap_or(""), call.insertion_position),
                        "indentation": call.indentation,
                        "payload": lsp_signal_graph_parameters(call.payload),
                        "inferredPayload": lsp_signal_graph_parameters(call.inferred_emit_payload),
                    })
                }).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>()
    })
}

fn lsp_signal_graph_parameters(parameters: Option<Vec<SignalGraphParameter>>) -> Value {
    parameters
        .map(|parameters| {
            parameters
                .into_iter()
                .map(|parameter| {
                    json!({
                        "name": parameter.name,
                        "type": parameter.type_label
                    })
                })
                .collect::<Vec<_>>()
        })
        .map(Value::Array)
        .unwrap_or(Value::Null)
}

pub(super) fn lsp_asset_inventory_from_core(
    inventory: AssetInventory,
    snapshot: &AnalysisSnapshot,
) -> Value {
    json!({
        "assets": inventory.assets.into_iter().map(|asset| {
            json!({
                "uri": uri_from_path(&asset.path),
                "path": asset.relative_path,
                "category": asset.category.name(),
                "categoryLabel": asset.category.label(),
                "extension": asset.extension,
                "dimensions": asset.dimensions.map(|(width, height)| json!({
                    "width": width,
                    "height": height
                })),
                "usageCount": asset.references.len(),
                "references": asset.references.into_iter().map(|reference| {
                    json!({
                        "uri": uri_from_path(&reference.path),
                        "range": lsp_range_for_source(snapshot.source(&reference.path).unwrap_or(""), reference.span),
                        "loaderKind": reference.loader_kind.name()
                    })
                }).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>(),
        "diagnostics": inventory.diagnostics.into_iter().map(|diagnostic| {
            let uri = uri_from_path(&diagnostic.file);
            let source = snapshot.source(&diagnostic.file).unwrap_or("");

            json!({
                "uri": uri,
                "diagnostic": lsp_diagnostic_from_core(diagnostic, source)
            })
        }).collect::<Vec<_>>()
    })
}

pub(super) fn lsp_module_dependency_graph_from_core(
    graph: ModuleDependencyGraph,
    snapshot: &AnalysisSnapshot,
) -> Value {
    json!({
        "modules": graph.modules.into_iter().map(|module| {
            json!({
                "uri": uri_from_path(&module.path),
                "sourceKind": match module.source_kind {
                    tua_core::AnalysisSourceKind::Tua => "tua",
                    tua_core::AnalysisSourceKind::Lua => "lua",
                },
                "root": module.is_root,
                "inCycle": module.in_cycle,
                "dependencyCount": module.dependency_count,
                "unresolvedDependencyCount": module.unresolved_dependency_count,
                "reverseDependencyCount": module.reverse_dependency_count,
                "exports": module.exports.into_iter().map(|export| {
                    json!({
                        "name": export.name,
                        "type": export.type_label,
                        "documentation": export.documentation,
                        "origin": match export.origin {
                            ModuleExportOrigin::Tua => "tua",
                            ModuleExportOrigin::Lua => "lua",
                            ModuleExportOrigin::LuaDoc => "luaDoc",
                            ModuleExportOrigin::GeneratedBuiltin => "generatedBuiltin",
                        },
                        "range": lsp_range_for_source(snapshot.source(&module.path).unwrap_or(""), export.definition_span),
                        "exportRange": lsp_range_for_source(snapshot.source(&module.path).unwrap_or(""), export.export_span)
                    })
                }).collect::<Vec<_>>(),
                "inclusionPath": module.inclusion_path.map(|path| {
                    json!({
                        "rootUri": uri_from_path(&path.root),
                        "steps": path.steps.into_iter().map(|step| {
                            json!({
                                "sourceUri": uri_from_path(&step.source),
                                "targetUri": uri_from_path(&step.target),
                                "specifier": step.specifier,
                                "kind": module_specifier_kind_name(step.kind),
                                "range": lsp_range_for_source(snapshot.source(&step.source).unwrap_or(""), step.span)
                            })
                        }).collect::<Vec<_>>()
                    })
                })
            })
        }).collect::<Vec<_>>(),
        "edges": graph.edges.into_iter().map(|edge| {
            let resolution = match edge.resolution {
                RequireResolution::Resolved { path } => json!({
                    "status": "resolved",
                    "targetUri": uri_from_path(&path)
                }),
                RequireResolution::Missing { search_roots } => json!({
                    "status": "missing",
                    "searchRootUris": search_roots.into_iter().map(|path| uri_from_path(&path)).collect::<Vec<_>>()
                }),
                RequireResolution::Invalid { reason } => json!({
                    "status": "invalid",
                    "reason": reason
                }),
                RequireResolution::Duplicate { candidates, selected } => json!({
                    "status": "duplicate",
                    "targetUri": uri_from_path(&selected),
                    "candidateUris": candidates.into_iter().map(|path| uri_from_path(&path)).collect::<Vec<_>>()
                }),
            };

            json!({
                "sourceUri": uri_from_path(&edge.source),
                "specifier": edge.specifier,
                "kind": module_specifier_kind_name(edge.kind),
                "range": lsp_range_for_source(snapshot.source(&edge.source).unwrap_or(""), edge.span),
                "resolution": resolution,
                "inCycle": edge.in_cycle
            })
        }).collect::<Vec<_>>(),
        "cycles": graph.cycles.into_iter().enumerate().map(|(index, paths)| {
            json!({
                "id": format!("cycle:{index}"),
                "moduleUris": paths.into_iter().map(|path| uri_from_path(&path)).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>()
    })
}

pub(super) fn lsp_ecs_graph_from_core(graph: EcsGraph, snapshot: &AnalysisSnapshot) -> Value {
    json!({
        "components": graph.components.into_iter().map(|component| {
            json!({
                "id": component.id,
                "uri": uri_from_path(&component.path),
                "localName": component.local_name,
                "runtimeName": component.runtime_name,
                "range": lsp_range_for_source(snapshot.source(&component.path).unwrap_or(""), component.span),
                "nameRange": lsp_range_for_source(snapshot.source(&component.path).unwrap_or(""), component.name_span),
                "fields": component.fields.into_iter().map(|field| {
                    json!({
                        "name": field.name,
                        "type": field.type_label,
                        "range": lsp_range_for_source(snapshot.source(&component.path).unwrap_or(""), field.span)
                    })
                }).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>(),
        "systems": graph.systems.into_iter().map(|system| {
            json!({
                "id": system.id,
                "uri": uri_from_path(&system.path),
                "name": system.name,
                "range": lsp_range_for_source(snapshot.source(&system.path).unwrap_or(""), system.span),
                "nameRange": system.name_span.map(|span| lsp_range_for_source(snapshot.source(&system.path).unwrap_or(""), span)),
                "callbackRange": system.callback_span.map(|span| lsp_range_for_source(snapshot.source(&system.path).unwrap_or(""), span)),
                "components": system.components.into_iter().map(|component| lsp_ecs_component_use(component, snapshot.source(&system.path).unwrap_or(""))).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>(),
        "queries": graph.queries.into_iter().map(|query| {
            json!({
                "id": query.id,
                "uri": uri_from_path(&query.path),
                "name": query.name,
                "kind": match query.kind {
                    EcsQueryKind::Standalone => "standalone",
                    EcsQueryKind::System => "system",
                },
                "ownerSystemId": query.owner_system_id,
                "range": lsp_range_for_source(snapshot.source(&query.path).unwrap_or(""), query.span),
                "nameRange": query.name_span.map(|span| lsp_range_for_source(snapshot.source(&query.path).unwrap_or(""), span)),
                "components": query.components.into_iter().map(|component| lsp_ecs_component_use(component, snapshot.source(&query.path).unwrap_or(""))).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>(),
        "insights": graph.insights.into_iter().map(|insight| {
            json!({
                "id": insight.id,
                "kind": match insight.kind {
                    EcsInsightKind::WideQueryAllocation => "wideQueryAllocation",
                    EcsInsightKind::CallbackAllocation => "callbackAllocation",
                    EcsInsightKind::ResourceConstruction => "resourceConstruction",
                },
                "title": insight.title,
                "message": insight.message,
                "uri": uri_from_path(&insight.path),
                "range": lsp_range_for_source(snapshot.source(&insight.path).unwrap_or(""), insight.span),
                "subjectId": insight.subject_id
            })
        }).collect::<Vec<_>>()
    })
}

pub(super) fn lsp_object_pool_graph_from_core(
    graph: ObjectPoolGraph,
    snapshot: &AnalysisSnapshot,
) -> Value {
    json!({
        "pools": graph.pools.into_iter().map(|pool| {
            json!({
                "id": pool.id,
                "uri": uri_from_path(&pool.path),
                "name": pool.name,
                "itemType": pool.item_type_label,
                "range": lsp_range_for_source(snapshot.source(&pool.path).unwrap_or(""), pool.span),
                "nameRange": lsp_range_for_source(snapshot.source(&pool.path).unwrap_or(""), pool.name_span)
            })
        }).collect::<Vec<_>>(),
        "operations": graph.operations.into_iter().map(|operation| {
            json!({
                "id": operation.id,
                "uri": uri_from_path(&operation.path),
                "poolId": operation.pool_id,
                "poolName": operation.pool_name,
                "kind": match operation.kind {
                    ObjectPoolOperationKind::Acquire => "acquire",
                    ObjectPoolOperationKind::Release => "release",
                    ObjectPoolOperationKind::Clear => "clear",
                    ObjectPoolOperationKind::Prewarm => "prewarm",
                },
                "itemName": operation.item_name,
                "range": lsp_range_for_source(snapshot.source(&operation.path).unwrap_or(""), operation.span),
                "itemRange": operation.item_span.map(|span| lsp_range_for_source(snapshot.source(&operation.path).unwrap_or(""), span))
            })
        }).collect::<Vec<_>>(),
        "insights": graph.insights.into_iter().map(|insight| {
            json!({
                "id": insight.id,
                "kind": match insight.kind {
                    ObjectPoolInsightKind::UnreleasedAcquire => "unreleasedAcquire",
                },
                "title": insight.title,
                "message": insight.message,
                "uri": uri_from_path(&insight.path),
                "range": lsp_range_for_source(snapshot.source(&insight.path).unwrap_or(""), insight.span),
                "poolId": insight.pool_id,
                "acquireOperationId": insight.acquire_operation_id,
                "itemName": insight.item_name
            })
        }).collect::<Vec<_>>()
    })
}

pub(super) fn lsp_audio_graph_from_core(
    graph: tua_core::AudioGraph,
    snapshot: &AnalysisSnapshot,
) -> Value {
    json!({
        "systems": graph.systems.into_iter().map(|system| json!({
            "id": system.id,
            "uri": uri_from_path(&system.path),
            "range": lsp_range_for_source(snapshot.source(&system.path).unwrap_or(""), system.span),
            "buses": system.buses,
            "cues": system.cues,
            "effects": system.effects,
            "musicStates": system.music_states,
            "layers": system.layers,
            "busItems": system.bus_items.into_iter().map(|item| lsp_audio_item(item, snapshot.source(&system.path).unwrap_or(""))).collect::<Vec<_>>(),
            "cueItems": system.cue_items.into_iter().map(|item| lsp_audio_item(item, snapshot.source(&system.path).unwrap_or(""))).collect::<Vec<_>>(),
            "effectItems": system.effect_items.into_iter().map(|item| lsp_audio_item(item, snapshot.source(&system.path).unwrap_or(""))).collect::<Vec<_>>(),
            "sceneItems": system.scene_items.into_iter().map(|item| lsp_audio_item(item, snapshot.source(&system.path).unwrap_or(""))).collect::<Vec<_>>(),
        })).collect::<Vec<_>>()
    })
}

fn lsp_audio_item(item: tua_core::AudioGraphItem, source: &str) -> Value {
    json!({
        "id": item.id,
        "name": item.name,
        "range": lsp_range_for_source(source, item.span),
        "properties": item.properties.into_iter().map(|(name, property)| {
            (name, json!({"kind": property.kind, "value": property.value}))
        }).collect::<serde_json::Map<String, Value>>(),
        "source": item.source.map(|source_item| json!({
            "kind": source_item.kind,
            "assetPath": source_item.asset_path,
            "loader": source_item.loader,
            "range": lsp_range_for_source(source, source_item.span),
        })),
        "layers": item.layers.into_iter().map(|layer| lsp_audio_item(layer, source)).collect::<Vec<_>>(),
    })
}

fn lsp_ecs_component_use(component: EcsComponentUse, source: &str) -> Value {
    json!({
        "reference": component.reference,
        "componentId": component.component_id,
        "access": match component.access {
            EcsAccessKind::Filter => "filter",
            EcsAccessKind::Read => "read",
            EcsAccessKind::Write => "write",
            EcsAccessKind::ReadWrite => "readWrite",
            EcsAccessKind::Unknown => "unknown",
        },
        "range": lsp_range_for_source(source, component.span)
    })
}

fn module_specifier_kind_name(kind: ModuleSpecifierKind) -> &'static str {
    match kind {
        ModuleSpecifierKind::Require => "require",
        ModuleSpecifierKind::Import => "import",
    }
}

pub(super) fn lsp_inspector_items_from_core(items: Vec<InspectorItem>, source: &str) -> Value {
    Value::Array(
        items
            .into_iter()
            .map(|item| match item {
                InspectorItem::SpriteSheet(sprite) => {
                    let image = sprite.image.map(|image| {
                        json!({
                            "path": image.path,
                            "uri": image.resolved_path.map(|path| uri_from_path(&path)),
                            "status": match image.status {
                                tua_core::AssetValidationStatus::Valid => "valid",
                                tua_core::AssetValidationStatus::Missing => "missing",
                                tua_core::AssetValidationStatus::UnsupportedExtension => "unsupportedExtension",
                                tua_core::AssetValidationStatus::WrongKind => "wrongKind",
                                tua_core::AssetValidationStatus::PathCaseMismatch => "pathCaseMismatch",
                                tua_core::AssetValidationStatus::Unchecked => "unchecked",
                            },
                            "dimensions": image.dimensions.map(|(width, height)| json!({
                                "width": width,
                                "height": height
                            })),
                            "range": lsp_range_for_source(source, image.span)
                        })
                    });
                    let frame = sprite.frame.map(|frame| {
                        json!({
                            "width": frame.width,
                            "height": frame.height,
                            "left": frame.left,
                            "top": frame.top,
                            "border": frame.border,
                            "range": lsp_range_for_source(source, frame.span)
                        })
                    });

                    json!({
                        "kind": "spriteSheet",
                        "name": sprite.name,
                        "range": lsp_range_for_source(source, sprite.span),
                        "selectionRange": lsp_range_for_source(source, sprite.name_span),
                        "image": image,
                        "frame": frame,
                        "origin": {
                            "x": sprite.origin.x,
                            "y": sprite.origin.y
                        },
                        "animations": sprite.animations.into_iter().map(|animation| {
                            json!({
                                "name": animation.name,
                                "from": animation.from,
                                "to": animation.to,
                                "row": animation.row,
                                "column": animation.column,
                                "frames": animation.ranges.into_iter().map(|range| json!({
                                    "from": range.from,
                                    "to": range.to,
                                    "row": range.row
                                })).collect::<Vec<_>>(),
                                "frame": animation.frame.map(|frame| json!({
                                    "width": frame.width,
                                    "height": frame.height,
                                    "left": frame.left,
                                    "top": frame.top,
                                    "border": frame.border,
                                    "range": lsp_range_for_source(source, frame.span)
                                })),
                                "origin": {
                                    "x": animation.origin.x,
                                    "y": animation.origin.y
                                },
                                "fps": animation.fps,
                                "durations": animation.durations,
                                "durationValues": animation.duration_values,
                                "markers": animation.markers.into_iter().map(|marker| json!({
                                    "frame": marker.frame,
                                    "name": marker.name
                                })).collect::<Vec<_>>(),
                                "metadata": animation.metadata.into_iter().map(|metadata| json!({
                                    "origin": {
                                        "x": metadata.origin.x,
                                        "y": metadata.origin.y
                                    },
                                    "trimX": metadata.trim_x,
                                    "trimY": metadata.trim_y,
                                    "sourceWidth": metadata.source_width,
                                    "sourceHeight": metadata.source_height
                                })).collect::<Vec<_>>(),
                                "onLoop": animation.on_loop,
                                "loop": animation.looping,
                                "range": lsp_range_for_source(source, animation.span),
                                "selectionRange": lsp_range_for_source(source, animation.name_span)
                            })
                        }).collect::<Vec<_>>(),
                        "children": sprite.children.into_iter().map(|child| json!({
                            "name": child.name,
                            "sheet": child.sheet,
                            "initial": child.initial,
                            "x": child.x,
                            "y": child.y,
                            "rotation": child.rotation,
                            "scaleX": child.scale_x,
                            "scaleY": child.scale_y,
                            "range": lsp_range_for_source(source, child.span),
                            "selectionRange": lsp_range_for_source(source, child.name_span)
                        })).collect::<Vec<_>>(),
                        "diagnostics": sprite.diagnostics.into_iter()
                            .map(|diagnostic| lsp_diagnostic_from_core(diagnostic, source))
                            .collect::<Vec<_>>()
                    })
                }
                InspectorItem::StateMachine(machine) => json!({
                    "kind": "stateMachine",
                    "name": machine.name,
                    "initial": machine.initial,
                    "range": lsp_range_for_source(source, machine.span),
                    "selectionRange": lsp_range_for_source(source, machine.name_span),
                    "states": machine.states.into_iter().map(|state| json!({
                        "name": state.name,
                        "initial": state.initial,
                        "range": lsp_range_for_source(source, state.span)
                    })).collect::<Vec<_>>(),
                    "transitions": machine.transitions.into_iter().map(|transition| json!({
                        "event": transition.event,
                        "from": transition.from,
                        "wildcardFrom": transition.wildcard_from,
                        "to": transition.to,
                        "range": lsp_range_for_source(source, transition.span),
                        "selectionRange": lsp_range_for_source(source, transition.event_span)
                    })).collect::<Vec<_>>()
                }),
                InspectorItem::BehaviorTree(tree) => json!({
                    "kind": "behaviorTree",
                    "name": tree.name,
                    "truncated": tree.truncated,
                    "range": lsp_range_for_source(source, tree.span),
                    "selectionRange": lsp_range_for_source(source, tree.name_span),
                    "root": tree.root.map(|root| lsp_behavior_tree_node(root, source))
                }),
            })
            .collect(),
    )
}

fn lsp_behavior_tree_node(node: tua_core::BehaviorTreeNodeInspection, source: &str) -> Value {
    json!({
        "kind": node.kind,
        "range": lsp_range_for_source(source, node.span),
        "children": node.children.into_iter()
            .map(|child| lsp_behavior_tree_node(child, source))
            .collect::<Vec<_>>()
    })
}

pub(super) fn lsp_type_explanation_from_core(
    explanation: TypeExplanation,
    source: &str,
    snapshot: &AnalysisSnapshot,
) -> Value {
    json!({
        "expression": explanation.expression,
        "type": explanation.ty.to_string(),
        "isAny": matches!(explanation.ty, tua_core::Type::Any),
        "range": lsp_range_for_source(source, explanation.span),
        "steps": explanation.steps.into_iter().map(|step| {
            json!({
                "kind": match step.kind {
                    TypeExplanationStepKind::Annotation => "annotation",
                    TypeExplanationStepKind::Inference => "inference",
                    TypeExplanationStepKind::Widening => "widening",
                    TypeExplanationStepKind::Narrowing => "narrowing",
                    TypeExplanationStepKind::Module => "module",
                    TypeExplanationStepKind::Field => "field",
                    TypeExplanationStepKind::Dynamic => "dynamic",
                    TypeExplanationStepKind::Unresolved => "unresolved",
                },
                "title": step.title,
                "detail": step.detail,
                "type": step.ty.map(|ty| ty.to_string()),
                "uri": step.path.as_deref().map(uri_from_path),
                "range": step.span.map(|span| lsp_range_for_source(step.path.as_deref().map(|path| snapshot.source(path).unwrap_or("")).unwrap_or(source), span)),
            })
        }).collect::<Vec<_>>()
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompletionResolveData {
    pub(super) uri: String,
    pub(super) position: SourcePosition,
    pub(super) label: String,
    pub(super) kind: Option<CompletionKind>,
    pub(super) module_path: Option<PathBuf>,
    pub(super) source_module: Option<String>,
}

// Snippets are opt-in. Keep callable candidates usable as plain identifiers,
// but omit template-only candidates when the client cannot expand them.
pub(super) fn completion_for_client(
    mut completion: Completion,
    snippets: bool,
) -> Option<Completion> {
    if !snippets && completion.is_snippet {
        if completion.kind == CompletionKind::Snippet {
            return None;
        }
        completion.insert_text = Some(completion.label.clone());
        completion.is_snippet = false;
    }
    Some(completion)
}

pub(super) fn lsp_completion_from_core(
    uri: &str,
    position: SourcePosition,
    completion: Completion,
    replacement_span: Option<SourceSpan>,
    label_details_support: bool,
    source: &str,
) -> Value {
    let replacement_text = completion
        .insert_text
        .clone()
        .unwrap_or_else(|| completion.label.clone());
    let kind = lsp_completion_kind(completion.kind);
    let label = completion.label;
    let insert_text = completion.insert_text;
    let filter_text = completion.filter_text;
    let is_snippet = completion.is_snippet;
    let deprecated = completion.deprecation.is_some();
    let detail = completion.detail;
    let core_kind = completion.kind.as_str();
    let module_path = completion
        .module_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let source_module = completion.source_module.clone();
    let mut item = json!({
        "label": label.clone(),
        "kind": kind,
        "data": {
            "uri": uri,
            "label": label,
            "kind": core_kind,
            "position": lsp_position(position)
        }
    });

    if let Some(insert_text) = insert_text {
        item["insertText"] = json!(insert_text);
    }

    if let Some(replacement_span) = replacement_span {
        item["textEdit"] = json!({
            "range": lsp_range_for_source(source, replacement_span),
            "newText": replacement_text
        });
    }

    if let Some(filter_text) = filter_text {
        item["filterText"] = json!(filter_text);
    }

    if is_snippet {
        item["insertTextFormat"] = json!(2);
    }

    if deprecated {
        item["tags"] = json!([1]);
    }

    if label_details_support {
        if let Some(detail) = detail {
            item["labelDetails"] = json!({
                "description": detail
            });
        }
    }

    if let Some(module_path) = module_path {
        item["data"]["modulePath"] = json!(module_path);
    }

    if let Some(source_module) = source_module {
        item["data"]["sourceModule"] = json!(source_module);
    }

    item
}

pub(super) fn completion_resolve_data(item: &Value) -> Option<CompletionResolveData> {
    let data = item.get("data")?;
    let uri = data.get("uri")?.as_str()?.to_owned();
    let label = data.get("label")?.as_str()?.to_owned();
    let position = source_position_from_lsp(data.get("position")?);
    let kind = data
        .get("kind")
        .and_then(Value::as_str)
        .and_then(CompletionKind::parse_kind);
    let module_path = data
        .get("modulePath")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let source_module = data
        .get("sourceModule")
        .and_then(Value::as_str)
        .map(str::to_owned);

    Some(CompletionResolveData {
        uri,
        position: position?,
        label,
        kind,
        module_path,
        source_module,
    })
}

pub(super) fn lsp_resolved_completion_item(
    mut item: Value,
    completion: Option<Completion>,
    source: &str,
) -> Value {
    let Some(completion) = completion else {
        return item;
    };

    if let Some(detail) = completion.detail {
        item["detail"] = json!(detail);
    }

    if let Some(documentation) = completion.documentation {
        item["documentation"] = json!({
            "kind": "markdown",
            "value": documentation
        });
    }

    if item.get("insertText").is_none() {
        if let Some(insert_text) = completion.insert_text {
            item["insertText"] = json!(insert_text);
        }
    }

    if item.get("filterText").is_none() {
        if let Some(filter_text) = completion.filter_text {
            item["filterText"] = json!(filter_text);
        }
    }

    if completion.is_snippet {
        item["insertTextFormat"] = json!(2);
    }

    if completion.deprecation.is_some() {
        item["tags"] = json!([1]);
    }

    if !completion.additional_text_edits.is_empty() {
        item["additionalTextEdits"] = json!(completion
            .additional_text_edits
            .into_iter()
            .map(|edit| lsp_text_edit(edit, source))
            .collect::<Vec<_>>());
    }

    let type_label = item
        .get("detail")
        .and_then(Value::as_str)
        .map(str::to_owned);

    if let Some(data) = item.get_mut("data").and_then(Value::as_object_mut) {
        data.insert("symbolKind".to_owned(), json!(completion.kind.as_str()));

        if let Some(type_label) = type_label {
            data.insert("typeLabel".to_owned(), json!(type_label));
        }

        if let Some(module_path) = completion.module_path {
            data.insert(
                "modulePath".to_owned(),
                json!(module_path.to_string_lossy().to_string()),
            );
        }

        if let Some(source_module) = completion.source_module {
            data.insert("sourceModule".to_owned(), json!(source_module));
        }
    }

    item
}

pub(super) fn lsp_signature_help_from_core(help: SignatureHelp) -> Value {
    json!({
        "signatures": help
            .signatures
            .into_iter()
            .map(lsp_signature_information_from_core)
            .collect::<Vec<_>>(),
        "activeSignature": help.active_signature,
        "activeParameter": help.active_parameter
    })
}

fn lsp_signature_information_from_core(signature: SignatureInformation) -> Value {
    let mut item = json!({
        "label": signature.label,
        "parameters": signature
            .parameters
            .into_iter()
            .map(lsp_parameter_information_from_core)
            .collect::<Vec<_>>()
    });

    if let Some(documentation) = signature.documentation {
        item["documentation"] = json!({
            "kind": "markdown",
            "value": documentation
        });
    }

    item
}

fn lsp_parameter_information_from_core(parameter: tua_core::ParameterInformation) -> Value {
    let mut item = json!({
        "label": parameter.label
    });

    if let Some(documentation) = parameter.documentation {
        item["documentation"] = json!({
            "kind": "markdown",
            "value": documentation
        });
    }

    item
}

fn lsp_completion_kind(kind: CompletionKind) -> u8 {
    match kind {
        CompletionKind::Snippet => 15,
        CompletionKind::Function => 3,
        CompletionKind::TypeAlias => 25,
        CompletionKind::Field => 5,
        CompletionKind::Keyword => 14,
        CompletionKind::Module => 9,
        CompletionKind::Variable => 6,
        CompletionKind::Value => 12,
    }
}

pub(super) fn lsp_location_from_core(definition: Definition, sources: &AnalysisSnapshot) -> Value {
    lsp_location(
        &uri_from_path(&definition.path),
        definition.span,
        sources.source(&definition.path).unwrap_or(""),
    )
}

pub(super) fn lsp_call_hierarchy_item(
    item: CallHierarchyItem,
    sources: &AnalysisSnapshot,
) -> Value {
    json!({
        "name": item.name,
        "kind": lsp_call_hierarchy_kind(item.kind),
        "uri": uri_from_path(&item.path),
        "range": lsp_range_for_source(sources.source(&item.path).unwrap_or(""), item.range),
        "selectionRange": lsp_range_for_source(sources.source(&item.path).unwrap_or(""), item.selection_range),
        "data": {
            "path": item.path.to_string_lossy().to_string(),
            "selectionRange": lsp_range(item.selection_range)
        }
    })
}

pub(super) fn call_hierarchy_identity_from_lsp(item: &Value) -> Option<CallHierarchyIdentity> {
    let data = item.get("data")?;
    let range = lsp_source_range(data.get("selectionRange")?)?;

    Some(CallHierarchyIdentity {
        path: PathBuf::from(data.get("path")?.as_str()?),
        selection_range: SourceSpan {
            start: range.start,
            end: range.end,
        },
    })
}

pub(super) fn lsp_call_hierarchy_incoming_call(
    call: CallHierarchyIncomingCall,
    sources: &AnalysisSnapshot,
) -> Value {
    let source = sources.source(&call.from.path).unwrap_or("");
    json!({
        "from": lsp_call_hierarchy_item(call.from, sources),
        "fromRanges": call
            .from_ranges
            .into_iter()
            .map(|range| lsp_range_for_source(source, range))
            .collect::<Vec<_>>()
    })
}

pub(super) fn lsp_call_hierarchy_outgoing_call(
    call: CallHierarchyOutgoingCall,
    sources: &AnalysisSnapshot,
    source: &str,
) -> Value {
    json!({
        "to": lsp_call_hierarchy_item(call.to, sources),
        "fromRanges": call
            .from_ranges
            .into_iter()
            .map(|range| lsp_range_for_source(source, range))
            .collect::<Vec<_>>()
    })
}

fn lsp_call_hierarchy_kind(kind: CallHierarchyItemKind) -> u8 {
    match kind {
        CallHierarchyItemKind::Function => 12,
        CallHierarchyItemKind::Method => 6,
    }
}

pub(super) fn lsp_document_link_from_core(link: ModuleLink, source: &str) -> Value {
    json!({
        "range": lsp_range_for_source(source, link.span),
        "target": uri_from_path(&link.target)
    })
}

pub(super) fn lsp_document_highlight_from_core(
    highlight: DocumentHighlight,
    source: &str,
) -> Value {
    json!({
        "range": lsp_range_for_source(source, highlight.span),
        "kind": lsp_document_highlight_kind(highlight.kind)
    })
}

fn lsp_document_highlight_kind(kind: DocumentHighlightKind) -> u8 {
    match kind {
        DocumentHighlightKind::Text => 1,
    }
}

pub(super) fn lsp_reference_location(reference: Reference, sources: &AnalysisSnapshot) -> Value {
    lsp_location(
        &uri_from_path(&reference.path),
        reference.span,
        sources.source(&reference.path).unwrap_or(""),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CodeLensResolveData {
    pub(super) target: CodeLensReferenceTarget,
}

pub(super) fn lsp_code_lens_from_core(lens: CodeLens, source: &str) -> Value {
    match lens.target {
        CodeLensTarget::References(target) => json!({
            "range": lsp_range_for_source(source, lens.range),
            "data": {
                "kind": "references",
                "symbolKind": code_lens_reference_kind(target.kind),
                "uri": uri_from_path(&target.path),
                "position": lsp_position(target.position),
                "name": target.name
            }
        }),
    }
}

pub(super) fn code_lens_resolve_data(lens: &Value) -> Option<CodeLensResolveData> {
    let data = lens.get("data")?;

    if data.get("kind")?.as_str()? != "references" {
        return None;
    }

    let uri = data.get("uri")?.as_str()?;
    let position = source_position_from_lsp(data.get("position")?)?;
    let name = data.get("name")?.as_str()?.to_owned();
    let kind = code_lens_reference_kind_from_str(data.get("symbolKind")?.as_str()?)?;

    Some(CodeLensResolveData {
        target: CodeLensReferenceTarget {
            path: super::uri::path_from_uri(uri),
            position,
            name,
            kind,
        },
    })
}

pub(super) fn lsp_resolved_reference_code_lens(
    mut lens: Value,
    target: &CodeLensReferenceTarget,
    references: Vec<Reference>,
    sources: &AnalysisSnapshot,
) -> Value {
    let title = reference_code_lens_title(references.len());
    let locations = references
        .into_iter()
        .map(|reference| lsp_reference_location(reference, sources))
        .collect::<Vec<_>>();

    lens["command"] = json!({
        "title": title,
        "command": "tua.hover.references",
        "arguments": [
            {
                "uri": uri_from_path(&target.path),
                "position": lsp_position_for_source(sources.source(&target.path).unwrap_or(""), target.position)
            },
            locations
        ]
    });

    lens
}

fn code_lens_reference_kind(kind: CodeLensReferenceKind) -> &'static str {
    match kind {
        CodeLensReferenceKind::ExportedFunction => "exportedFunction",
        CodeLensReferenceKind::TypeAlias => "typeAlias",
    }
}

fn code_lens_reference_kind_from_str(kind: &str) -> Option<CodeLensReferenceKind> {
    match kind {
        "exportedFunction" => Some(CodeLensReferenceKind::ExportedFunction),
        "typeAlias" => Some(CodeLensReferenceKind::TypeAlias),
        _ => None,
    }
}

fn reference_code_lens_title(count: usize) -> String {
    match count {
        1 => "1 reference".to_owned(),
        _ => format!("{count} references"),
    }
}

pub(super) fn lsp_prepare_rename_from_core(target: RenameTarget, source: &str) -> Value {
    json!({
        "range": lsp_range_for_source(source, target.span),
        "placeholder": target.placeholder
    })
}

pub(super) fn lsp_code_action_from_core(action: CodeAction, sources: &AnalysisSnapshot) -> Value {
    json!({
        "title": action.title,
        "kind": lsp_code_action_kind(action.kind),
        "edit": lsp_code_action_edit(action.edit, sources)
    })
}

fn lsp_code_action_kind(kind: CodeActionKind) -> &'static str {
    match kind {
        CodeActionKind::QuickFix => "quickfix",
        CodeActionKind::SourceOrganizeImports => "source.organizeImports",
        CodeActionKind::SourceSortImports => "source.sortImports",
        CodeActionKind::SourceRemoveUnusedImports => "source.removeUnusedImports",
        CodeActionKind::SourceCreateLoveCallbacks => "source.tua.createMissingLoveCallbacks",
        CodeActionKind::SourceFixAll => "source.fixAll",
    }
}

fn lsp_code_action_edit(edit: CodeActionEdit, sources: &AnalysisSnapshot) -> Value {
    let raw_changes = edit.changes;
    let changes = lsp_changes(raw_changes.clone(), sources);

    if edit.create_files.is_empty() {
        return json!({
            "changes": changes
        });
    }

    let mut document_changes = edit
        .create_files
        .into_iter()
        .map(|file| {
            json!({
                "kind": "create",
                "uri": uri_from_path(&file.path),
                "options": {
                    "ignoreIfExists": file.ignore_if_exists
                }
            })
        })
        .collect::<Vec<_>>();

    for (uri, edits) in changes_for_document_changes(raw_changes, sources) {
        document_changes.push(json!({
            "textDocument": {
                "uri": uri,
                "version": null
            },
            "edits": edits
        }));
    }

    json!({
        "documentChanges": document_changes
    })
}

pub(super) fn lsp_workspace_edit(edit: RenameEdit, sources: &AnalysisSnapshot) -> Value {
    json!({
        "changes": lsp_changes(group_text_edits(edit.edits), sources)
    })
}

fn group_text_edits(edits: Vec<TextEdit>) -> BTreeMap<std::path::PathBuf, Vec<TextEdit>> {
    let mut changes = BTreeMap::new();

    for edit in edits {
        changes
            .entry(edit.path.clone())
            .or_insert_with(Vec::new)
            .push(edit);
    }

    changes
}

fn lsp_changes(
    changes: BTreeMap<std::path::PathBuf, Vec<TextEdit>>,
    sources: &AnalysisSnapshot,
) -> BTreeMap<String, Vec<Value>> {
    changes
        .into_iter()
        .map(|(path, edits)| {
            (
                uri_from_path(&path),
                edits
                    .into_iter()
                    .map(|edit| lsp_text_edit(edit, sources.source(&path).unwrap_or("")))
                    .collect(),
            )
        })
        .collect()
}

fn changes_for_document_changes(
    changes: BTreeMap<std::path::PathBuf, Vec<TextEdit>>,
    sources: &AnalysisSnapshot,
) -> Vec<(String, Vec<Value>)> {
    lsp_changes(changes, sources).into_iter().collect()
}

pub(super) fn lsp_text_edit(edit: TextEdit, source: &str) -> Value {
    let source = if edit
        .path
        .file_name()
        .is_some_and(|name| name == tua_core::CONFIG_FILE)
    {
        ""
    } else {
        source
    };
    json!({
        "range": lsp_range_for_source(source, edit.span),
        "newText": edit.new_text
    })
}

pub(super) fn lsp_full_document_text_edit(source: &str, new_text: String) -> Value {
    let (end_line, end_character) = lsp_document_end_position(source);

    json!({
        "range": {
            "start": {
                "line": 0,
                "character": 0
            },
            "end": {
                "line": end_line,
                "character": end_character
            }
        },
        "newText": new_text
    })
}

fn lsp_document_end_position(source: &str) -> (usize, usize) {
    let mut line = 0usize;
    let mut character = 0usize;

    for ch in source.chars() {
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf16();
        }
    }

    (line, character)
}

pub(super) fn lsp_workspace_symbol_from_core(
    symbol: WorkspaceSymbol,
    sources: &AnalysisSnapshot,
) -> Value {
    let mut item = json!({
        "name": symbol.name,
        "kind": lsp_workspace_symbol_kind(symbol.kind),
        "location": {
            "uri": uri_from_path(&symbol.path),
            "range": lsp_range_for_source(sources.source(&symbol.path).unwrap_or(""), symbol.selection_range)
        }
    });

    if let Some(container_name) = symbol.container_name {
        item["containerName"] = json!(container_name);
    }

    item
}

fn lsp_workspace_symbol_kind(kind: WorkspaceSymbolKind) -> u8 {
    match kind {
        WorkspaceSymbolKind::Class => 5,
        WorkspaceSymbolKind::Module => 2,
        WorkspaceSymbolKind::Function => 12,
        WorkspaceSymbolKind::TypeAlias => 23,
        WorkspaceSymbolKind::Local => 13,
        WorkspaceSymbolKind::Parameter => 13,
        WorkspaceSymbolKind::Field => 8,
        WorkspaceSymbolKind::Export => 8,
    }
}

pub(super) fn lsp_semantic_token_data_from_core(
    tokens: Vec<SemanticToken>,
    source: &str,
) -> Vec<usize> {
    let mut data = Vec::with_capacity(tokens.len() * 5);
    let lines = source.split('\n').collect::<Vec<_>>();
    let column = |position: SourcePosition| {
        lines
            .get(position.line.saturating_sub(1))
            .map(|line| {
                line.chars()
                    .take(position.column.saturating_sub(1))
                    .map(char::len_utf16)
                    .sum::<usize>()
            })
            .unwrap_or_else(|| position.column.saturating_sub(1))
    };
    let mut previous_line = 0usize;
    let mut previous_start = 0usize;

    for token in tokens {
        if token.span.start.line != token.span.end.line {
            continue;
        }

        let line = token.span.start.line.saturating_sub(1);
        let start = column(token.span.start);
        let length = column(token.span.end).saturating_sub(start);
        let delta_line = line.saturating_sub(previous_line);
        let delta_start = if delta_line == 0 {
            start.saturating_sub(previous_start)
        } else {
            start
        };

        data.extend([
            delta_line,
            delta_start,
            length,
            semantic_token_type_index(token.kind),
            semantic_token_modifier_bits(&token.modifiers),
        ]);

        previous_line = line;
        previous_start = start;
    }

    data
}

pub(super) fn lsp_semantic_tokens_from_core(tokens: Vec<SemanticToken>, source: &str) -> Value {
    lsp_semantic_tokens(lsp_semantic_token_data_from_core(tokens, source), None)
}

pub(super) fn lsp_semantic_tokens(data: Vec<usize>, result_id: Option<&str>) -> Value {
    let mut result = json!({ "data": data });

    if let Some(result_id) = result_id {
        result["resultId"] = json!(result_id);
    }

    result
}

pub(super) fn lsp_semantic_token_delta(
    previous_data: &[usize],
    current_data: Vec<usize>,
    result_id: &str,
) -> Value {
    let edits = semantic_token_delta_edits(previous_data, &current_data);

    json!({
        "resultId": result_id,
        "edits": edits,
    })
}

fn semantic_token_delta_edits(previous_data: &[usize], current_data: &[usize]) -> Vec<Value> {
    if previous_data == current_data {
        return Vec::new();
    }

    let prefix = previous_data
        .iter()
        .zip(current_data.iter())
        .take_while(|(previous, current)| previous == current)
        .count();
    let mut suffix = 0usize;

    while suffix < previous_data.len().saturating_sub(prefix)
        && suffix < current_data.len().saturating_sub(prefix)
        && previous_data[previous_data.len() - 1 - suffix]
            == current_data[current_data.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let delete_count = previous_data.len() - prefix - suffix;
    let insert_end = current_data.len() - suffix;
    let inserted = current_data[prefix..insert_end].to_vec();
    let mut edit = json!({
        "start": prefix,
        "deleteCount": delete_count,
    });

    if !inserted.is_empty() {
        edit["data"] = json!(inserted);
    }

    vec![edit]
}

fn semantic_token_type_index(kind: SemanticTokenKind) -> usize {
    match kind {
        SemanticTokenKind::TypeAlias | SemanticTokenKind::BuiltinType => 0,
        SemanticTokenKind::Function => 1,
        SemanticTokenKind::Variable
        | SemanticTokenKind::UnresolvedReference
        | SemanticTokenKind::Global => 2,
        SemanticTokenKind::Parameter => 3,
        SemanticTokenKind::Property => 4,
        SemanticTokenKind::Method => 5,
        SemanticTokenKind::Class => 6,
        SemanticTokenKind::Enum => 7,
        SemanticTokenKind::EnumMember => 8,
        SemanticTokenKind::Module | SemanticTokenKind::Namespace => 9,
        SemanticTokenKind::Event => 10,
        SemanticTokenKind::Label => 11,
        SemanticTokenKind::AnnotationTag => 12,
        SemanticTokenKind::Protocol => 13,
    }
}

fn semantic_token_modifier_bits(modifiers: &[SemanticTokenModifier]) -> usize {
    modifiers.iter().fold(0usize, |bits, modifier| {
        bits | semantic_token_modifier_bit(*modifier)
    })
}

fn semantic_token_modifier_bit(modifier: SemanticTokenModifier) -> usize {
    match modifier {
        SemanticTokenModifier::Declaration => 1 << 0,
        SemanticTokenModifier::Readonly => 1 << 1,
        SemanticTokenModifier::Deprecated => 1 << 2,
        SemanticTokenModifier::Modification => 1 << 3,
        SemanticTokenModifier::DefaultLibrary => 1 << 4,
        SemanticTokenModifier::PropertyChain1 => 1 << 5,
        SemanticTokenModifier::PropertyChain2 => 1 << 6,
        SemanticTokenModifier::PropertyChain3 => 1 << 7,
        SemanticTokenModifier::PropertyChain4 => 1 << 8,
    }
}

pub(super) fn lsp_inlay_hint_from_core(hint: InlayHint, source: &str) -> Value {
    let mut item = json!({
        "position": lsp_position_for_source(source, hint.position),
        "label": hint.label,
        "kind": lsp_inlay_hint_kind(hint.kind)
    });

    if hint.kind == InlayHintKind::Parameter {
        item["paddingRight"] = json!(true);
    }

    item
}

fn lsp_inlay_hint_kind(kind: InlayHintKind) -> u8 {
    match kind {
        InlayHintKind::Type => 1,
        InlayHintKind::Parameter => 2,
    }
}

fn lsp_location(uri: &str, span: SourceSpan, source: &str) -> Value {
    json!({
        "uri": uri,
        "range": lsp_range_for_source(source, span)
    })
}

pub(super) fn lsp_document_symbol_from_core(symbol: DocumentSymbol, source: &str) -> Value {
    let children = symbol
        .children
        .into_iter()
        .map(|child| lsp_document_symbol_from_core(child, source))
        .collect::<Vec<_>>();
    let mut item = json!({
        "name": symbol.name,
        "kind": lsp_symbol_kind(symbol.kind),
        "range": lsp_range_for_source(source, symbol.range),
        "selectionRange": lsp_range_for_source(source, symbol.selection_range),
    });

    if let Some(detail) = symbol.detail {
        item["detail"] = json!(detail);
    }

    if !children.is_empty() {
        item["children"] = Value::Array(children);
    }

    item
}

pub(super) fn lsp_selection_range_from_core(range: SelectionRange, source: &str) -> Value {
    let mut item = json!({
        "range": lsp_range_for_source(source, range.range)
    });

    if let Some(parent) = range.parent {
        item["parent"] = lsp_selection_range_from_core(*parent, source);
    }

    item
}

pub(super) fn lsp_folding_range_from_core(range: FoldingRange, source: &str) -> Value {
    json!({
        "startLine": range.range.start.line.saturating_sub(1),
        "startCharacter": utf16_column(source, range.range.start.line, range.range.start.column),
        "endLine": range.range.end.line.saturating_sub(1),
        "endCharacter": utf16_column(source, range.range.end.line, range.range.end.column),
        "kind": lsp_folding_range_kind(range.kind)
    })
}

fn lsp_folding_range_kind(kind: FoldingRangeKind) -> &'static str {
    match kind {
        FoldingRangeKind::Comment => "comment",
        FoldingRangeKind::Function | FoldingRangeKind::Table | FoldingRangeKind::TypeAlias => {
            "region"
        }
    }
}

fn lsp_symbol_kind(kind: DocumentSymbolKind) -> u8 {
    match kind {
        DocumentSymbolKind::Class => 5,
        DocumentSymbolKind::Method => 6,
        DocumentSymbolKind::Field => 8,
        DocumentSymbolKind::Constructor => 9,
        DocumentSymbolKind::Function => 12,
        DocumentSymbolKind::Local | DocumentSymbolKind::Parameter => 13,
        DocumentSymbolKind::ModuleExports => 19,
        DocumentSymbolKind::TypeAlias => 23,
    }
}

// Core columns count Unicode scalar values; LSP defaults to UTF-16 code units.
// Keep the raw scalar serializers below only for server-owned resolve data.
fn utf16_column(source: &str, line: usize, column: usize) -> usize {
    if source.is_empty() {
        return column.saturating_sub(1);
    }
    source
        .split('\n')
        .nth(line.saturating_sub(1))
        .map(|text| {
            text.chars()
                .take(column.saturating_sub(1))
                .map(char::len_utf16)
                .sum()
        })
        .unwrap_or_else(|| column.saturating_sub(1))
}

pub(super) fn lsp_position_for_source(source: &str, position: SourcePosition) -> Value {
    json!({ "line": position.line.saturating_sub(1),
        "character": utf16_column(source, position.line, position.column) })
}

pub(super) fn lsp_range_for_source(source: &str, span: SourceSpan) -> Value {
    json!({ "start": lsp_position_for_source(source, span.start),
        "end": lsp_position_for_source(source, span.end) })
}

pub(super) fn source_position_for_source(source: &str, position: &Value) -> Option<SourcePosition> {
    if source.is_empty() {
        return source_position_from_lsp(position);
    }
    let line = usize::try_from(position.get("line")?.as_u64()?).ok()?;
    let requested = usize::try_from(position.get("character")?.as_u64()?).ok()?;
    let text = source.split('\n').nth(line)?.trim_end_matches('\r');
    let mut units = 0;
    let mut column = 1;
    for ch in text.chars() {
        if units >= requested {
            break;
        }
        units += ch.len_utf16();
        if units > requested {
            return None;
        } // A cursor cannot split a surrogate pair.
        column += 1;
    }
    Some(SourcePosition {
        line: line.checked_add(1)?,
        column,
    })
}

pub(super) fn source_range_for_source(source: &str, range: &Value) -> Option<SourceRange> {
    let position = |value: &Value| {
        let requested_line = usize::try_from(value.get("line")?.as_u64()?).ok()?;
        let (last_line, last_text) = source.split('\n').enumerate().last()?;
        if requested_line > last_line {
            // Clients commonly request whole-document assists with an oversized
            // end line. Clamp to EOF rather than silently dropping the action.
            Some(SourcePosition {
                line: last_line.checked_add(1)?,
                column: last_text
                    .trim_end_matches('\r')
                    .chars()
                    .count()
                    .checked_add(1)?,
            })
        } else {
            source_position_for_source(source, value)
        }
    };
    Some(SourceRange {
        start: position(range.get("start")?)?,
        end: position(range.get("end")?)?,
    })
}

pub(super) fn lsp_range(span: SourceSpan) -> Value {
    json!({
        "start": {
            "line": span.start.line.saturating_sub(1),
            "character": span.start.column.saturating_sub(1)
        },
        "end": {
            "line": span.end.line.saturating_sub(1),
            "character": span.end.column.saturating_sub(1)
        }
    })
}

pub(super) fn lsp_inline_completion_from_core(
    candidate: InlineCompletionCandidate,
    range: Value,
) -> Value {
    json!({
        "insertText": candidate.insert_text,
        "isSnippet": candidate.is_snippet,
        "range": range,
        "source": candidate.source.as_str(),
        "score": candidate.score.0,
        "patternId": candidate.pattern_id.map(|pattern| pattern.0)
    })
}

fn lsp_position(position: SourcePosition) -> Value {
    json!({
        "line": position.line.saturating_sub(1),
        "character": position.column.saturating_sub(1)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SourceRange {
    pub(super) start: SourcePosition,
    pub(super) end: SourcePosition,
}

impl SourceRange {
    pub(super) fn contains(self, position: SourcePosition) -> bool {
        position_key(self.start) <= position_key(position)
            && position_key(position) <= position_key(self.end)
    }
}

pub(super) fn lsp_source_range(range: &Value) -> Option<SourceRange> {
    Some(SourceRange {
        start: source_position_from_lsp(range.get("start")?)?,
        end: source_position_from_lsp(range.get("end")?)?,
    })
}

pub(super) fn source_position_from_lsp(position: &Value) -> Option<SourcePosition> {
    Some(SourcePosition {
        line: usize::try_from(position.get("line")?.as_u64()?)
            .ok()?
            .checked_add(1)?,
        column: usize::try_from(position.get("character")?.as_u64()?)
            .ok()?
            .checked_add(1)?,
    })
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}
