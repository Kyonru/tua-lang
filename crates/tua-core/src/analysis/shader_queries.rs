use super::*;
use crate::completion::context::{completion_context, CompletionContext};
use full_moon::visitors::Visitor;

impl AnalysisSnapshot {
    fn shader_model_for_receiver(
        &self,
        parsed: &ParsedSource,
        receiver: &str,
        position: SourcePosition,
    ) -> Option<(PathBuf, Arc<asset::ShaderUniformSet>)> {
        let fallback_scope;
        let scope = match self.file(&parsed.path).and_then(AnalysisFile::scope) {
            Some(scope) => scope,
            None => {
                fallback_scope = scope_map_parsed_source(parsed);
                &fallback_scope
            }
        };
        let loader = crate::semantic::shader_loader_for_local(parsed, scope, receiver, position)?;
        if let Some(inline) = self.file(&parsed.path).and_then(|file| {
            file.inline_shaders()
                .iter()
                .find(|shader| Some(shader.call_span) == node_span(loader))
        }) {
            return Some((parsed.path.clone(), inline.source.uniforms.clone()?));
        }
        let call = asset::asset_loader_call(loader)?;
        if call.kind != AssetLoaderKind::Shader {
            return None;
        }
        let metadata = asset::validate_asset_with_index(
            call.kind,
            &call.path,
            call.path_span,
            self.effective_asset_roots(),
            self.asset_index.as_ref(),
        );
        let path = asset::resolved_asset_path(
            &metadata,
            self.effective_asset_roots(),
            self.asset_index.as_ref(),
        )?;
        let model = self.asset_index.as_ref()?.shader(&path)?.uniforms.clone()?;
        Some((path, model))
    }

    pub(super) fn shader_uniform_completions(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
    ) -> Option<Vec<Completion>> {
        let CompletionContext::StringArgument {
            callee_parts,
            active_parameter: 0,
            prefix,
            ..
        } = completion_context(source, position)?
        else {
            return None;
        };
        let [receiver, method] = callee_parts.as_slice() else {
            return None;
        };
        if !matches!(method.as_str(), "send" | "sendColor")
            || !crate::completion::context::string_argument_is_method(source, position)
        {
            return None;
        }
        let (_, model) = self.shader_model_for_receiver(parsed, receiver, position)?;
        let mut uniforms: Vec<_> = model
            .uniforms
            .values()
            .filter(|uniform| uniform.name.starts_with(&prefix))
            .collect();
        uniforms.sort_by_key(|uniform| {
            (
                !uniform.read,
                method == "sendColor" && !matches!(uniform.glsl_type.as_str(), "vec3" | "vec4"),
                &uniform.name,
            )
        });
        let call = send_at(parsed, position);
        Some(
            uniforms
                .into_iter()
                .map(|uniform| Completion {
                    label: uniform.name.clone(),
                    kind: CompletionKind::Value,
                    detail: Some(format!(
                        "{}{}",
                        uniform.detail(),
                        if uniform.read { "" } else { " (unread)" }
                    )),
                    documentation: Some(format!(
                        "{}\nExpected Lua value: {}.{}",
                        uniform.declaration,
                        uniform.lua_value(),
                        if !uniform.array_suffix.is_empty() {
                            " Array elements are sent as separate values."
                        } else {
                            ""
                        }
                    )),
                    deprecation: None,
                    insert_text: Some(completion_literal(&uniform.name, &prefix, call.as_ref())),
                    filter_text: None,
                    is_snippet: false,
                    module_path: None,
                    source_module: None,
                    additional_text_edits: Vec::new(),
                })
                .collect(),
        )
    }

