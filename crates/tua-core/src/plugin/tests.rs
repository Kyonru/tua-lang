use super::*;
use crate::{CompletionKind, DiagnosticCode, DiagnosticSeverity, SourcePosition, SourceSpan};

#[derive(Debug)]
struct TestPlugin;

impl TuaPlugin for TestPlugin {
    fn name(&self) -> &'static str {
        "tua.test"
    }

    fn declarations(&self) -> Vec<PluginDeclaration> {
        vec![
            PluginDeclaration::new("sprite", PluginDeclarationKind::TopLevelDeclaration)
                .with_documentation("test declaration"),
        ]
    }

    fn diagnostics(&self, context: PluginContext<'_>) -> Vec<Diagnostic> {
        if context.source.contains("plugin_problem") {
            vec![Diagnostic::new(
                context.path,
                1,
                1,
                DiagnosticSeverity::Warning,
                "test plugin diagnostic",
            )
            .with_code(DiagnosticCode::PLUGIN_DIAGNOSTIC)]
        } else {
            Vec::new()
        }
    }

    fn completions(&self, context: PluginCompletionContext<'_>) -> Vec<crate::Completion> {
        if context.position.line == 1 {
            vec![crate::Completion {
                label: "pluginItem".to_owned(),
                kind: CompletionKind::Snippet,
                detail: Some("plugin completion".to_owned()),
                documentation: None,
                insert_text: Some("pluginItem".to_owned()),
                is_snippet: false,
                module_path: None,
                source_module: Some(self.name().to_owned()),
                additional_text_edits: Vec::new(),
            }]
        } else {
            Vec::new()
        }
    }

    fn hovers(&self, context: PluginHoverContext<'_>) -> Vec<PluginHover> {
        if context.position.line == 1 {
            vec![PluginHover {
                contents: "plugin hover".to_owned(),
                span: None,
                documentation: Some("plugin docs".to_owned()),
            }]
        } else {
            Vec::new()
        }
    }

    fn lowering_requests(&self, context: PluginContext<'_>) -> Vec<PluginLoweringRequest> {
        if context.source.contains("plugin_lower") {
            vec![PluginLoweringRequest {
                span: SourceSpan {
                    start: SourcePosition { line: 1, column: 1 },
                    end: SourcePosition {
                        line: 1,
                        column: 13,
                    },
                },
                operation: PluginLoweringOperation::RequireModule {
                    module: "plugin.runtime".to_owned(),
                },
            }]
        } else {
            Vec::new()
        }
    }
}

#[test]
fn registry_collects_plugin_contributions() {
    let mut registry = PluginRegistry::new();

    registry.register(TestPlugin);

    assert!(!registry.is_empty());
    assert_eq!(registry.plugins().next().unwrap().name(), "tua.test");
    assert_eq!(registry.declarations().len(), 1);

    let path = std::path::Path::new("src/main.tua");
    let diagnostics = registry.diagnostics(PluginContext {
        path,
        source: "plugin_problem()",
        parsed: None,
    });

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::PLUGIN_DIAGNOSTIC));
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);

    let completions = registry.completions(PluginCompletionContext {
        path,
        source: "",
        parsed: None,
        position: SourcePosition { line: 1, column: 1 },
    });

    assert_eq!(completions[0].label, "pluginItem");

    let hovers = registry.hovers(PluginHoverContext {
        path,
        source: "",
        parsed: None,
        position: SourcePosition { line: 1, column: 1 },
    });

    assert_eq!(hovers[0].contents, "plugin hover");

    let lowering_requests = registry.lowering_requests(PluginContext {
        path,
        source: "plugin_lower",
        parsed: None,
    });

    assert_eq!(
        lowering_requests[0].operation,
        PluginLoweringOperation::RequireModule {
            module: "plugin.runtime".to_owned()
        }
    );
}
