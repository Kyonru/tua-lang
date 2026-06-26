use std::fmt;
use std::path::Path;
use std::sync::Arc;

use crate::{Completion, Diagnostic, ParsedSource, SourcePosition, SourceSpan, TextEdit};

#[derive(Clone, Debug, Default)]
pub struct PluginRegistry {
    plugins: Vec<Arc<dyn TuaPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_plugin(mut self, plugin: impl TuaPlugin + 'static) -> Self {
        self.register(plugin);
        self
    }

    pub fn register(&mut self, plugin: impl TuaPlugin + 'static) {
        self.plugins.push(Arc::new(plugin));
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub fn plugins(&self) -> impl Iterator<Item = &dyn TuaPlugin> {
        self.plugins.iter().map(Arc::as_ref)
    }

    pub fn declarations(&self) -> Vec<PluginDeclaration> {
        self.plugins()
            .flat_map(|plugin| plugin.declarations().into_iter())
            .collect()
    }

    pub fn diagnostics(&self, context: PluginContext<'_>) -> Vec<Diagnostic> {
        self.plugins()
            .flat_map(|plugin| plugin.diagnostics(context).into_iter())
            .collect()
    }

    pub fn completions(&self, context: PluginCompletionContext<'_>) -> Vec<Completion> {
        self.plugins()
            .flat_map(|plugin| plugin.completions(context).into_iter())
            .collect()
    }

    pub fn hovers(&self, context: PluginHoverContext<'_>) -> Vec<PluginHover> {
        self.plugins()
            .flat_map(|plugin| plugin.hovers(context).into_iter())
            .collect()
    }

    pub fn lowering_requests(&self, context: PluginContext<'_>) -> Vec<PluginLoweringRequest> {
        self.plugins()
            .flat_map(|plugin| plugin.lowering_requests(context).into_iter())
            .collect()
    }
}

pub trait TuaPlugin: fmt::Debug + Send + Sync {
    fn name(&self) -> &'static str;

    fn declarations(&self) -> Vec<PluginDeclaration> {
        Vec::new()
    }

    fn diagnostics(&self, _context: PluginContext<'_>) -> Vec<Diagnostic> {
        Vec::new()
    }

    fn completions(&self, _context: PluginCompletionContext<'_>) -> Vec<Completion> {
        Vec::new()
    }

    fn hovers(&self, _context: PluginHoverContext<'_>) -> Vec<PluginHover> {
        Vec::new()
    }

    fn lowering_requests(&self, _context: PluginContext<'_>) -> Vec<PluginLoweringRequest> {
        Vec::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginDeclaration {
    pub keyword: String,
    pub kind: PluginDeclarationKind,
    pub documentation: Option<String>,
}

impl PluginDeclaration {
    pub fn new(keyword: impl Into<String>, kind: PluginDeclarationKind) -> Self {
        Self {
            keyword: keyword.into(),
            kind,
            documentation: None,
        }
    }

    pub fn with_documentation(mut self, documentation: impl Into<String>) -> Self {
        self.documentation = Some(documentation.into());
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PluginDeclarationKind {
    TopLevelDeclaration,
    DomainBlock,
}

#[derive(Clone, Copy, Debug)]
pub struct PluginContext<'a> {
    pub path: &'a Path,
    pub source: &'a str,
    pub parsed: Option<&'a ParsedSource>,
}

#[derive(Clone, Copy, Debug)]
pub struct PluginCompletionContext<'a> {
    pub path: &'a Path,
    pub source: &'a str,
    pub parsed: Option<&'a ParsedSource>,
    pub position: SourcePosition,
}

#[derive(Clone, Copy, Debug)]
pub struct PluginHoverContext<'a> {
    pub path: &'a Path,
    pub source: &'a str,
    pub parsed: Option<&'a ParsedSource>,
    pub position: SourcePosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginHover {
    pub contents: String,
    pub span: Option<SourceSpan>,
    pub documentation: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginLoweringRequest {
    pub span: SourceSpan,
    pub operation: PluginLoweringOperation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginLoweringOperation {
    RequireModule {
        module: String,
    },
    Call {
        callee: String,
        arguments: Vec<PluginLoweringValue>,
    },
    InsertText {
        edit: TextEdit,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginLoweringValue {
    String(String),
    Number(String),
    Boolean(bool),
    Identifier(String),
}

#[cfg(test)]
mod tests;