    fn shader_uniform_at(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<(PathBuf, Arc<asset::ShaderUniformSet>, asset::ShaderSendCall)> {
        let file = self.file(path)?;
        // Do no shader traversal for ordinary identifier hovers/definitions.
        if !matches!(
            completion_context(&file.source, position),
            Some(CompletionContext::StringArgument { .. })
        ) {
            return None;
        }
        let parsed = file.parsed.as_ref().ok()?;
        let call = send_at(parsed, position)?;
        let (shader, model) = self.shader_model_for_receiver(parsed, &call.receiver, position)?;
        Some((shader, model, call))
    }

    pub(super) fn shader_uniform_hover(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Hover> {
        let (shader, model, call) = self.shader_uniform_at(path, position)?;
        let uniforms = matching_uniforms(&model, &call.uniform);
        if uniforms.is_empty() {
            return None;
        }
        let contents = uniforms
            .iter()
            .map(|uniform| {
                let locations = uniform
                    .declarations
                    .iter()
                    .map(|span| format!("{}:{}", shader.display(), span.start.line))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "```glsl\n{}\n```\n{}\n\nExpected Lua value: {}.{}{}",
                    uniform.declaration,
                    locations,
                    uniform.lua_value(),
                    if !uniform.array_suffix.is_empty() {
                        " Array elements are sent as separate values."
                    } else {
                        ""
                    },
                    if uniform.read {
                        ""
                    } else {
                        "\n\nUnread uniform: GLSL may remove it, so LOVE can reject sends to it."
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");
        Some(Hover {
            contents,
            span: Some(call.span),
            documentation: None,
            context: Vec::new(),
        })
    }

    pub(super) fn shader_uniform_definitions(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Vec<Definition> {
        let Some((shader, model, call)) = self.shader_uniform_at(path, position) else {
            return Vec::new();
        };
        matching_uniforms(&model, &call.uniform)
            .into_iter()
            .flat_map(|uniform| {
                uniform.declarations.iter().map(|span| Definition {
                    path: shader.clone(),
                    span: *span,
                    kind: DefinitionKind::Asset,
                })
            })
            .collect()
    }
}

fn matching_uniforms<'a>(
    model: &'a asset::ShaderUniformSet,
    reference: &asset::ShaderUniformReference,
) -> Vec<&'a asset::ShaderUniform> {
    model
        .uniforms
        .values()
        .filter(|uniform| match reference {
            asset::ShaderUniformReference::Exact(name) => uniform.name == *name,
            asset::ShaderUniformReference::Pattern { prefix, suffix, .. } => {
                uniform.name.starts_with(prefix)
                    && uniform.name.ends_with(suffix)
                    && uniform.name.len() > prefix.len() + suffix.len()
            }
        })
        .collect()
}

fn send_at(parsed: &ParsedSource, position: SourcePosition) -> Option<asset::ShaderSendCall> {
    struct Finder {
        position: SourcePosition,
        call: Option<asset::ShaderSendCall>,
    }
    impl Visitor for Finder {
        fn visit_function_call(&mut self, function: &full_moon::ast::FunctionCall) {
            if let Some(call) = asset::shader_send_call(function) {
                if contains_position(call.span, self.position) {
                    self.call = Some(call);
                }
            }
        }
    }
    let mut finder = Finder {
        position,
        call: None,
    };
    finder.visit_ast(&parsed.ast);
    finder.call
}

// Completing a concatenated numeric-name prefix must not duplicate the index
// that the expression already appends ("ripple1" .. i).
fn completion_literal(name: &str, typed: &str, call: Option<&asset::ShaderSendCall>) -> String {
    if let Some(asset::ShaderSendCall {
        uniform: asset::ShaderUniformReference::Pattern { suffix, .. },
        ..
    }) = call
    {
        if let Some(stem) = name.strip_suffix(suffix) {
            let prefix = stem.trim_end_matches(|character: char| character.is_ascii_digit());
            if prefix.starts_with(typed) && prefix.len() < stem.len() {
                return prefix.to_owned();
            }
        }
        return typed.to_owned();
    }
    name.to_owned()
}

impl AnalysisSnapshot {
    /// Literal shader sources use the cached parsed document, including unsaved edits.
    pub fn inline_shader_validation_requests(
        &self,
        config_for_path: impl Fn(&Path) -> crate::config::ShadersConfig,
    ) -> Result<Vec<crate::shader_validation::ShaderValidationRequest>, String> {
        use crate::shader_validation::{check_request_limits, ShaderValidationRequest};
        let mut requests = Vec::new();
        for (path, file) in &self.files {
            if self.is_generated_path(path) {
                continue;
            }
            let config = config_for_path(path);
            if config.validate == crate::config::ShaderValidationMode::Off {
                continue;
            }
            let targets: BTreeSet<_> = config.targets.iter().copied().collect();
            for shader in file.inline_shaders() {
                if shader.source.missing_entry_point() {
                    continue;
                }
                for target in &targets {
                    requests.push(ShaderValidationRequest {
                        source_origin: Some(shader.origin),
                        mode: config.validate,
                        target: *target,
                        source: shader.source.source.to_string(),
                        asset: ProjectAsset {
                            path: path.clone(),
                            relative_path: "<inline shader>".into(),
                            category: crate::AssetCategory::Shader,
                            extension: "glsl".into(),
                            dimensions: None,
                            references: Vec::new(),
                        },
                    });
                    check_request_limits(&requests)?;
                }
            }
        }
        Ok(requests)
    }
}
