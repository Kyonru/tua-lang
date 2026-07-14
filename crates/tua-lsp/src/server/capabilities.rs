use serde_json::{json, Value};
use tua_core::project_description;

pub(super) const SERVER_NAME: &str = "tua-lsp";

pub(super) const SEMANTIC_TOKEN_TYPES: [&str; 11] = [
    "typeAlias",
    "function",
    "variable",
    "parameter",
    "property",
    "builtinType",
    "unresolvedReference",
    "global",
    "module",
    "method",
    "annotationTag",
];

pub(super) const SEMANTIC_TOKEN_MODIFIERS: [&str; 3] = ["declaration", "readonly", "deprecated"];

pub(super) fn server_description() -> String {
    format!("{} language server", project_description())
}

pub(super) fn initialize_result() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": {
                "openClose": true,
                "change": 2,
                "save": {
                    "includeText": true
                }
            },
            "hoverProvider": true,
            "completionProvider": {
                "triggerCharacters": [":", ".", "\"", "'", "@", "-"],
                "resolveProvider": true,
                "completionItem": {
                    "labelDetailsSupport": true
                }
            },
            "signatureHelpProvider": {
                "triggerCharacters": ["(", ","],
                "retriggerCharacters": [")"]
            },
            "definitionProvider": true,
            "typeDefinitionProvider": true,
            "implementationProvider": true,
            "callHierarchyProvider": true,
            "documentLinkProvider": {
                "resolveProvider": false
            },
            "documentHighlightProvider": true,
            "documentSymbolProvider": true,
            "selectionRangeProvider": true,
            "foldingRangeProvider": true,
            "codeActionProvider": true,
            "documentFormattingProvider": true,
            "referencesProvider": true,
            "workspaceSymbolProvider": true,
            "renameProvider": {
                "prepareProvider": true
            },
            "inlayHintProvider": {
                "resolveProvider": false
            },
            "semanticTokensProvider": {
                "legend": {
                    "tokenTypes": SEMANTIC_TOKEN_TYPES,
                    "tokenModifiers": SEMANTIC_TOKEN_MODIFIERS
                },
                "full": true,
                "range": true
            },
            "workspace": {
                "workspaceFolders": {
                    "supported": true,
                    "changeNotifications": true
                }
            }
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": tua_core::VERSION
        }
    })
}
