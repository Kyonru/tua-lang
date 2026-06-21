use serde_json::{json, Value};
use tua_core::project_description;

pub(super) const SERVER_NAME: &str = "tua-lsp";

pub(super) const SEMANTIC_TOKEN_TYPES: [&str; 7] = [
    "typeAlias",
    "function",
    "variable",
    "parameter",
    "property",
    "builtinType",
    "unresolvedReference",
];

pub(super) const SEMANTIC_TOKEN_MODIFIERS: [&str; 1] = ["declaration"];

pub(super) fn server_description() -> String {
    format!("{} language server", project_description())
}

pub(super) fn initialize_result() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": 1,
            "hoverProvider": true,
            "completionProvider": {
                "triggerCharacters": [":", ".", "\"", "'"]
            },
            "definitionProvider": true,
            "documentSymbolProvider": true,
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
                "full": true
            }
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": tua_core::VERSION
        }
    })
}
