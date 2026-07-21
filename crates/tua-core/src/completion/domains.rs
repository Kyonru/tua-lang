use crate::semantic_domains::BuiltinDomainRegistry;
use crate::types::{Type, TypeEnvironment, TypedFunction};
use crate::{input, sprite, state_machine, ParsedSource, SourcePosition};

use super::{completion_text_matches, Completion, CompletionKind};

pub(super) struct CompletionDomainRegistry;

impl CompletionDomainRegistry {
    pub(super) fn config_items(
        parsed: &ParsedSource,
        source: &str,
        prefix: &str,
        position: SourcePosition,
    ) -> Option<Vec<Completion>> {
        if !super::table_key_completion_position(source, prefix, position) {
            return None;
        }

        if let Some(items) = state_machine::config_completion_items(parsed, position) {
            return Some(
                items
                    .into_iter()
                    .filter(|item| completion_text_matches(&item.label, prefix))
                    .map(|item| {
                        config_completion(
                            item.label,
                            item.detail,
                            item.documentation,
                            item.insert_text,
                        )
                    })
                    .collect(),
            );
        }

        if let Some(items) = input::config_completion_items(parsed, position) {
            return Some(
                items
                    .into_iter()
                    .filter(|item| completion_text_matches(&item.label, prefix))
                    .map(|item| {
                        config_completion(
                            item.label,
                            item.detail,
                            item.documentation,
                            item.insert_text,
                        )
                    })
                    .collect(),
            );
        }

        let context = sprite::config_context_at(parsed, position)?;

        Some(
            sprite::config_completion_items(context)
                .iter()
                .filter(|item| completion_text_matches(item.label, prefix))
                .map(|item| {
                    config_completion(
                        item.label,
                        item.detail,
                        item.documentation,
                        item.insert_text,
                    )
                })
                .collect(),
        )
    }

    pub(super) fn direct_string_items(
        parsed: &ParsedSource,
        source: &str,
        prefix: &str,
        position: SourcePosition,
    ) -> Option<Vec<Completion>> {
        if let Some(items) = input::string_value_completion_items(parsed, position) {
            return Some(
                items
                    .into_iter()
                    .filter(|item| completion_text_matches(&item.label, prefix))
                    .map(|item| Completion {
                        label: item.label,
                        kind: CompletionKind::Value,
                        detail: Some(item.detail),
                        documentation: Some(item.documentation),
                        insert_text: Some(item.insert_text),
                        filter_text: None,
                        is_snippet: false,
                        module_path: None,
                        source_module: None,
                        additional_text_edits: Vec::new(),
                    })
                    .collect(),
            );
        }

        if let Some(items) = state_machine::string_value_completion_items(parsed, position) {
            return Some(
                items
                    .into_iter()
                    .filter(|item| completion_text_matches(&item.label, prefix))
                    .map(|item| value_completion(item.label, item.detail, item.documentation, None))
                    .collect(),
            );
        }

        if sprite::config_context_at(parsed, position)
            == Some(sprite::SpriteConfigContext::Animation)
            && super::sprite_on_loop_value_position(source, position)
        {
            return Some(
                sprite::on_loop_method_values()
                    .iter()
                    .filter(|value| completion_text_matches(value, prefix))
                    .map(|value| {
                        value_completion(
                            *value,
                            "anim8 onLoop method",
                            "Animation method invoked by anim8 whenever the animation loops.",
                            None,
                        )
                    })
                    .collect(),
            );
        }

        None
    }

    pub(super) fn method_string_items(
        environment: &TypeEnvironment,
        receiver_type: &Type,
        method: &str,
        active_parameter: usize,
        prefix: &str,
    ) -> Option<Vec<Completion>> {
        if let Some(items) = state_machine::method_string_completion_items(
            environment,
            receiver_type,
            method,
            active_parameter,
        ) {
            return Some(
                items
                    .into_iter()
                    .filter(|item| completion_text_matches(&item.label, prefix))
                    .map(|item| value_completion(item.label, item.detail, item.documentation, None))
                    .collect(),
            );
        }

        input::method_string_completion_items(environment, receiver_type, method, active_parameter)
            .map(|items| {
                items
                    .into_iter()
                    .filter(|item| completion_text_matches(&item.label, prefix))
                    .map(|item| {
                        value_completion(
                            item.label,
                            item.detail,
                            item.documentation,
                            Some(item.insert_text),
                        )
                    })
                    .collect()
            })
    }

    pub(super) fn typed_member(
        environment: &TypeEnvironment,
        receiver_type: &Type,
        member: &str,
    ) -> Option<TypedFunction> {
        BuiltinDomainRegistry::standard().typed_member(environment, receiver_type, member)
    }
}

fn config_completion(
    label: impl Into<String>,
    detail: impl Into<String>,
    documentation: impl Into<String>,
    insert_text: impl Into<String>,
) -> Completion {
    Completion {
        label: label.into(),
        kind: CompletionKind::Field,
        detail: Some(detail.into()),
        documentation: Some(documentation.into()),
        insert_text: Some(insert_text.into()),
        filter_text: None,
        is_snippet: true,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    }
}

fn value_completion(
    label: impl Into<String>,
    detail: impl Into<String>,
    documentation: impl Into<String>,
    insert_text: Option<String>,
) -> Completion {
    Completion {
        label: label.into(),
        kind: CompletionKind::Value,
        detail: Some(detail.into()),
        documentation: Some(documentation.into()),
        insert_text,
        filter_text: None,
        is_snippet: false,
        module_path: None,
        source_module: None,
        additional_text_edits: Vec::new(),
    }
}
