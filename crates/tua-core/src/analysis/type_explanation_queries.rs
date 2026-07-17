use super::*;

impl AnalysisSnapshot {
    pub fn explain_type(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Option<TypeExplanation> {
        let _profile = profile::span("analysis.query.explain_type");
        let path = path.as_ref();
        let file = self.file(path)?;

        match &file.parsed {
            Ok(parsed) => {
                let environment = self.semantic_environment(parsed);

                self.explain_type_parsed(parsed, &environment, position)
            }
            Err(_) => {
                let facts = self.degraded_semantic_facts(path, file)?;

                self.explain_type_parsed(&facts.parsed, &facts.environment, position)
            }
        }
    }

    fn explain_type_parsed(
        &self,
        parsed: &ParsedSource,
        environment: &TypeEnvironment,
        position: SourcePosition,
    ) -> Option<TypeExplanation> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;
        let selected_parts = chain
            .parts
            .iter()
            .take(chain.cursor_index + 1)
            .collect::<Vec<_>>();
        let root = selected_parts.first()?;
        let selected = selected_parts.last()?;
        let parts = selected_parts
            .iter()
            .map(|part| part.text.clone())
            .collect::<Vec<_>>();
        let expression = parts.join(".");
        let span = SourceSpan {
            start: root.span.start,
            end: selected.span.end,
        };
        let semantic = SemanticModel::new(parsed, environment);
        let definition = self.definition(&parsed.path, root.span.start);
        let annotation =
            local_annotation_for_root(parsed, &root.text, position, definition.as_ref());
        let parameter =
            parameter_annotation_for_root(parsed, &root.text, position, definition.as_ref());
        let module_binding = require_bindings_for_parsed_source(parsed)
            .into_iter()
            .find(|binding| binding.alias == root.text);
        let module_type = module_binding
            .as_ref()
            .and_then(|_| self.indexed_module_binding_type(parsed, &root.text));
        let mut steps = Vec::new();

        if let Some(annotation) = annotation {
            let annotation_type = self
                .project_type_from_tua_type(parsed, &annotation.ty)
                .unwrap_or_else(|| crate::types::type_from_tua_type(&annotation.ty));
            let is_dynamic = matches!(annotation_type, Type::Any);

            steps.push(TypeExplanationStep {
                kind: if is_dynamic {
                    TypeExplanationStepKind::Dynamic
                } else {
                    TypeExplanationStepKind::Annotation
                },
                title: if is_dynamic {
                    "Explicit `any` annotation".to_owned()
                } else {
                    "Explicit local annotation".to_owned()
                },
                detail: if is_dynamic {
                    format!(
                        "`{}` is explicitly annotated as `any`, which is Tua's dynamic escape hatch.",
                        root.text
                    )
                } else {
                    format!("`{}` is declared with type `{annotation_type}`.", root.text)
                },
                ty: Some(annotation_type),
                path: Some(parsed.path.clone()),
                span: annotation.name_span.or(annotation.span),
            });
        } else if let Some(parameter) = parameter {
            let parameter_type = self
                .project_type_from_tua_type(parsed, &parameter.ty)
                .unwrap_or_else(|| crate::types::type_from_tua_type(&parameter.ty));

            steps.push(TypeExplanationStep {
                kind: if matches!(parameter_type, Type::Any) {
                    TypeExplanationStepKind::Dynamic
                } else {
                    TypeExplanationStepKind::Annotation
                },
                title: "Function parameter annotation".to_owned(),
                detail: format!(
                    "Parameter `{}` is declared with type `{parameter_type}`.",
                    root.text
                ),
                ty: Some(parameter_type),
                path: Some(parsed.path.clone()),
                span: parameter.name_span.or(parameter.span),
            });
        }

        if let Some(binding) = module_binding.as_ref() {
            self.push_module_explanation(parsed, binding, module_type.as_ref(), &mut steps);
        }

        let inferred = semantic.local_initializer_inference(&root.text, position);
        if annotation.is_none() && parameter.is_none() && module_binding.is_none() {
            if let Some(inferred) = inferred.as_ref() {
                let dynamic = matches!(inferred.ty, Type::Any);
                let inference_definition = inferred
                    .expression_span
                    .and_then(|span| self.definition(&parsed.path, span.start))
                    .or_else(|| definition.clone());

                steps.push(TypeExplanationStep {
                    kind: if dynamic {
                        TypeExplanationStepKind::Dynamic
                    } else {
                        TypeExplanationStepKind::Inference
                    },
                    title: if dynamic {
                        "Initializer is dynamic".to_owned()
                    } else {
                        "Inferred from initializer".to_owned()
                    },
                    detail: if dynamic {
                        format!(
                            "The initializer for `{}` has no statically known result type, so it contributes `any`.",
                            root.text
                        )
                    } else {
                        format!(
                            "The initializer for `{}` has type `{}`.",
                            root.text, inferred.ty
                        )
                    },
                    ty: Some(inferred.ty.clone()),
                    path: inference_definition
                        .as_ref()
                        .map(|definition| definition.path.clone()),
                    span: inference_definition
                        .as_ref()
                        .map(|definition| definition.span),
                });

                let widened = widen_inferred_type(&inferred.ty);
                if widened != inferred.ty {
                    steps.push(TypeExplanationStep {
                        kind: TypeExplanationStepKind::Widening,
                        title: "Widened inferred local".to_owned(),
                        detail: format!(
                            "Tua widens `{}` to the stable reassignment type `{widened}`.",
                            inferred.ty
                        ),
                        ty: Some(widened),
                        path: definition
                            .as_ref()
                            .map(|definition| definition.path.clone()),
                        span: definition.as_ref().map(|definition| definition.span),
                    });
                }
            }
        }

        let declared_type = || {
            environment
                .local(&root.text)
                .cloned()
                .or_else(|| parameter_type_before_position(parsed, &root.text, position))
                .or_else(|| semantic.type_of_name(&root.text, position))
        };
        let mut current = if annotation.is_some() || parameter.is_some() {
            declared_type()
        } else if module_binding.is_some() {
            module_type.or(Some(Type::Any))
        } else {
            declared_type()
        };

        if module_binding.is_none() {
            if let (Some(base), Some(narrowed)) = (
                current.as_ref(),
                semantic.type_of_name(&root.text, position),
            ) {
                if narrowed != *base {
                    steps.push(TypeExplanationStep {
                        kind: TypeExplanationStepKind::Narrowing,
                        title: "Narrowed by control flow".to_owned(),
                        detail: format!(
                            "The visible contract `{base}` is narrowed to `{narrowed}` at this position."
                        ),
                        ty: Some(narrowed.clone()),
                        path: Some(parsed.path.clone()),
                        span: Some(root.span),
                    });
                    current = Some(narrowed);
                }
            }
        }

        if current.is_none() {
            steps.push(TypeExplanationStep {
                kind: TypeExplanationStepKind::Unresolved,
                title: "Unresolved symbol".to_owned(),
                detail: format!(
                    "Tua cannot find a visible declaration or compiler type fact for `{}`.",
                    root.text
                ),
                ty: Some(Type::Any),
                path: Some(parsed.path.clone()),
                span: Some(root.span),
            });
            current = Some(Type::Any);
        } else if matches!(current, Some(Type::Any))
            && !steps
                .iter()
                .any(|step| step.kind == TypeExplanationStepKind::Dynamic)
        {
            steps.push(TypeExplanationStep {
                kind: TypeExplanationStepKind::Dynamic,
                title: "Dynamic compiler fact".to_owned(),
                detail: format!(
                    "The best available compiler fact for `{}` is `any`.",
                    root.text
                ),
                ty: Some(Type::Any),
                path: definition
                    .as_ref()
                    .map(|definition| definition.path.clone()),
                span: definition.as_ref().map(|definition| definition.span),
            });
        }

        let mut prefix = root.text.clone();
        for (index, field) in parts.iter().enumerate().skip(1) {
            let field_part = selected_parts[index];
            let receiver = current.clone().unwrap_or(Type::Any);
            prefix.push('.');
            prefix.push_str(field);

            if matches!(receiver, Type::Any) {
                steps.push(TypeExplanationStep {
                    kind: TypeExplanationStepKind::Dynamic,
                    title: format!("`{prefix}` remains dynamic"),
                    detail: format!(
                        "Member `{field}` cannot be checked because its receiver is `any`."
                    ),
                    ty: Some(Type::Any),
                    path: Some(parsed.path.clone()),
                    span: Some(field_part.span),
                });
                current = Some(Type::Any);
                continue;
            }

            if let Some(field_type) = self.field_type_for_type(&parsed.path, &receiver, field) {
                let field_definition = self.definition(&parsed.path, field_part.span.start);

                steps.push(TypeExplanationStep {
                    kind: TypeExplanationStepKind::Field,
                    title: format!("Resolved member `{field}`"),
                    detail: format!(
                        "`{prefix}` has type `{field_type}` from receiver `{receiver}`."
                    ),
                    ty: Some(field_type.clone()),
                    path: field_definition
                        .as_ref()
                        .map(|definition| definition.path.clone()),
                    span: field_definition.map(|definition| definition.span),
                });
                current = Some(field_type);
            } else {
                steps.push(TypeExplanationStep {
                    kind: TypeExplanationStepKind::Unresolved,
                    title: format!("Unknown member `{field}`"),
                    detail: format!(
                        "Tua cannot prove that `{receiver}` has member `{field}`, so `{prefix}` falls back to `any`."
                    ),
                    ty: Some(Type::Any),
                    path: Some(parsed.path.clone()),
                    span: Some(field_part.span),
                });
                current = Some(Type::Any);
            }
        }

        Some(TypeExplanation {
            expression,
            ty: current.unwrap_or(Type::Any),
            span,
            steps,
        })
    }

