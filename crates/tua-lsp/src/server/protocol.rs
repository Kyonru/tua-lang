use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{json, Value};
use tua_core::{
    AssetInventory, CodeAction, CodeActionEdit, CodeActionKind, CodeLens, CodeLensReferenceKind,
    CodeLensReferenceTarget, CodeLensTarget, Completion, CompletionKind, Definition, Diagnostic,
    DiagnosticCode, DiagnosticSeverity, DocumentHighlight, DocumentHighlightKind, DocumentSymbol,
    DocumentSymbolKind, EcsAccessKind, EcsComponentUse, EcsGraph, EcsInsightKind, EcsQueryKind,
    FoldingRange, FoldingRangeKind, Hover, InlayHint, InlayHintKind, InspectorItem,
    ModuleDependencyGraph, ModuleExportOrigin, ModuleLink, ModuleSpecifierKind, ObjectPoolGraph,
    ObjectPoolInsightKind, ObjectPoolOperationKind, Reference, RenameEdit, RenameTarget,
    RequireResolution, SelectionRange, SemanticToken, SemanticTokenKind, SemanticTokenModifier,
    SignalGraph, SignalGraphCallRole, SignalGraphParameter, SignatureHelp, SignatureInformation,
    SourcePosition, SourceSpan, TextEdit, TypeExplanation, TypeExplanationStepKind,
    WorkspaceSymbol, WorkspaceSymbolKind,
};

use super::analysis::{
    CallHierarchyIdentity, CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyItemKind,
    CallHierarchyOutgoingCall,
};
use super::uri::uri_from_path;

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

pub(super) fn lsp_diagnostic_from_core(diagnostic: Diagnostic) -> Value {
    let start_line = diagnostic.line.saturating_sub(1);
    let start_character = diagnostic.column.saturating_sub(1);
    let end_line = diagnostic.end_line.saturating_sub(1);
    let end_character = diagnostic.end_column.saturating_sub(1);

    json!({
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
    })
}

pub(super) fn core_diagnostic_from_lsp(
    path: &std::path::Path,
    diagnostic: &Value,
) -> Option<Diagnostic> {
    let start = diagnostic.get("range")?.get("start")?;
    let position = source_position_from_lsp(start)?;
    let end = diagnostic
        .get("range")
        .and_then(|range| range.get("end"))
        .and_then(source_position_from_lsp)
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

pub(super) fn lsp_hover_from_core(hover: Hover) -> Value {
    let mut markdown = format!("```tua\n{}\n```", hover.contents);

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
        result["range"] = lsp_range(span);
    }

    result
}

pub(super) fn lsp_signal_graph_from_core(graph: SignalGraph) -> Value {
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
                        "range": lsp_range(call.span),
                        "callRange": lsp_range(call.call_span),
                        "insertionPosition": lsp_position(call.insertion_position),
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

pub(super) fn lsp_asset_inventory_from_core(inventory: AssetInventory) -> Value {
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
                        "range": lsp_range(reference.span),
                        "loaderKind": reference.loader_kind.name()
                    })
                }).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>(),
        "diagnostics": inventory.diagnostics.into_iter().map(|diagnostic| {
            let uri = uri_from_path(&diagnostic.file);

            json!({
                "uri": uri,
                "diagnostic": lsp_diagnostic_from_core(diagnostic)
            })
        }).collect::<Vec<_>>()
    })
}

