use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use crate::{AssetMetadata, Diagnostic, DiagnosticCategory, DiagnosticCode, ParsedSource};

#[derive(Clone, Debug, Default)]
pub struct PluginRegistry {
    plugins: BTreeMap<&'static str, Arc<dyn TuaPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_plugin(mut self, plugin: impl TuaPlugin + 'static) -> Self {
        let _ = self.register(plugin);
        self
    }

    /// Registers a statically linked plugin. Duplicate or empty names are rejected.
    pub fn register(&mut self, plugin: impl TuaPlugin + 'static) -> bool {
        let name = plugin.name();

        if name.is_empty() || self.plugins.contains_key(name) {
            return false;
        }

        self.plugins.insert(name, Arc::new(plugin));
        true
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    pub fn plugin_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.plugins.keys().copied()
    }

    pub fn plugins(&self) -> impl Iterator<Item = &dyn TuaPlugin> {
        self.plugins.values().map(Arc::as_ref)
    }

    pub fn diagnostics(&self, context: PluginContext<'_>) -> Vec<Diagnostic> {
        self.plugins()
            .flat_map(|plugin| plugin.diagnostics(context).into_iter())
            .map(|mut diagnostic| {
                diagnostic.file = context.path.to_path_buf();
                if diagnostic
                    .code
                    .is_none_or(|code| code.category() != DiagnosticCategory::Plugin)
                {
                    diagnostic.code = Some(DiagnosticCode::PLUGIN_DIAGNOSTIC);
                }
                diagnostic
            })
            .collect()
    }

    pub(crate) fn extend_missing(&mut self, registry: &Self) {
        for (name, plugin) in &registry.plugins {
            self.plugins
                .entry(name)
                .or_insert_with(|| Arc::clone(plugin));
        }
    }
}

pub trait TuaPlugin: fmt::Debug + Send + Sync {
    fn name(&self) -> &'static str;

    fn diagnostics(&self, _context: PluginContext<'_>) -> Vec<Diagnostic> {
        Vec::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PluginContext<'a> {
    pub path: &'a Path,
    pub source: &'a str,
    pub parsed: Option<&'a ParsedSource>,
    pub assets: &'a [AssetMetadata],
}

#[cfg(test)]
mod tests;