    fn indexed_module_binding_type(&self, parsed: &ParsedSource, receiver: &str) -> Option<Type> {
        let path = self.indexed_required_module_path(parsed, receiver)?;
        let exports = self.module_exports(path)?;

        Some(Type::table(exports.fields.iter().map(|(name, field)| {
            (name.clone(), TableField::required(field.ty.clone()))
        })))
    }

    fn push_module_explanation(
        &self,
        parsed: &ParsedSource,
        binding: &crate::RequireBinding,
        module_type: Option<&Type>,
        steps: &mut Vec<TypeExplanationStep>,
    ) {
        let Some(edge) = self.module_graph.module(&parsed.path).and_then(|module| {
            module
                .require_edges
                .iter()
                .find(|edge| edge.module == binding.module && edge.kind == binding.kind)
        }) else {
            return;
        };

        let (kind, title, detail) = match &edge.resolution {
            RequireResolution::Resolved { path } => (
                TypeExplanationStepKind::Module,
                "Resolved module".to_owned(),
                format!(
                    "{} `{}` resolves to `{}` and contributes its indexed exports.",
                    binding.kind.label(),
                    binding.module,
                    path.display()
                ),
            ),
            RequireResolution::Missing { search_roots } => (
                TypeExplanationStepKind::Unresolved,
                "Missing module".to_owned(),
                format!(
                    "{} `{}` was not found under: {}. Its binding remains dynamic.",
                    binding.kind.label(),
                    binding.module,
                    search_roots
                        .iter()
                        .map(|root| root.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ),
            RequireResolution::Invalid { reason } => (
                TypeExplanationStepKind::Unresolved,
                "Invalid module specifier".to_owned(),
                format!(
                    "{} `{}` is invalid: {reason}.",
                    binding.kind.label(),
                    binding.module
                ),
            ),
            RequireResolution::Duplicate {
                candidates,
                selected,
            } => (
                TypeExplanationStepKind::Module,
                "Ambiguous module".to_owned(),
                format!(
                    "{} `{}` matched {} files; Tua selected `{}`.",
                    binding.kind.label(),
                    binding.module,
                    candidates.len(),
                    selected.display()
                ),
            ),
        };

        steps.push(TypeExplanationStep {
            kind,
            title,
            detail,
            ty: module_type.cloned(),
            path: Some(parsed.path.clone()),
            span: Some(edge.span),
        });
    }
}

fn local_annotation_for_root<'a>(
    parsed: &'a ParsedSource,
    name: &str,
    position: SourcePosition,
    definition: Option<&Definition>,
) -> Option<&'a crate::LocalAnnotation> {
    parsed
        .local_annotations
        .iter()
        .filter(|annotation| annotation.name == name)
        .filter(|annotation| {
            annotation
                .name_span
                .or(annotation.span)
                .is_some_and(|span| position_key(span.start) <= position_key(position))
        })
        .filter(|annotation| {
            let Some(definition) = definition else {
                return true;
            };

            definition.path != parsed.path
                || annotation.name_span.or(annotation.span) == Some(definition.span)
        })
        .max_by_key(|annotation| {
            annotation
                .name_span
                .or(annotation.span)
                .map(|span| position_key(span.start))
        })
}

fn parameter_annotation_for_root<'a>(
    parsed: &'a ParsedSource,
    name: &str,
    position: SourcePosition,
    definition: Option<&Definition>,
) -> Option<&'a crate::ParameterAnnotation> {
    parsed
        .function_annotations
        .iter()
        .flat_map(|function| function.parameters.iter())
        .rfind(|parameter| {
            parameter.name == name
                && parameter
                    .name_span
                    .or(parameter.span)
                    .is_some_and(|span| position_key(span.start) <= position_key(position))
                && definition.is_none_or(|definition| {
                    definition.path != parsed.path
                        || parameter.name_span.or(parameter.span) == Some(definition.span)
                })
        })
}