pub(super) fn lsp_module_dependency_graph_from_core(graph: ModuleDependencyGraph) -> Value {
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
                        "range": lsp_range(export.definition_span),
                        "exportRange": lsp_range(export.export_span)
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
                                "range": lsp_range(step.span)
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
                "range": lsp_range(edge.span),
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

pub(super) fn lsp_ecs_graph_from_core(graph: EcsGraph) -> Value {
    json!({
        "components": graph.components.into_iter().map(|component| {
            json!({
                "id": component.id,
                "uri": uri_from_path(&component.path),
                "localName": component.local_name,
                "runtimeName": component.runtime_name,
                "range": lsp_range(component.span),
                "nameRange": lsp_range(component.name_span),
                "fields": component.fields.into_iter().map(|field| {
                    json!({
                        "name": field.name,
                        "type": field.type_label,
                        "range": lsp_range(field.span)
                    })
                }).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>(),
        "systems": graph.systems.into_iter().map(|system| {
            json!({
                "id": system.id,
                "uri": uri_from_path(&system.path),
                "name": system.name,
                "range": lsp_range(system.span),
                "nameRange": system.name_span.map(lsp_range),
                "callbackRange": system.callback_span.map(lsp_range),
                "components": system.components.into_iter().map(lsp_ecs_component_use).collect::<Vec<_>>()
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
                "range": lsp_range(query.span),
                "nameRange": query.name_span.map(lsp_range),
                "components": query.components.into_iter().map(lsp_ecs_component_use).collect::<Vec<_>>()
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
                "range": lsp_range(insight.span),
                "subjectId": insight.subject_id
            })
        }).collect::<Vec<_>>()
    })
}

pub(super) fn lsp_object_pool_graph_from_core(graph: ObjectPoolGraph) -> Value {
    json!({
        "pools": graph.pools.into_iter().map(|pool| {
            json!({
                "id": pool.id,
                "uri": uri_from_path(&pool.path),
                "name": pool.name,
                "itemType": pool.item_type_label,
                "range": lsp_range(pool.span),
                "nameRange": lsp_range(pool.name_span)
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
                "range": lsp_range(operation.span),
                "itemRange": operation.item_span.map(lsp_range)
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
                "range": lsp_range(insight.span),
                "poolId": insight.pool_id,
                "acquireOperationId": insight.acquire_operation_id,
                "itemName": insight.item_name
            })
        }).collect::<Vec<_>>()
    })
}

fn lsp_ecs_component_use(component: EcsComponentUse) -> Value {
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
        "range": lsp_range(component.span)
    })
}

fn module_specifier_kind_name(kind: ModuleSpecifierKind) -> &'static str {
    match kind {
        ModuleSpecifierKind::Require => "require",
        ModuleSpecifierKind::Import => "import",
    }
}

pub(super) fn lsp_inspector_items_from_core(items: Vec<InspectorItem>) -> Value {
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
                            "range": lsp_range(image.span)
                        })
                    });
                    let frame = sprite.frame.map(|frame| {
                        json!({
                            "width": frame.width,
                            "height": frame.height,
                            "left": frame.left,
                            "top": frame.top,
                            "border": frame.border,
                            "range": lsp_range(frame.span)
                        })
                    });

                    json!({
                        "kind": "spriteSheet",
                        "name": sprite.name,
                        "range": lsp_range(sprite.span),
                        "selectionRange": lsp_range(sprite.name_span),
                        "image": image,
                        "frame": frame,
                        "animations": sprite.animations.into_iter().map(|animation| {
                            json!({
                                "name": animation.name,
                                "from": animation.from,
                                "to": animation.to,
                                "row": animation.row,
                                "column": animation.column,
                                "frame": animation.frame.map(|frame| json!({
                                    "width": frame.width,
                                    "height": frame.height,
                                    "left": frame.left,
                                    "top": frame.top,
                                    "border": frame.border,
                                    "range": lsp_range(frame.span)
                                })),
                                "fps": animation.fps,
                                "durations": animation.durations,
                                "onLoop": animation.on_loop,
                                "loop": animation.looping,
                                "range": lsp_range(animation.span),
                                "selectionRange": lsp_range(animation.name_span)
                            })
                        }).collect::<Vec<_>>(),
                        "diagnostics": sprite.diagnostics.into_iter()
                            .map(lsp_diagnostic_from_core)
                            .collect::<Vec<_>>()
                    })
                }
            })
            .collect(),
    )
}

