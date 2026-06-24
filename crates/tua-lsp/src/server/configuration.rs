use std::path::{Path, PathBuf};

use serde_json::Value;
use tua_core::{module_name_for_path, DiagnosticsConfig, ModulePathSeparator};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct WorkspaceSettings {
    pub(super) diagnostics: DiagnosticOverrides,
    pub(super) file_associations: Vec<FileAssociation>,
    pub(super) ignored_paths: Vec<String>,
    pub(super) require_path_separator: RequirePathSeparator,
    pub(super) preview: PreviewSettings,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DiagnosticOverrides {
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
    pub(super) globals: Option<Vec<String>>,
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

    pub(super) fn source_kind_for_path(
        &self,
        path: &Path,
        project_extension: &str,
        roots: &[PathBuf],
    ) -> Option<SourceKind> {
        let extension = path.extension().and_then(|extension| extension.to_str());

        if extension.is_some_and(|extension| extension == project_extension) {
            return Some(SourceKind::Tua);
        }

        if extension == Some("lua") {
            return Some(SourceKind::Lua);
        }

        self.file_associations
            .iter()
            .find(|association| path_matches_any_root(path, &association.pattern, roots))
            .map(|association| match association.language {
                AssociatedLanguage::Tua => SourceKind::Tua,
                AssociatedLanguage::Lua => SourceKind::Lua,
            })
    }

    pub(super) fn is_ignored(&self, path: &Path, roots: &[PathBuf]) -> bool {
        self.ignored_paths
            .iter()
            .any(|pattern| path_matches_any_root(path, pattern, roots))
    }

    pub(super) fn module_name_for_path(&self, source_dir: &Path, path: &Path) -> Option<String> {
        module_name_for_path(
            source_dir,
            path,
            match self.require_path_separator {
                RequirePathSeparator::Dot => ModulePathSeparator::Dot,
                RequirePathSeparator::Slash => ModulePathSeparator::Slash,
            },
        )
    }
}

impl DiagnosticOverrides {
    fn from_settings(value: Option<&Value>) -> Self {
        let Some(settings) = value.and_then(Value::as_object) else {
            return Self::default();
        };

        Self {
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
            globals: string_array_value(settings.get("globals")),
        }
    }

    fn apply(&self, diagnostics: &mut DiagnosticsConfig) {
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

        if let Some(globals) = &self.globals {
            diagnostics.globals = globals.clone();
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
