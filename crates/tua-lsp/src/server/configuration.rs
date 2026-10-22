use std::path::{Path, PathBuf};

use serde_json::Value;
use tua_core::{
    CompletionMultiTokenConfig, CompletionSequenceRankingConfig, DiagnosticsConfig, EcsConfig,
    FormatConfig, FormatOptions, JitConfig, ModulePathSeparator, ProjectConfig,
    ProjectFileAssociation, QuoteStyle, VfsSourceKind,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct WorkspaceSettings {
    pub(super) diagnostics: DiagnosticOverrides,
    pub(super) format: FormatOverrides,
    pub(super) jit: JitOverrides,
    pub(super) ecs: EcsOverrides,
    pub(super) completion: CompletionOverrides,
    pub(super) file_associations: Vec<FileAssociation>,
    pub(super) ignored_paths: Vec<String>,
    pub(super) require_path_separator: RequirePathSeparator,
    pub(super) preview: PreviewSettings,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DiagnosticOverrides {
    pub(super) rules: Option<bool>,
    pub(super) duplicate_keys: Option<bool>,
    pub(super) missing_requires: Option<bool>,
    pub(super) self_assignments: Option<bool>,
    pub(super) doc_annotations: Option<bool>,
    pub(super) undefined_variables: Option<bool>,
    pub(super) unused_locals: Option<bool>,
    pub(super) unused_parameters: Option<bool>,
    pub(super) use_before_definition: Option<bool>,
    pub(super) duplicate_parameters: Option<bool>,
    pub(super) wrong_argument_count: Option<bool>,
    pub(super) bad_requires: Option<bool>,
    pub(super) impossible_boolean_expressions: Option<bool>,
    pub(super) duplicate_if_conditions: Option<bool>,
    pub(super) duplicate_binary_operands: Option<bool>,
    pub(super) float_equality: Option<bool>,
    pub(super) const_assignments: Option<bool>,
    pub(super) parameter_type_mismatches: Option<bool>,
    pub(super) return_type_mismatches: Option<bool>,
    pub(super) assignment_arity: Option<bool>,
    pub(super) local_declaration_arity: Option<bool>,
    pub(super) unused_assignments: Option<bool>,
    pub(super) binary_operator_type_mismatches: Option<bool>,
    pub(super) uncalled_local_functions: Option<bool>,
    pub(super) inline_ignores: Option<bool>,
    pub(super) inline_ignore_scopes: Option<Vec<String>>,
    pub(super) inline_ignore_max_per_file: Option<usize>,
    pub(super) globals: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct FormatOverrides {
    pub(super) indent_width: Option<usize>,
    pub(super) line_width: Option<usize>,
    pub(super) quote_style: Option<QuoteStyle>,
    pub(super) final_newline: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct JitOverrides {
    pub(super) enabled: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct EcsOverrides {
    pub(super) enabled: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CompletionOverrides {
    pub(super) sequence_ranking: CompletionSequenceRankingOverrides,
    pub(super) multi_token: CompletionMultiTokenOverrides,
    pub(super) inline: InlineCompletionOverrides,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct InlineCompletionOverrides {
    pub(super) enabled: Option<bool>,
    pub(super) semantic: Option<bool>,
    pub(super) current_file: Option<bool>,
    /// Experimental Phase 2 gate-development switch. It is intentionally not
    /// exposed by the extension until project retrieval passes its gates.
    pub(super) project: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CompletionSequenceRankingOverrides {
    pub(super) enabled: Option<bool>,
    pub(super) timeout_ms: Option<u64>,
    pub(super) max_candidates: Option<usize>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CompletionMultiTokenOverrides {
    pub(super) enabled: Option<bool>,
    pub(super) advanced: Option<bool>,
    pub(super) allow_model_integration: Option<bool>,
    pub(super) max_suggestions: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileAssociation {
    pattern: String,
    language: AssociatedLanguage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AssociatedLanguage {
    Tua,
    Lua,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum RequirePathSeparator {
    #[default]
    Dot,
    Slash,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct PreviewSettings {
    pub(super) enabled: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceKind {
    Tua,
    Lua,
}

impl WorkspaceSettings {
    pub(super) fn from_did_change_configuration(message: &Value) -> Self {
        let Some(settings) = message
            .get("params")
            .and_then(|params| params.get("settings"))
        else {
            return Self::default();
        };
        let settings = settings.get("tua").unwrap_or(settings);

        Self {
            diagnostics: DiagnosticOverrides::from_settings(settings.get("diagnostics")),
            format: FormatOverrides::from_settings(settings.get("format")),
            jit: JitOverrides::from_settings(settings.get("jit")),
            ecs: EcsOverrides::from_settings(settings.get("ecs")),
            completion: CompletionOverrides::from_settings(settings.get("completion")),
            file_associations: file_associations_from_settings(settings),
            ignored_paths: string_array_setting(settings, &["ignoredPaths", "ignored_paths"]),
            require_path_separator: require_path_separator_from_settings(settings),
            preview: PreviewSettings::from_settings(settings.get("preview")),
        }
    }

    pub(super) fn diagnostics_config(&self, mut project: DiagnosticsConfig) -> DiagnosticsConfig {
        self.diagnostics.apply(&mut project);
        project
    }

    pub(super) fn format_options(&self, mut project: FormatConfig) -> FormatOptions {
        self.format.apply(&mut project);
        project.options()
    }

    pub(super) fn apply_project_config(&self, mut project: ProjectConfig) -> ProjectConfig {
        project.diagnostics = self.diagnostics_config(project.diagnostics);
        self.format.apply(&mut project.format);
        self.jit.apply(&mut project.jit);
        self.ecs.apply(&mut project.ecs);

        project
    }

    pub(super) fn is_ignored(&self, path: &Path, roots: &[PathBuf]) -> bool {
        self.ignored_paths
            .iter()
            .any(|pattern| path_matches_any_root(path, pattern, roots))
    }

    pub(super) fn project_file_associations(&self) -> Vec<ProjectFileAssociation> {
        self.file_associations
            .iter()
            .map(|association| ProjectFileAssociation {
                pattern: association.pattern.clone(),
                kind: match association.language {
                    AssociatedLanguage::Tua => VfsSourceKind::Tua,
                    AssociatedLanguage::Lua => VfsSourceKind::Lua,
                },
            })
            .collect()
    }

    pub(super) fn module_path_separator(&self) -> ModulePathSeparator {
        match self.require_path_separator {
            RequirePathSeparator::Dot => ModulePathSeparator::Dot,
            RequirePathSeparator::Slash => ModulePathSeparator::Slash,
        }
    }

    pub(super) fn completion_sequence_ranking_config(&self) -> CompletionSequenceRankingConfig {
        let mut config = CompletionSequenceRankingConfig::default();

        self.completion.sequence_ranking.apply(&mut config);
        config
    }

    pub(super) fn completion_multi_token_config(&self) -> CompletionMultiTokenConfig {
        let mut config = CompletionMultiTokenConfig::default();

        self.completion.multi_token.apply(&mut config);
        config
    }

    pub(super) fn inline_completion_enabled(&self) -> bool {
        self.completion.inline.enabled.unwrap_or(false)
    }

    pub(super) fn inline_semantic_enabled(&self) -> bool {
        self.completion.inline.semantic.unwrap_or(true)
    }

    pub(super) fn inline_current_file_enabled(&self) -> bool {
        self.completion.inline.current_file.unwrap_or(true)
    }

    pub(super) fn inline_project_enabled(&self) -> bool {
        self.inline_completion_enabled() && self.completion.inline.project.unwrap_or(false)
    }
}

impl DiagnosticOverrides {
    fn from_settings(value: Option<&Value>) -> Self {
        let Some(settings) = value.and_then(Value::as_object) else {
            return Self::default();
        };

        Self {
            rules: bool_setting(settings, "rules", "rules"),
            duplicate_keys: bool_setting(settings, "duplicate_keys", "duplicateKeys"),
            missing_requires: bool_setting(settings, "missing_requires", "missingRequires"),
            self_assignments: bool_setting(settings, "self_assignments", "selfAssignments"),
            doc_annotations: bool_setting(settings, "doc_annotations", "docAnnotations"),
            undefined_variables: bool_setting(
                settings,
                "undefined_variables",
                "undefinedVariables",
            ),
            unused_locals: bool_setting(settings, "unused_locals", "unusedLocals"),
            unused_parameters: bool_setting(settings, "unused_parameters", "unusedParameters"),
            use_before_definition: bool_setting(
                settings,
                "use_before_definition",
                "useBeforeDefinition",
            ),
            duplicate_parameters: bool_setting(
                settings,
                "duplicate_parameters",
                "duplicateParameters",
            ),
            wrong_argument_count: bool_setting(
                settings,
                "wrong_argument_count",
                "wrongArgumentCount",
            ),
            bad_requires: bool_setting(settings, "bad_requires", "badRequires"),
            impossible_boolean_expressions: bool_setting(
                settings,
                "impossible_boolean_expressions",
                "impossibleBooleanExpressions",
            ),
            duplicate_if_conditions: bool_setting(
                settings,
                "duplicate_if_conditions",
                "duplicateIfConditions",
            ),
            duplicate_binary_operands: bool_setting(
                settings,
                "duplicate_binary_operands",
                "duplicateBinaryOperands",
            ),
            float_equality: bool_setting(settings, "float_equality", "floatEquality"),
            const_assignments: bool_setting(settings, "const_assignments", "constAssignments"),
            parameter_type_mismatches: bool_setting(
                settings,
                "parameter_type_mismatches",
                "parameterTypeMismatches",
            ),
            return_type_mismatches: bool_setting(
                settings,
                "return_type_mismatches",
                "returnTypeMismatches",
            ),
            assignment_arity: bool_setting(settings, "assignment_arity", "assignmentArity"),
            local_declaration_arity: bool_setting(
                settings,
                "local_declaration_arity",
                "localDeclarationArity",
            ),
            unused_assignments: bool_setting(settings, "unused_assignments", "unusedAssignments"),
            binary_operator_type_mismatches: bool_setting(
                settings,
                "binary_operator_type_mismatches",
                "binaryOperatorTypeMismatches",
            ),
            uncalled_local_functions: bool_setting(
                settings,
                "uncalled_local_functions",
                "uncalledLocalFunctions",
            ),
            inline_ignores: bool_setting(settings, "inline_ignores", "inlineIgnores"),
            inline_ignore_scopes: string_array_value(
                settings
                    .get("inlineIgnoreScopes")
                    .or_else(|| settings.get("inline_ignore_scopes")),
            ),
            inline_ignore_max_per_file: usize_setting(
                settings,
                "inline_ignore_max_per_file",
                "inlineIgnoreMaxPerFile",
            ),
            globals: string_array_value(settings.get("globals")),
        }
    }

    fn apply(&self, diagnostics: &mut DiagnosticsConfig) {
        apply_bool(self.rules, &mut diagnostics.rules);
        apply_bool(self.duplicate_keys, &mut diagnostics.duplicate_keys);
        apply_bool(self.missing_requires, &mut diagnostics.missing_requires);
        apply_bool(self.self_assignments, &mut diagnostics.self_assignments);
        apply_bool(self.doc_annotations, &mut diagnostics.doc_annotations);
        apply_bool(
            self.undefined_variables,
            &mut diagnostics.undefined_variables,
        );
        apply_bool(self.unused_locals, &mut diagnostics.unused_locals);
        apply_bool(self.unused_parameters, &mut diagnostics.unused_parameters);
        apply_bool(
            self.use_before_definition,
            &mut diagnostics.use_before_definition,
        );
        apply_bool(
            self.duplicate_parameters,
            &mut diagnostics.duplicate_parameters,
        );
        apply_bool(
            self.wrong_argument_count,
            &mut diagnostics.wrong_argument_count,
        );
        apply_bool(self.bad_requires, &mut diagnostics.bad_requires);
        apply_bool(
            self.impossible_boolean_expressions,
            &mut diagnostics.impossible_boolean_expressions,
        );
        apply_bool(
            self.duplicate_if_conditions,
            &mut diagnostics.duplicate_if_conditions,
        );
        apply_bool(
            self.duplicate_binary_operands,
            &mut diagnostics.duplicate_binary_operands,
        );
        apply_bool(self.float_equality, &mut diagnostics.float_equality);
        apply_bool(self.const_assignments, &mut diagnostics.const_assignments);
        apply_bool(
            self.parameter_type_mismatches,
            &mut diagnostics.parameter_type_mismatches,
        );
        apply_bool(
            self.return_type_mismatches,
            &mut diagnostics.return_type_mismatches,
        );
        apply_bool(self.assignment_arity, &mut diagnostics.assignment_arity);
        apply_bool(
            self.local_declaration_arity,
            &mut diagnostics.local_declaration_arity,
        );
        apply_bool(self.unused_assignments, &mut diagnostics.unused_assignments);
        apply_bool(
            self.binary_operator_type_mismatches,
            &mut diagnostics.binary_operator_type_mismatches,
        );
        apply_bool(
            self.uncalled_local_functions,
            &mut diagnostics.uncalled_local_functions,
        );
        apply_bool(self.inline_ignores, &mut diagnostics.inline_ignores);

        if let Some(scopes) = &self.inline_ignore_scopes {
            diagnostics.inline_ignore_scopes = scopes.clone();
        }

        if let Some(max_per_file) = self.inline_ignore_max_per_file {
            diagnostics.inline_ignore_max_per_file = max_per_file;
        }

        if let Some(globals) = &self.globals {
            diagnostics.globals = globals.clone();
        }
    }
}

impl FormatOverrides {
    fn from_settings(value: Option<&Value>) -> Self {
        let Some(settings) = value.and_then(Value::as_object) else {
            return Self::default();
        };

        Self {
            indent_width: usize_setting(settings, "indent_width", "indentWidth"),
            line_width: usize_setting(settings, "line_width", "lineWidth"),
            quote_style: quote_style_setting(settings, "quote_style", "quoteStyle"),
            final_newline: bool_setting(settings, "final_newline", "finalNewline"),
        }
    }

    fn apply(&self, format: &mut FormatConfig) {
        if let Some(indent_width) = self.indent_width {
            format.indent_width = indent_width;
        }

        if let Some(line_width) = self.line_width {
            format.line_width = line_width;
        }

        if let Some(quote_style) = self.quote_style {
            format.quote_style = quote_style;
        }

        apply_bool(self.final_newline, &mut format.final_newline);
    }
}

impl JitOverrides {
    fn from_settings(value: Option<&Value>) -> Self {
        let Some(settings) = value.and_then(Value::as_object) else {
            return Self::default();
        };

        Self {
            enabled: bool_setting(settings, "enabled", "enabled"),
        }
    }

    fn apply(&self, jit: &mut JitConfig) {
        apply_bool(self.enabled, &mut jit.enabled);
    }
}

impl EcsOverrides {
    fn from_settings(value: Option<&Value>) -> Self {
        let Some(settings) = value.and_then(Value::as_object) else {
            return Self::default();
        };

        Self {
            enabled: bool_setting(settings, "enabled", "enabled"),
        }
    }

    fn apply(&self, ecs: &mut EcsConfig) {
        apply_bool(self.enabled, &mut ecs.enabled);
    }
}

impl CompletionOverrides {
    fn from_settings(value: Option<&Value>) -> Self {
        let Some(settings) = value.and_then(Value::as_object) else {
            return Self::default();
        };
        let sequence_ranking = settings
            .get("sequenceRanking")
            .or_else(|| settings.get("sequence_ranking"));
        let multi_token = settings
            .get("multiToken")
            .or_else(|| settings.get("multi_token"));
        let inline = settings.get("inline");

        Self {
            sequence_ranking: CompletionSequenceRankingOverrides::from_settings(sequence_ranking),
            multi_token: CompletionMultiTokenOverrides::from_settings(multi_token),
            inline: InlineCompletionOverrides::from_settings(inline),
        }
    }
}

impl InlineCompletionOverrides {
    fn from_settings(value: Option<&Value>) -> Self {
        let Some(settings) = value.and_then(Value::as_object) else {
            return Self::default();
        };
        let sources = settings.get("sources").and_then(Value::as_object);

        Self {
            enabled: bool_setting(settings, "enabled", "enabled"),
            semantic: sources.and_then(|sources| bool_setting(sources, "semantic", "semantic")),
            current_file: sources
                .and_then(|sources| bool_setting(sources, "current_file", "currentFile")),
            project: sources.and_then(|sources| bool_setting(sources, "project", "project")),
        }
    }
}

impl CompletionSequenceRankingOverrides {
    fn from_settings(value: Option<&Value>) -> Self {
        let Some(settings) = value.and_then(Value::as_object) else {
            return Self::default();
        };

        Self {
            enabled: bool_setting(settings, "enabled", "enabled"),
            timeout_ms: u64_setting(settings, "timeout_ms", "timeoutMs"),
            max_candidates: usize_setting(settings, "max_candidates", "maxCandidates"),
        }
    }

    fn apply(&self, config: &mut CompletionSequenceRankingConfig) {
        if let Some(enabled) = self.enabled {
            config.enabled = enabled;
        }

        if let Some(timeout_ms) = self.timeout_ms {
            config.timeout_ms = timeout_ms;
        }

        if let Some(max_candidates) = self.max_candidates {
            config.max_candidates = max_candidates;
        }
    }
}

impl CompletionMultiTokenOverrides {
    fn from_settings(value: Option<&Value>) -> Self {
        let Some(settings) = value.and_then(Value::as_object) else {
            return Self::default();
        };

        Self {
            enabled: bool_setting(settings, "enabled", "enabled"),
            advanced: bool_setting(settings, "advanced", "advanced"),
            allow_model_integration: bool_setting(
                settings,
                "allow_model_integration",
                "allowModelIntegration",
            ),
            max_suggestions: usize_setting(settings, "max_suggestions", "maxSuggestions"),
        }
    }

    fn apply(&self, config: &mut CompletionMultiTokenConfig) {
        if let Some(enabled) = self.enabled {
            config.enabled = enabled;
        }

        if let Some(advanced) = self.advanced {
            config.advanced = advanced;
        }

        if let Some(allow_model_integration) = self.allow_model_integration {
            config.allow_model_integration = allow_model_integration;
        }

        if let Some(max_suggestions) = self.max_suggestions {
            config.max_suggestions = max_suggestions;
        }
    }
}

impl PreviewSettings {
    fn from_settings(value: Option<&Value>) -> Self {
        let Some(settings) = value.and_then(Value::as_object) else {
            return Self::default();
        };

        Self {
            enabled: bool_setting(settings, "enabled", "enabled"),
        }
    }
}

fn file_associations_from_settings(settings: &Value) -> Vec<FileAssociation> {
    let Some(value) = settings
        .get("fileAssociations")
        .or_else(|| settings.get("file_associations"))
    else {
        return Vec::new();
    };

    if let Some(object) = value.as_object() {
        return object
            .iter()
            .filter_map(|(pattern, language)| {
                Some(FileAssociation {
                    pattern: pattern.clone(),
                    language: associated_language(language.as_str()?)?,
                })
            })
            .collect();
    }

    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let object = item.as_object()?;

            Some(FileAssociation {
                pattern: object.get("pattern")?.as_str()?.to_owned(),
                language: associated_language(object.get("language")?.as_str()?)?,
            })
        })
        .collect()
}

fn require_path_separator_from_settings(settings: &Value) -> RequirePathSeparator {
    let Some(value) = settings
        .get("requirePathSeparator")
        .or_else(|| settings.get("require_path_separator"))
        .and_then(Value::as_str)
    else {
        return RequirePathSeparator::Dot;
    };

    match value {
        "/" | "slash" => RequirePathSeparator::Slash,
        _ => RequirePathSeparator::Dot,
    }
}

fn associated_language(value: &str) -> Option<AssociatedLanguage> {
    match value {
        "tua" | "Tua" => Some(AssociatedLanguage::Tua),
        "lua" | "Lua" => Some(AssociatedLanguage::Lua),
        _ => None,
    }
}

fn bool_setting(
    settings: &serde_json::Map<String, Value>,
    snake_case: &str,
    camel_case: &str,
) -> Option<bool> {
    settings
        .get(camel_case)
        .or_else(|| settings.get(snake_case))
        .and_then(Value::as_bool)
}

fn usize_setting(
    settings: &serde_json::Map<String, Value>,
    snake_case: &str,
    camel_case: &str,
) -> Option<usize> {
    settings
        .get(camel_case)
        .or_else(|| settings.get(snake_case))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn u64_setting(
    settings: &serde_json::Map<String, Value>,
    snake_case: &str,
    camel_case: &str,
) -> Option<u64> {
    settings
        .get(camel_case)
        .or_else(|| settings.get(snake_case))
        .and_then(Value::as_u64)
}

fn quote_style_setting(
    settings: &serde_json::Map<String, Value>,
    snake_case: &str,
    camel_case: &str,
) -> Option<QuoteStyle> {
    match settings
        .get(camel_case)
        .or_else(|| settings.get(snake_case))
        .and_then(Value::as_str)
    {
        Some("preserve") => Some(QuoteStyle::Preserve),
        _ => None,
    }
}

fn string_array_setting(settings: &Value, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .find_map(|name| settings.get(*name))
        .and_then(|value| string_array_value(Some(value)))
        .unwrap_or_default()
}

fn string_array_value(value: Option<&Value>) -> Option<Vec<String>> {
    let values = value?.as_array()?;

    Some(
        values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
    )
}

fn apply_bool(source: Option<bool>, target: &mut bool) {
    if let Some(source) = source {
        *target = source;
    }
}

fn path_matches_any_root(path: &Path, pattern: &str, roots: &[PathBuf]) -> bool {
    if path_matches(path, pattern) {
        return true;
    }

    roots.iter().any(|root| {
        path.strip_prefix(root)
            .ok()
            .is_some_and(|relative| path_matches(relative, pattern))
    })
}

fn path_matches(path: &Path, pattern: &str) -> bool {
    let path = normalized_path(path);
    let pattern = pattern.replace('\\', "/");

    if let Some(extension) = pattern.strip_prefix("*.") {
        return path
            .rsplit('/')
            .next()
            .is_some_and(|name| name.ends_with(&format!(".{extension}")));
    }

    if let Some(suffix) = pattern.strip_prefix("**/*") {
        return path.ends_with(suffix);
    }

    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path == prefix || path.starts_with(&format!("{prefix}/"));
    }

    path == pattern || path.ends_with(&format!("/{pattern}"))
}

fn normalized_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::WorkspaceSettings;

    #[test]
    fn inline_completion_settings_are_disabled_by_default_and_parse_sources() {
        let defaults = WorkspaceSettings::default();
        assert!(!defaults.inline_completion_enabled());
        assert!(defaults.inline_semantic_enabled());
        assert!(defaults.inline_current_file_enabled());
        assert!(!defaults.inline_project_enabled());

        let settings = WorkspaceSettings::from_did_change_configuration(&json!({
            "params": {
                "settings": {
                    "tua": {
                        "completion": {
                            "inline": {
                                "enabled": true,
                                "sources": {
                                    "semantic": false,
                                    "currentFile": false,
                                    "project": true
                                }
                            }
                        }
                    }
                }
            }
        }));

        assert!(settings.inline_completion_enabled());
        assert!(!settings.inline_semantic_enabled());
        assert!(!settings.inline_current_file_enabled());
        assert!(settings.inline_project_enabled());
    }
}
