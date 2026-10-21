#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestKind {
    Initialize,
    Shutdown,
    Hover,
    Completion,
    CompletionResolve,
    SignatureHelp,
    Definition,
    TypeDefinition,
    Implementation,
    Diagnostic,
    BuiltinDocument,
    ProjectLayout,
    CacheStats,
    Health,
    ResetCompletionStats,
    LocalInlineCompletion,
    InlineCompletionContext,
    ValidateInlineCompletion,
    Inspect,
    ExplainType,
    AssetInventory,
    ModuleGraph,
    EcsGraph,
    ObjectPoolGraph,
    AudioGraph,
    SignalGraph,
    PrepareCallHierarchy,
    IncomingCalls,
    OutgoingCalls,
    DocumentLink,
    DocumentHighlight,
    DocumentSymbol,
    SelectionRange,
    FoldingRange,
    CodeAction,
    CodeLens,
    CodeLensResolve,
    Formatting,
    RangeFormatting,
    OnTypeFormatting,
    References,
    PrepareRename,
    Rename,
    InlayHint,
    SemanticTokensFull,
    SemanticTokensDelta,
    SemanticTokensRange,
    WorkspaceSymbol,
    WillRenameFiles,
}

impl RequestKind {
    pub(super) fn from_method(method: &str) -> Option<Self> {
        Some(match method {
            "initialize" => Self::Initialize,
            "shutdown" => Self::Shutdown,
            "textDocument/hover" => Self::Hover,
            "textDocument/completion" => Self::Completion,
            "completionItem/resolve" => Self::CompletionResolve,
            "textDocument/signatureHelp" => Self::SignatureHelp,
            "textDocument/definition" => Self::Definition,
            "textDocument/typeDefinition" => Self::TypeDefinition,
            "textDocument/implementation" => Self::Implementation,
            "textDocument/diagnostic" => Self::Diagnostic,
            "tua/builtinDocument" => Self::BuiltinDocument,
            "tua/projectLayout" => Self::ProjectLayout,
            "tua/cacheStats" => Self::CacheStats,
            "tua/health" => Self::Health,
            "tua/resetCompletionStats" => Self::ResetCompletionStats,
            "tua/localInlineCompletion" => Self::LocalInlineCompletion,
            "tua/inlineCompletionContext" => Self::InlineCompletionContext,
            "tua/validateInlineCompletion" => Self::ValidateInlineCompletion,
            "tua/inspect" => Self::Inspect,
            "tua/explainType" => Self::ExplainType,
            "tua/assetInventory" => Self::AssetInventory,
            "tua/moduleGraph" => Self::ModuleGraph,
            "tua/ecsGraph" => Self::EcsGraph,
            "tua/objectPoolGraph" => Self::ObjectPoolGraph,
            "tua/audioGraph" => Self::AudioGraph,
            "tua/signalGraph" => Self::SignalGraph,
            "textDocument/prepareCallHierarchy" => Self::PrepareCallHierarchy,
            "callHierarchy/incomingCalls" => Self::IncomingCalls,
            "callHierarchy/outgoingCalls" => Self::OutgoingCalls,
            "textDocument/documentLink" => Self::DocumentLink,
            "textDocument/documentHighlight" => Self::DocumentHighlight,
            "textDocument/documentSymbol" => Self::DocumentSymbol,
            "textDocument/selectionRange" => Self::SelectionRange,
            "textDocument/foldingRange" => Self::FoldingRange,
            "textDocument/codeAction" => Self::CodeAction,
            "textDocument/codeLens" => Self::CodeLens,
            "codeLens/resolve" => Self::CodeLensResolve,
            "textDocument/formatting" => Self::Formatting,
            "textDocument/rangeFormatting" => Self::RangeFormatting,
            "textDocument/onTypeFormatting" => Self::OnTypeFormatting,
            "textDocument/references" => Self::References,
            "textDocument/prepareRename" => Self::PrepareRename,
            "textDocument/rename" => Self::Rename,
            "textDocument/inlayHint" => Self::InlayHint,
            "textDocument/semanticTokens/full" => Self::SemanticTokensFull,
            "textDocument/semanticTokens/full/delta" => Self::SemanticTokensDelta,
            "textDocument/semanticTokens/range" => Self::SemanticTokensRange,
            "workspace/symbol" => Self::WorkspaceSymbol,
            "workspace/willRenameFiles" => Self::WillRenameFiles,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NotificationKind {
    Initialized,
    Exit,
    DidOpen,
    DidChange,
    DidSave,
    DidClose,
    DidChangeWatchedFiles,
    DidChangeWorkspaceFolders,
    DidChangeConfiguration,
    SetTrace,
}

impl NotificationKind {
    pub(super) fn from_method(method: &str) -> Option<Self> {
        Some(match method {
            "initialized" => Self::Initialized,
            "exit" => Self::Exit,
            "textDocument/didOpen" => Self::DidOpen,
            "textDocument/didChange" => Self::DidChange,
            "textDocument/didSave" => Self::DidSave,
            "textDocument/didClose" => Self::DidClose,
            "workspace/didChangeWatchedFiles" => Self::DidChangeWatchedFiles,
            "workspace/didChangeWorkspaceFolders" => Self::DidChangeWorkspaceFolders,
            "workspace/didChangeConfiguration" => Self::DidChangeConfiguration,
            "tua/setTrace" => Self::SetTrace,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_protocol_methods_without_accepting_unknown_extensions() {
        assert_eq!(
            RequestKind::from_method("textDocument/completion"),
            Some(RequestKind::Completion)
        );
        assert_eq!(
            NotificationKind::from_method("textDocument/didChange"),
            Some(NotificationKind::DidChange)
        );
        assert_eq!(
            RequestKind::from_method("tua/localInlineCompletion"),
            Some(RequestKind::LocalInlineCompletion)
        );
        assert_eq!(
            RequestKind::from_method("tua/inlineCompletionContext"),
            Some(RequestKind::InlineCompletionContext)
        );
        assert_eq!(
            RequestKind::from_method("tua/validateInlineCompletion"),
            Some(RequestKind::ValidateInlineCompletion)
        );
        assert_eq!(
            RequestKind::from_method("tua/projectLayout"),
            Some(RequestKind::ProjectLayout)
        );
        assert_eq!(RequestKind::from_method("tua/unknown"), None);
        assert_eq!(NotificationKind::from_method("tua/unknown"), None);
    }
}
