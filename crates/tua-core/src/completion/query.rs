use super::*;

#[derive(Clone, Copy)]
pub(super) struct CompletionQuery<'a> {
    parsed: &'a ParsedSource,
    source: &'a str,
    environment: &'a TypeEnvironment,
    ranking: CompletionRankingInputs<'a>,
}

impl<'a> CompletionQuery<'a> {
    pub(super) fn new(
        parsed: &'a ParsedSource,
        source: &'a str,
        environment: &'a TypeEnvironment,
        ranking: CompletionRankingInputs<'a>,
    ) -> Self {
        Self {
            parsed,
            source,
            environment,
            ranking,
        }
    }

    pub(super) fn complete(self, context: CompletionContext) -> Vec<Completion> {
        let prefix = context.filter_prefix().map(str::to_owned);
        let ranking_context = CompletionRankingContext::from_context(
            &self.parsed.path,
            self.source,
            &context,
            self.ranking,
        );
        let completions = match context {
            CompletionContext::AnnotationTag { prefix } => {
                complete_annotation_tags(prefix.as_str())
            }
            CompletionContext::DocumentationSnippet { position } => {
                complete_documentation_snippet(self.parsed, position)
            }
            CompletionContext::TypeAnnotation { prefix } => {
                complete_type_names(self.environment, prefix.as_str())
            }
            CompletionContext::FieldAccess {
                receiver_parts,
                prefix,
                position,
                separator,
                separator_span,
            } => {
                let module_fields =
                    complete_required_module_fields(self.parsed, &receiver_parts, prefix.as_str());

                if !module_fields.is_empty() {
                    module_fields
                } else {
                    let field_context = FieldCompletionContext {
                        receiver_parts: &receiver_parts,
                        prefix: prefix.as_str(),
                        position,
                        separator,
                        separator_span,
                    };
                    let fields = self.complete_fields(field_context);

                    if !fields.is_empty() || self.environment.local(&receiver_parts[0]).is_some() {
                        fields
                    } else {
                        complete_lua_fields(self.source, &receiver_parts, prefix.as_str(), position)
                    }
                }
            }
            CompletionContext::StringArgument {
                callee_parts,
                active_parameter,
                prefix,
                position,
            } => {
                let string_context = StringArgumentCompletionContext {
                    callee_parts: &callee_parts,
                    active_parameter,
                    prefix: prefix.as_str(),
                    position,
                    project_data: self.ranking.project_data,
                };

                self.complete_string_argument_values(string_context)
            }
            CompletionContext::RequirePath { .. } | CompletionContext::ImportPath { .. } => {
                Vec::new()
            }
            CompletionContext::Identifier { prefix, position } => {
                CompletionDomainRegistry::config_items(
                    self.parsed,
                    self.source,
                    prefix.as_str(),
                    position,
                )
                .unwrap_or_else(|| {
                    complete_identifier_items(
                        self.parsed,
                        self.environment,
                        prefix.as_str(),
                        position,
                    )
                })
            }
        };

        rank_completions_for_context(completions, prefix.as_deref(), ranking_context.as_ref())
    }

    fn complete_fields(self, context: FieldCompletionContext<'_>) -> Vec<Completion> {
        complete_fields(self.parsed, self.source, self.environment, context)
    }

    fn complete_string_argument_values(
        self,
        context: StringArgumentCompletionContext<'_>,
    ) -> Vec<Completion> {
        complete_string_argument_values(self.parsed, self.source, self.environment, context)
    }
}
