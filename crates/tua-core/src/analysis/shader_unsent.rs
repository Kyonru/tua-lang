//! Optional, deliberately conservative project-wide shader usage lint.
use super::*;
use full_moon::ast::{Call, FunctionArgs, Prefix, Suffix, Var};

#[derive(Default)]
struct Syntax {
    locals: Vec<ast::LocalAssignment>,
    calls: Vec<FunctionCall>,
}
impl Visitor for Syntax {
    fn visit_local_assignment(&mut self, local: &ast::LocalAssignment) {
        self.locals.push(local.clone());
    }
    fn visit_function_call(&mut self, call: &FunctionCall) {
        self.calls.push(call.clone());
    }
}
#[derive(Default)]
struct Usage {
    allowed: BTreeSet<(usize, usize)>,
    sent: BTreeSet<String>,
    escaped: bool,
}
fn key(node: &impl Node) -> (usize, usize) {
    node.start_position()
        .map(|p| (p.line(), p.character()))
        .unwrap_or_default()
}
impl AnalysisSnapshot {
    pub(super) fn unsent_shader_diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        if !self.config.shaders.warn_unsent {
            return Vec::new();
        }
        self.shader_lint_cache
            .get_or_init(|| self.collect_unsent_shader_diagnostics())
            .get(&normalize_path(path))
            .cloned()
            .unwrap_or_default()
    }

    fn collect_unsent_shader_diagnostics(&self) -> BTreeMap<PathBuf, Vec<Diagnostic>> {
        let Some(index) = self.asset_index.as_ref() else {
            return BTreeMap::new();
        };
        let inventory = self.asset_inventory();
        let mut sent = BTreeMap::<PathBuf, BTreeSet<String>>::new();
        let mut blocked = BTreeSet::new();
        for (path, file) in &self.files {
            if self.is_generated_path(path) {
                continue;
            }
            let Ok(parsed) = &file.parsed else {
                return BTreeMap::new();
            };
            let Some(scope) = file.scope() else {
                return BTreeMap::new();
            };
            if scope
                .declarations()
                .iter()
                .any(|d| matches!(d.name.as_str(), "shader" | "love"))
            {
                return BTreeMap::new();
            }
            let mut syntax = Syntax::default();
            syntax.visit_ast(&parsed.ast);
            let mut candidates = BTreeMap::<String, (PathBuf, Usage)>::new();
            let mut recognized = BTreeSet::new();
            for local in &syntax.locals {
                if local.names().iter().count() != 1 || local.expressions().iter().count() != 1 {
                    continue;
                }
                let Some(Expression::FunctionCall(call)) = local.expressions().iter().next() else {
                    continue;
                };
                let Some(loader) =
                    asset::asset_loader_call(call).filter(|c| c.kind == AssetLoaderKind::Shader)
                else {
                    continue;
                };
                let name = local.names().iter().next().unwrap();
                let text = token_text(name);
                if scope
                    .declarations()
                    .iter()
                    .filter(|d| d.name == text)
                    .count()
                    != 1
                {
                    continue;
                }
                let metadata = asset::validate_asset_with_index(
                    loader.kind,
                    &loader.path,
                    loader.path_span,
                    self.effective_asset_roots(),
                    Some(index),
                );
                let Some(asset) = asset::resolved_asset_path(
                    &metadata,
                    self.effective_asset_roots(),
                    Some(index),
                ) else {
                    continue;
                };
                recognized.insert(key(call));
                candidates.insert(
                    text,
                    (
                        asset,
                        Usage {
                            allowed: BTreeSet::from([key(name)]),
                            ..Default::default()
                        },
                    ),
                );
            }
            for call in &syntax.calls {
                if let Some(parts) = field_chain_before_first_call(call) {
                    if parts == ["love", "graphics", "getShader"] {
                        return BTreeMap::new();
                    }
                    if parts == ["shader"] || parts == ["love", "graphics", "newShader"] {
                        if asset::inline_shader_call(call).is_some() {
                            continue;
                        }
                        if call.suffixes().any(|suffix| matches!(suffix, Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })) if arguments.iter().count() != 1)) {
                    continue;
                }
                        let Some(loader) = asset::asset_loader_call(call) else {
                            // A dynamic loader could initialize any asset in this project.
                            return BTreeMap::new();
                        };
                        if !recognized.contains(&key(call)) {
                            let metadata = asset::validate_asset_with_index(
                                loader.kind,
                                &loader.path,
                                loader.path_span,
                                self.effective_asset_roots(),
                                Some(index),
                            );
                            if let Some(asset) = asset::resolved_asset_path(
                                &metadata,
                                self.effective_asset_roots(),
                                Some(index),
                            ) {
                                blocked.insert(asset);
                            }
                        }
                    }
                }
                if let Some(send) = asset::shader_send_call(call) {
                    if let Some((asset, usage)) = candidates.get_mut(&send.receiver) {
                        usage.allowed.insert(key(call.prefix()));
                        if let Some(model) = index.shader(asset).and_then(|s| s.uniforms.as_ref()) {
                            for name in &model.names {
                                let matches = match &send.uniform {
                                    asset::ShaderUniformReference::Exact(value) => name == value,
                                    asset::ShaderUniformReference::Pattern {
                                        prefix,
                                        suffix,
                                        ..
                                    } => name.starts_with(prefix) && name.ends_with(suffix),
                                };
                                if matches {
                                    usage.sent.insert(name.clone());
                                }
                            }
                        }
                    }
                } else if let Prefix::Name(receiver) = call.prefix() {
                    if let Some((_, usage)) = candidates.get_mut(&token_text(receiver)) {
                        // Only hasUniform is safe besides recognized sends. Unknown
                        // sends, methods and chained calls conservatively escape.
                        let suffixes: Vec<_> = call.suffixes().collect();
                        if matches!(suffixes.as_slice(), [Suffix::Call(Call::MethodCall(method))] if token_text(method.name()) == "hasUniform")
                        {
                            usage.allowed.insert(key(receiver));
                        }
                    }
                }
                if field_chain_before_first_call(call)
                    .is_some_and(|parts| parts == ["love", "graphics", "setShader"])
                {
                    if let Some(Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses {
                        arguments,
                        ..
                    }))) = call.suffixes().last()
                    {
                        if arguments.iter().count() == 1 {
                            if let Some(Expression::Var(Var::Name(name))) = arguments.iter().next()
                            {
                                if let Some((_, usage)) = candidates.get_mut(&token_text(name)) {
                                    usage.allowed.insert(key(name));
                                }
                            }
                        }
                    }
                }
            }
            for token in parsed.ast.nodes().tokens() {
                if let Some((_, usage)) = candidates.get_mut(&token_text(token)) {
                    if !usage.allowed.contains(&key(token)) {
                        usage.escaped = true;
                    }
                }
            }
            for (_, (asset, usage)) in candidates {
                if usage.escaped {
                    blocked.insert(asset.clone());
                }
                sent.entry(asset).or_default().extend(usage.sent);
            }
        }
        let mut diagnostics = BTreeMap::<PathBuf, Vec<Diagnostic>>::new();
        for asset in inventory.assets {
            let Some(sent) = sent
                .get(&asset.path)
                .filter(|_| !blocked.contains(&asset.path))
            else {
                continue;
            };
            let Some(source) = index.shader(&asset.path) else {
                continue;
            };
            if source
                .source
                .lines()
                .any(|line| line.trim_start().starts_with('#'))
            {
                continue;
            }
            let Some(model) = &source.uniforms else {
                continue;
            };
            let missing = model
                .uniforms
                .values()
                .filter(|u| u.read && !u.initialized && !sent.contains(&u.name))
                .map(|u| format!("`{}`", u.name))
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                for reference in &asset.references {
                    diagnostics.entry(normalize_path(&reference.path)).or_default().push(
                        Diagnostic::new(&reference.path, reference.span.start.line, reference.span.start.column, crate::DiagnosticSeverity::Warning,
                            format!("shader `{}` reads uniforms {} but no statically traced project code sends them; their initial values are used", asset.relative_path, missing.join(", ")))
                        .with_code(crate::DiagnosticCode::ASSET_SHADER_UNSENT_UNIFORM));
                }
            }
        }
        diagnostics
    }
}
