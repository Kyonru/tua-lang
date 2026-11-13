//! Diagnose ShaderScan paths that only exist before source-root relocation.

use super::*;
use crate::DiagnosticSeverity;
use full_moon::ast::{Call, FunctionArgs, Prefix, Suffix};

#[derive(Default)]
struct ShaderScanPaths<'a> {
    scope: Option<&'a crate::scope::ScopeMap>,
    scanners: BTreeSet<crate::scope::ScopeSymbolId>,
    loads: Vec<(crate::scope::ScopeSymbolId, String, SourceSpan)>,
    writes: Vec<(crate::scope::ScopeSymbolId, SourcePosition)>,
}

impl Visitor for ShaderScanPaths<'_> {
    fn visit_assignment(&mut self, assignment: &ast::Assignment) {
        let Some(scope) = self.scope else { return };
        for variable in assignment.variables() {
            let ast::Var::Name(name) = variable else {
                continue;
            };
            let Some(span) = node_span(name) else {
                continue;
            };
            if let Some(id) = scope.symbol_at_position(span.start) {
                self.writes.push((id, span.start));
            }
        }
    }

    fn visit_local_assignment(&mut self, local: &ast::LocalAssignment) {
        let Some(scope) = self.scope else { return };
        let Some(name) = local.names().iter().next() else {
            return;
        };
        if local.names().iter().count() != 1 || local.expressions().iter().count() != 1 {
            return;
        }
        let Some(Expression::FunctionCall(call)) = local.expressions().iter().next() else {
            return;
        };
        if !field_chain_before_first_call(call).is_some_and(|parts| parts == ["ShaderScan", "new"])
            || !asset::loader_binding_is_builtin(scope, call)
        {
            return;
        }
        if let Some(id) = node_span(name)
            .and_then(|span| scope.declaration_at_span(span))
            .map(|declaration| declaration.id)
        {
            self.scanners.insert(id);
        }
    }

    fn visit_function_call(&mut self, call: &FunctionCall) {
        let Some(scope) = self.scope else { return };
        let Prefix::Name(receiver) = call.prefix() else {
            return;
        };
        let mut suffixes = call.suffixes();
        let Some(Suffix::Call(Call::MethodCall(method))) = suffixes.next() else {
            return;
        };
        if suffixes.next().is_some() || token_text(method.name()) != "load_shader" {
            return;
        }
        let Some(receiver_id) =
            node_span(receiver).and_then(|span| scope.symbol_at_position(span.start))
        else {
            return;
        };
        let FunctionArgs::Parentheses { arguments, .. } = method.args() else {
            return;
        };
        let Some(Expression::String(token)) = arguments.iter().nth(1) else {
            return;
        };
        let Some(path) = asset::string_literal_value(token) else {
            return;
        };
        let Some(span) = node_span(token) else { return };
        self.loads.push((receiver_id, path, span));
    }
}

impl AnalysisSnapshot {
    pub(super) fn shader_scan_runtime_path_diagnostics(
        &self,
        path: &Path,
        file: &AnalysisFile,
    ) -> Vec<Diagnostic> {
        let Ok(parsed) = &file.parsed else {
            return Vec::new();
        };
        let Some(scope) = file.scope() else {
            return Vec::new();
        };
        let mut syntax = ShaderScanPaths {
            scope: Some(scope),
            ..ShaderScanPaths::default()
        };
        syntax.visit_ast(&parsed.ast);
        let ShaderScanPaths {
            scanners,
            loads,
            writes,
            ..
        } = syntax;
        loads
            .into_iter()
            .filter(|(receiver, _, span)| {
                scanners.contains(receiver)
                    && !writes.iter().any(|(id, position)| {
                        id == receiver
                            && (position.line, position.column)
                                < (span.start.line, span.start.column)
                    })
            })
            .filter_map(|(_, literal, span)| {
                let suggestion = build_relative_shader_path(&literal, self.source_roots())?;
                Some(
                    Diagnostic::new(
                        path,
                        span.start.line,
                        span.start.column,
                        DiagnosticSeverity::Error,
                        format!(
                            "ShaderScan path `{literal}` exists under the source root but will be `{suggestion}` in the built game; use `{suggestion}` in load_shader"
                        ),
                    )
                    .with_range(
                        span.start.line,
                        span.start.column,
                        span.end.line,
                        span.end.column,
                    )
                    .with_code(crate::DiagnosticCode::ASSET_SHADER_RUNTIME_PATH),
                )
            })
            .collect()
    }
}

fn build_relative_shader_path(literal: &str, source_roots: &[PathBuf]) -> Option<String> {
    if Path::new(literal).is_absolute() {
        return None;
    }
    let path = literal.replace('\\', "/");
    source_roots.iter().find_map(|root| {
        let source_name = root.file_name()?.to_str()?;
        let relative = path.strip_prefix(source_name)?.strip_prefix('/')?;
        if relative.is_empty() || root.join(&path).is_file() || !root.join(relative).is_file() {
            return None;
        }
        Some(relative.to_owned())
    })
}
