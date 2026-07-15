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

pub(super) fn initialize_result(
    client_pull_diagnostics: bool,
    client_completion_label_details: bool,
) -> Value {
    let completion_trigger_characters = completion_trigger_characters();
    let mut result = json!({
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
                "triggerCharacters": completion_trigger_characters,
                "resolveProvider": true
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
            "codeActionProvider": {
                "codeActionKinds": [
                    "quickfix",
                    "source.organizeImports",
                    "source.sortImports",
                    "source.removeUnusedImports",
                    "source.fixAll"
                ]
            },
            "codeLensProvider": {
                "resolveProvider": true
            },
            "documentFormattingProvider": true,
            "documentRangeFormattingProvider": true,
            "documentOnTypeFormattingProvider": {
                "firstTriggerCharacter": "\n",
                "moreTriggerCharacter": ["}", ")", "]", ",", "d"]
            },
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
                "full": {
                    "delta": true
                },
                "range": true
            },
            "workspace": {
                "workspaceFolders": {
                    "supported": true,
                    "changeNotifications": true
                },
                "fileOperations": {
                    "willRename": {
                        "filters": [
                            {
                                "scheme": "file",
                                "pattern": {
                                    "glob": "**/*"
                                }
                            }
                        ]
                    }
                }
            }
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": tua_core::VERSION
        }
    });

    if client_pull_diagnostics {
        result["capabilities"]["diagnosticProvider"] = json!({
            "interFileDependencies": true,
            "workspaceDiagnostics": false
        });
    }

    if client_completion_label_details {
        result["capabilities"]["completionProvider"]["completionItem"] = json!({
            "labelDetailsSupport": true
        });
    }

    result
}

fn completion_trigger_characters() -> Vec<String> {
    let mut characters = [":", ".", "\"", "'", "@", "-", "_"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();

    characters.extend(('a'..='z').map(|character| character.to_string()));
    characters.extend(('A'..='Z').map(|character| character.to_string()));
    characters.extend(('0'..='9').map(|character| character.to_string()));

    characters
}