pub(super) fn lsp_type_explanation_from_core(explanation: TypeExplanation) -> Value {
    json!({
        "expression": explanation.expression,
        "type": explanation.ty.to_string(),
        "isAny": matches!(explanation.ty, tua_core::Type::Any),
        "range": lsp_range(explanation.span),
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
                "range": step.span.map(lsp_range),
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

pub(super) fn lsp_completion_from_core(
    uri: &str,
    position: SourcePosition,
    completion: Completion,
    label_details_support: bool,
) -> Value {
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
            .map(lsp_text_edit)
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

pub(super) fn lsp_location_from_core(definition: Definition) -> Value {
    lsp_location(&uri_from_path(&definition.path), definition.span)
}

pub(super) fn lsp_call_hierarchy_item(item: CallHierarchyItem) -> Value {
    json!({
        "name": item.name,
        "kind": lsp_call_hierarchy_kind(item.kind),
        "uri": uri_from_path(&item.path),
        "range": lsp_range(item.range),
        "selectionRange": lsp_range(item.selection_range),
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

pub(super) fn lsp_call_hierarchy_incoming_call(call: CallHierarchyIncomingCall) -> Value {
    json!({
        "from": lsp_call_hierarchy_item(call.from),
        "fromRanges": call
            .from_ranges
            .into_iter()
            .map(lsp_range)
            .collect::<Vec<_>>()
    })
}

pub(super) fn lsp_call_hierarchy_outgoing_call(call: CallHierarchyOutgoingCall) -> Value {
    json!({
        "to": lsp_call_hierarchy_item(call.to),
        "fromRanges": call
            .from_ranges
            .into_iter()
            .map(lsp_range)
            .collect::<Vec<_>>()
    })
}

fn lsp_call_hierarchy_kind(kind: CallHierarchyItemKind) -> u8 {
    match kind {
        CallHierarchyItemKind::Function => 12,
        CallHierarchyItemKind::Method => 6,
    }
}

pub(super) fn lsp_document_link_from_core(link: ModuleLink) -> Value {
    json!({
        "range": lsp_range(link.span),
        "target": uri_from_path(&link.target)
    })
}

pub(super) fn lsp_document_highlight_from_core(highlight: DocumentHighlight) -> Value {
    json!({
        "range": lsp_range(highlight.span),
        "kind": lsp_document_highlight_kind(highlight.kind)
    })
}

fn lsp_document_highlight_kind(kind: DocumentHighlightKind) -> u8 {
    match kind {
        DocumentHighlightKind::Text => 1,
    }
}

pub(super) fn lsp_reference_location(reference: Reference) -> Value {
    lsp_location(&uri_from_path(&reference.path), reference.span)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CodeLensResolveData {
    pub(super) target: CodeLensReferenceTarget,
}

pub(super) fn lsp_code_lens_from_core(lens: CodeLens) -> Value {
    match lens.target {
        CodeLensTarget::References(target) => json!({
            "range": lsp_range(lens.range),
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
) -> Value {
    let title = reference_code_lens_title(references.len());
    let locations = references
        .into_iter()
        .map(lsp_reference_location)
        .collect::<Vec<_>>();

    lens["command"] = json!({
        "title": title,
        "command": "editor.action.showReferences",
        "arguments": [
            uri_from_path(&target.path),
            lsp_position(target.position),
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

pub(super) fn lsp_prepare_rename_from_core(target: RenameTarget) -> Value {
    json!({
        "range": lsp_range(target.span),
        "placeholder": target.placeholder
    })
}

pub(super) fn lsp_code_action_from_core(action: CodeAction) -> Value {
    json!({
        "title": action.title,
        "kind": lsp_code_action_kind(action.kind),
        "edit": lsp_code_action_edit(action.edit)
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

fn lsp_code_action_edit(edit: CodeActionEdit) -> Value {
    let raw_changes = edit.changes;
    let changes = lsp_changes(raw_changes.clone());

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

    for (uri, edits) in changes_for_document_changes(raw_changes) {
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

pub(super) fn lsp_workspace_edit(edit: RenameEdit) -> Value {
    json!({
        "changes": lsp_changes(group_text_edits(edit.edits))
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
) -> BTreeMap<String, Vec<Value>> {
    changes
        .into_iter()
        .map(|(path, edits)| {
            (
                uri_from_path(&path),
                edits.into_iter().map(lsp_text_edit).collect(),
            )
        })
        .collect()
}

fn changes_for_document_changes(
    changes: BTreeMap<std::path::PathBuf, Vec<TextEdit>>,
) -> Vec<(String, Vec<Value>)> {
    lsp_changes(changes).into_iter().collect()
}

pub(super) fn lsp_text_edit(edit: TextEdit) -> Value {
    json!({
        "range": lsp_range(edit.span),
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
            character += 1;
        }
    }

    (line, character)
}

pub(super) fn lsp_workspace_symbol_from_core(symbol: WorkspaceSymbol) -> Value {
    let mut item = json!({
        "name": symbol.name,
        "kind": lsp_workspace_symbol_kind(symbol.kind),
        "location": {
            "uri": uri_from_path(&symbol.path),
            "range": lsp_range(symbol.selection_range)
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

pub(super) fn lsp_semantic_token_data_from_core(tokens: Vec<SemanticToken>) -> Vec<usize> {
    let mut data = Vec::with_capacity(tokens.len() * 5);
    let mut previous_line = 0usize;
    let mut previous_start = 0usize;

    for token in tokens {
        if token.span.start.line != token.span.end.line {
            continue;
        }

        let line = token.span.start.line.saturating_sub(1);
        let start = token.span.start.column.saturating_sub(1);
        let length = token
            .span
            .end
            .column
            .saturating_sub(token.span.start.column);
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

pub(super) fn lsp_semantic_tokens_from_core(tokens: Vec<SemanticToken>) -> Value {
    lsp_semantic_tokens(lsp_semantic_token_data_from_core(tokens), None)
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
    }
}

pub(super) fn lsp_inlay_hint_from_core(hint: InlayHint) -> Value {
    let mut item = json!({
        "position": lsp_position(hint.position),
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

fn lsp_location(uri: &str, span: SourceSpan) -> Value {
    json!({
        "uri": uri,
        "range": lsp_range(span)
    })
}

pub(super) fn lsp_document_symbol_from_core(symbol: DocumentSymbol) -> Value {
    let children = symbol
        .children
        .into_iter()
        .map(lsp_document_symbol_from_core)
        .collect::<Vec<_>>();
    let mut item = json!({
        "name": symbol.name,
        "kind": lsp_symbol_kind(symbol.kind),
        "range": lsp_range(symbol.range),
        "selectionRange": lsp_range(symbol.selection_range),
    });

    if let Some(detail) = symbol.detail {
        item["detail"] = json!(detail);
    }

    if !children.is_empty() {
        item["children"] = Value::Array(children);
    }

    item
}

pub(super) fn lsp_selection_range_from_core(range: SelectionRange) -> Value {
    let mut item = json!({
        "range": lsp_range(range.range)
    });

    if let Some(parent) = range.parent {
        item["parent"] = lsp_selection_range_from_core(*parent);
    }

    item
}

pub(super) fn lsp_folding_range_from_core(range: FoldingRange) -> Value {
    json!({
        "startLine": range.range.start.line.saturating_sub(1),
        "startCharacter": range.range.start.column.saturating_sub(1),
        "endLine": range.range.end.line.saturating_sub(1),
        "endCharacter": range.range.end.column.saturating_sub(1),
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

fn lsp_range(span: SourceSpan) -> Value {
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
        line: position.get("line")?.as_u64()? as usize + 1,
        column: position.get("character")?.as_u64()? as usize + 1,
    })
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}
