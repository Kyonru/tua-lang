use super::*;
use crate::{
    AssetLoaderKind, AssetMetadata, AssetValidationStatus, DiagnosticSeverity, SourcePosition,
    SourceSpan,
};

#[derive(Debug)]
struct TestPlugin;

impl TuaPlugin for TestPlugin {
    fn name(&self) -> &'static str {
        "tua.test"
    }

    fn diagnostics(&self, context: PluginContext<'_>) -> Vec<Diagnostic> {
        if context.source.contains("plugin_problem")
            || (context.source.contains("plugin_asset")
                && context
                    .assets
                    .iter()
                    .any(|asset| asset.relative_path == "assets/player.png"))
        {
            let message = if context.source.contains("plugin_asset") {
                "test plugin saw asset metadata"
            } else {
                "test plugin diagnostic"
            };

            vec![
                Diagnostic::new(context.path, 1, 1, DiagnosticSeverity::Warning, message)
                    .with_code(DiagnosticCode::PLUGIN_DIAGNOSTIC),
            ]
        } else {
            Vec::new()
        }
    }
}

#[derive(Debug)]
struct OrderedPlugin {
    name: &'static str,
    code: Option<DiagnosticCode>,
}

impl TuaPlugin for OrderedPlugin {
    fn name(&self) -> &'static str {
        self.name
    }

    fn diagnostics(&self, _context: PluginContext<'_>) -> Vec<Diagnostic> {
        let diagnostic = Diagnostic::new(
            "src/other.tua",
            2,
            3,
            DiagnosticSeverity::Warning,
            self.name,
        );

        vec![match self.code {
            Some(code) => diagnostic.with_code(code),
            None => diagnostic,
        }]
    }
}

#[test]
fn registry_runs_validation_plugins_with_asset_metadata() {
    let mut registry = PluginRegistry::new();

    assert!(registry.register(TestPlugin));
    assert!(!registry.is_empty());
    assert!(registry.contains("tua.test"));
    assert_eq!(
        registry.plugin_names().collect::<Vec<_>>(),
        vec!["tua.test"]
    );

    let path = std::path::Path::new("src/main.tua");
    let diagnostics = registry.diagnostics(PluginContext {
        path,
        source: "plugin_problem()",
        parsed: None,
        assets: &[],
    });

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::PLUGIN_DIAGNOSTIC));
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);

    let asset_metadata = [AssetMetadata {
        kind: AssetLoaderKind::Image,
        relative_path: "assets/player.png".to_owned(),
        extension: Some("png".to_owned()),
        span: SourceSpan {
            start: SourcePosition {
                line: 1,
                column: 22,
            },
            end: SourcePosition {
                line: 1,
                column: 41,
            },
        },
        inferred_love_type: "Image",
        status: AssetValidationStatus::Valid,
        dimensions: None,
        duration_seconds: None,
    }];
    let asset_diagnostics = registry.diagnostics(PluginContext {
        path,
        source: "plugin_asset()",
        parsed: None,
        assets: &asset_metadata,
    });

    assert_eq!(asset_diagnostics.len(), 1);
    assert_eq!(
        asset_diagnostics[0].message,
        "test plugin saw asset metadata"
    );
}

#[test]
fn registry_rejects_duplicate_names_and_normalizes_diagnostics() {
    let mut registry = PluginRegistry::new();

    assert!(registry.register(OrderedPlugin {
        name: "z.plugin",
        code: None,
    }));
    assert!(registry.register(OrderedPlugin {
        name: "a.plugin",
        code: Some(DiagnosticCode::TYPE_MISMATCH),
    }));
    assert!(!registry.register(OrderedPlugin {
        name: "a.plugin",
        code: Some(DiagnosticCode::PLUGIN_SPRITE),
    }));
    assert!(!registry.register(OrderedPlugin {
        name: "",
        code: Some(DiagnosticCode::PLUGIN_SPRITE),
    }));

    assert_eq!(
        registry.plugin_names().collect::<Vec<_>>(),
        vec!["a.plugin", "z.plugin"]
    );

    let path = std::path::Path::new("src/main.tua");
    let diagnostics = registry.diagnostics(PluginContext {
        path,
        source: "",
        parsed: None,
        assets: &[],
    });

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        vec!["a.plugin", "z.plugin"]
    );
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.file == path && diagnostic.code == Some(DiagnosticCode::PLUGIN_DIAGNOSTIC)
    }));
}
