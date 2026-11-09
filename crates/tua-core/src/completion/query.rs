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
        let context = class_field_type_context(self.parsed, self.source, context);
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
            CompletionContext::TypeAnnotation { prefix, position } => {
                complete_type_names(self.parsed, self.environment, prefix.as_str(), position)
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

                    if !fields.is_empty()
                        || receiver_parts.is_empty()
                        || receiver_parts
                            .first()
                            .is_some_and(|root| self.environment.local(root).is_some())
                    {
                        fields
                    } else {
                        complete_lua_fields(self.source, &receiver_parts, prefix.as_str(), position)
                    }
                }
            }
            CompletionContext::StringArgument {
                is_method_call,
                callee_parts,
                active_parameter,
                prefix,
                position,
            } => {
                let string_context = StringArgumentCompletionContext {
                    is_method_call,
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

/// Class fields are parsed outside the Lua AST's ordinary annotation context.
/// Recognize their type position here so generic class binders and ordinary
/// type names use the same completion path as other annotations.
fn class_field_type_context(
    parsed: &ParsedSource,
    source: &str,
    context: CompletionContext,
) -> CompletionContext {
    let CompletionContext::Identifier { prefix, position } = &context else {
        return context;
    };
    let Some(line) = crate::source_text::line_at(source, position.line) else {
        return context;
    };
    let cursor = crate::source_text::byte_for_column(line, position.column).min(line.len());

    for field in parsed
        .class_declarations
        .iter()
        .flat_map(|class| &class.fields)
    {
        if field.name_span.start.line != position.line
            || field.name_span.end.column > position.column
            || !crate::analysis::span_contains_position(field.span, *position)
        {
            continue;
        }
        let name_end =
            crate::source_text::byte_for_column(line, field.name_span.end.column).min(line.len());
        let Some(after_name) = line.get(name_end..cursor) else {
            continue;
        };
        let Some(colon) = after_name.find(':') else {
            continue;
        };
        if after_name[..colon]
            .chars()
            .any(|character| !character.is_whitespace())
        {
            continue;
        }
        let type_text = &after_name[colon + 1..];
        let typed_prefix = type_text.trim_start();
        if typed_prefix
            .chars()
            .any(|character| !crate::source_text::is_identifier_continue_char(character))
        {
            continue;
        }
        return CompletionContext::TypeAnnotation {
            prefix: prefix.clone(),
            position: *position,
        };
    }
    context
}
