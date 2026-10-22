use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::OnceLock;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use toml_edit::{Document, InlineTable, Table, Value};

use super::{ConfigParseError, ProjectConfig};
use crate::formatter::QuoteStyle;

const CONFIG_SCHEMA_JSON: &str = include_str!("../../../../config/schema.json");

#[derive(Debug, Deserialize)]
struct ConfigSchema {
    settings: Vec<ConfigSetting>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSettingMetadata {
    pub path: String,
    pub section: Option<String>,
    pub key: String,
    pub type_name: String,
    pub default: serde_json::Value,
    pub description: String,
    pub enum_values: Vec<String>,
    pub minimum: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ConfigSetting {
    path: String,
    #[serde(rename = "type")]
    type_name: String,
    default: serde_json::Value,
    description: String,
    #[serde(default, rename = "enum")]
    enum_values: Vec<String>,
    #[serde(default)]
    minimum: Option<usize>,
}

pub fn parse_config(input: &str) -> Result<ProjectConfig, ConfigParseError> {
    let document = Document::parse(input)
        .map_err(|error| parse_error(input, error.span(), error.message()))?;

    if let Some(error) = first_unknown_setting(input, document.as_table(), "", config_schema()) {
        return Err(error);
    }

    toml::from_str(input).map_err(|error| parse_error(input, error.span(), error.message()))
}

pub fn config_settings_metadata() -> Vec<ConfigSettingMetadata> {
    config_schema()
        .settings
        .iter()
        .map(ConfigSetting::metadata)
        .collect()
}

pub fn config_setting_metadata(path: &str) -> Option<ConfigSettingMetadata> {
    config_schema()
        .settings
        .iter()
        .find(|setting| setting.path == path)
        .map(ConfigSetting::metadata)
}

pub fn config_section_names() -> Vec<String> {
    schema_child_names(config_schema(), "")
        .into_iter()
        .filter(|name| schema_has_children(config_schema(), name))
        .map(str::to_owned)
        .collect()
}

pub fn config_settings_in_section(section: Option<&str>) -> Vec<ConfigSettingMetadata> {
    config_schema()
        .settings
        .iter()
        .filter(|setting| setting.section().as_deref() == section)
        .map(ConfigSetting::metadata)
        .collect()
}

pub fn closest_config_child_name(prefix: &str, name: &str) -> Option<String> {
    closest_name(name, schema_child_names(config_schema(), prefix)).map(str::to_owned)
}

pub(super) fn deserialize_quote_style<'de, D>(deserializer: D) -> Result<QuoteStyle, D::Error>
where
    D: Deserializer<'de>,
{
    match String::deserialize(deserializer)?.as_str() {
        "preserve" => Ok(QuoteStyle::Preserve),
        "double" => Ok(QuoteStyle::Double),
        "single" => Ok(QuoteStyle::Single),
        _ => Err(D::Error::custom(
            "expected quote style \"preserve\", \"double\", or \"single\"",
        )),
    }
}

pub(super) fn deserialize_positive_usize<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let value = usize::deserialize(deserializer)?;
    if value == 0 {
        Err(D::Error::custom(
            "expected an integer greater than or equal to 1",
        ))
    } else {
        Ok(value)
    }
}

fn config_schema() -> &'static ConfigSchema {
    static SCHEMA: OnceLock<ConfigSchema> = OnceLock::new();

    SCHEMA.get_or_init(|| {
        serde_json::from_str(CONFIG_SCHEMA_JSON).expect("config/schema.json should be valid")
    })
}

impl ConfigSetting {
    fn metadata(&self) -> ConfigSettingMetadata {
        ConfigSettingMetadata {
            path: self.path.clone(),
            section: self.section(),
            key: self.key().to_owned(),
            type_name: self.type_name.clone(),
            default: self.default.clone(),
            description: self.description.clone(),
            enum_values: self.enum_values.clone(),
            minimum: self.minimum,
        }
    }

    fn section(&self) -> Option<String> {
        self.path
            .rsplit_once('.')
            .map(|(section, _)| section.to_owned())
    }

    fn key(&self) -> &str {
        self.path
            .rsplit_once('.')
            .map_or(self.path.as_str(), |(_, key)| key)
    }
}

fn first_unknown_setting(
    input: &str,
    table: &Table,
    prefix: &str,
    schema: &ConfigSchema,
) -> Option<ConfigParseError> {
    for (name, item) in table.iter() {
        let (key, _) = table
            .get_key_value(name)
            .expect("iterated TOML key should remain available");
        let path = join_path(prefix, name);
        let has_children = schema_has_children(schema, &path);

        if !schema_has_path(schema, &path) && !has_children {
            return Some(unknown_setting_error(
                input,
                prefix,
                name,
                item.is_table(),
                key.span(),
                schema,
            ));
        }

        if !has_children {
            continue;
        }

        if let Some(child_table) = item.as_table() {
            if let Some(error) = first_unknown_setting(input, child_table, &path, schema) {
                return Some(error);
            }
        } else if let Some(inline_table) = item.as_value().and_then(Value::as_inline_table) {
            if let Some(error) = first_unknown_inline_setting(input, inline_table, &path, schema) {
                return Some(error);
            }
        }
    }

    None
}

fn first_unknown_inline_setting(
    input: &str,
    table: &InlineTable,
    prefix: &str,
    schema: &ConfigSchema,
) -> Option<ConfigParseError> {
    for (name, value) in table.iter() {
        let (key, _) = table
            .get_key_value(name)
            .expect("iterated inline TOML key should remain available");
        let path = join_path(prefix, name);
        let has_children = schema_has_children(schema, &path);

        if !schema_has_path(schema, &path) && !has_children {
            return Some(unknown_setting_error(
                input,
                prefix,
                name,
                false,
                key.span(),
                schema,
            ));
        }

        if has_children {
            if let Some(child_table) = value.as_inline_table() {
                if let Some(error) = first_unknown_inline_setting(input, child_table, &path, schema)
                {
                    return Some(error);
                }
            }
        }
    }

    None
}

fn unknown_setting_error(
    input: &str,
    prefix: &str,
    name: &str,
    table: bool,
    span: Option<Range<usize>>,
    schema: &ConfigSchema,
) -> ConfigParseError {
    if prefix == "diagnostics"
        && matches!(
            name,
            "wrong_argument_count" | "parameter_type_mismatches" | "return_type_mismatches"
        )
    {
        let replacement = if name == "wrong_argument_count" {
            "TL1004"
        } else {
            "TL1003"
        };

        return parse_error(
            input,
            span,
            format!(
                "diagnostics.{name} was removed because function contracts are always checked; use `severity = {{ {replacement} = \"warning\" }}` to override its severity"
            ),
        );
    }

    let is_section = prefix.is_empty() && table;
    let mut message = if is_section {
        format!("unknown config section `{name}`")
    } else if prefix.is_empty() {
        format!("unknown config key `{name}`")
    } else {
        format!("unknown {prefix} config key `{name}`")
    };

    if let Some(suggestion) = closest_name(name, schema_child_names(schema, prefix)) {
        if is_section {
            message.push_str(&format!("; did you mean `[{suggestion}]`?"));
        } else {
            message.push_str(&format!("; did you mean `{suggestion}`?"));
        }
    }

    parse_error(input, span, message)
}

fn schema_has_path(schema: &ConfigSchema, path: &str) -> bool {
    schema.settings.iter().any(|setting| setting.path == path)
}

fn schema_has_children(schema: &ConfigSchema, path: &str) -> bool {
    let prefix = format!("{path}.");

    schema
        .settings
        .iter()
        .any(|setting| setting.path.starts_with(&prefix))
}

fn schema_child_names<'a>(schema: &'a ConfigSchema, prefix: &str) -> Vec<&'a str> {
    let nested_prefix = if prefix.is_empty() {
        None
    } else {
        Some(format!("{prefix}."))
    };
    let mut names = BTreeSet::new();

    for setting in &schema.settings {
        let remainder = match &nested_prefix {
            Some(nested_prefix) => setting.path.strip_prefix(nested_prefix),
            None => Some(setting.path.as_str()),
        };
        let Some(remainder) = remainder else {
            continue;
        };

        if let Some(name) = remainder.split('.').next() {
            names.insert(name);
        }
    }

    names.into_iter().collect()
}

fn closest_name<'a>(name: &str, candidates: Vec<&'a str>) -> Option<&'a str> {
    let name_len = name.chars().count();
    let (candidate, distance) = candidates
        .into_iter()
        .map(|candidate| (candidate, edit_distance(name, candidate)))
        .min_by_key(|(candidate, distance)| (*distance, *candidate))?;
    let candidate_len = candidate.chars().count();
    let longest = name_len.max(candidate_len);

    (distance <= 2 && distance.saturating_mul(3) <= longest).then_some(candidate)
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();

    for (left_index, left_char) in left.chars().enumerate() {
        let mut current = Vec::with_capacity(right.len() + 1);
        current.push(left_index + 1);

        for (right_index, right_char) in right.iter().enumerate() {
            let substitution = previous[right_index] + usize::from(left_char != *right_char);
            let insertion = current[right_index] + 1;
            let deletion = previous[right_index + 1] + 1;
            current.push(substitution.min(insertion).min(deletion));
        }

        previous = current;
    }

    previous[right.len()]
}

fn join_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}.{name}")
    }
}

fn parse_error(
    input: &str,
    span: Option<Range<usize>>,
    message: impl Into<String>,
) -> ConfigParseError {
    let span = normalized_span(input, span.unwrap_or(0..0));
    let (line, column) = source_position(input, span.start);
    let (end_line, end_column) = source_position(input, span.end);

    ConfigParseError {
        line,
        column,
        end_line,
        end_column,
        message: message.into(),
    }
}

fn normalized_span(input: &str, span: Range<usize>) -> Range<usize> {
    let start = floor_char_boundary(input, span.start.min(input.len()));
    let mut end = floor_char_boundary(input, span.end.min(input.len()));

    if end <= start {
        end = input[start..]
            .chars()
            .next()
            .map_or(start, |character| start + character.len_utf8());
    }

    start..end
}

fn floor_char_boundary(input: &str, mut byte: usize) -> usize {
    while byte > 0 && !input.is_char_boundary(byte) {
        byte -= 1;
    }

    byte
}

fn source_position(input: &str, byte: usize) -> (usize, usize) {
    let prefix = &input[..floor_char_boundary(input, byte.min(input.len()))];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_prefix = prefix.rsplit_once('\n').map_or(prefix, |(_, line)| line);
    let column = line_prefix.encode_utf16().count() + 1;

    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_CONFIG_TEXT;

    #[test]
    fn edit_distance_handles_insertions_deletions_and_substitutions() {
        assert_eq!(edit_distance("missing_requirez", "missing_requires"), 1);
        assert_eq!(edit_distance("format", "form"), 2);
        assert_eq!(edit_distance("source", "target"), 5);
    }

    #[test]
    fn schema_paths_and_defaults_match_default_config_text() {
        let defaults =
            toml::from_str::<toml::Value>(DEFAULT_CONFIG_TEXT).expect("defaults should parse");
        let mut default_paths = BTreeSet::new();

        collect_leaf_paths(&defaults, "", &mut default_paths);

        let schema_paths = config_schema()
            .settings
            .iter()
            .map(|setting| setting.path.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(
            schema_paths,
            default_paths.iter().map(String::as_str).collect()
        );

        for setting in &config_schema().settings {
            let value = value_at_path(&defaults, &setting.path)
                .unwrap_or_else(|| panic!("missing default for {}", setting.path));
            let value = serde_json::to_value(value).expect("TOML default should serialize");

            assert_eq!(
                value, setting.default,
                "schema default should match {}",
                setting.path
            );
        }
    }

    #[test]
    fn default_template_matches_default_config_text() {
        const DEFAULT_TEMPLATE: &str = include_str!("../../../../config/templates/default.toml");

        assert_eq!(DEFAULT_TEMPLATE, DEFAULT_CONFIG_TEXT);
    }

    #[test]
    fn schema_metadata_exposes_sections_and_settings() {
        assert!(config_section_names().contains(&"diagnostics".to_owned()));

        let diagnostics = config_settings_in_section(Some("diagnostics"));

        assert!(diagnostics
            .iter()
            .any(|setting| setting.key == "missing_requires"
                && setting.path == "diagnostics.missing_requires"
                && setting.type_name == "boolean"));

        let quote_style = config_setting_metadata("format.quote_style")
            .expect("quote style should be schema backed");

        assert_eq!(quote_style.enum_values, ["preserve", "double", "single"]);
        assert_eq!(
            closest_config_child_name("diagnostics", "missing_requirez").as_deref(),
            Some("missing_requires")
        );
    }

    fn collect_leaf_paths(value: &toml::Value, prefix: &str, paths: &mut BTreeSet<String>) {
        let Some(table) = value.as_table() else {
            paths.insert(prefix.to_owned());
            return;
        };

        if table.is_empty() {
            paths.insert(prefix.to_owned());
            return;
        }

        for (name, value) in table {
            collect_leaf_paths(value, &join_path(prefix, name), paths);
        }
    }

    fn value_at_path<'a>(value: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
        path.split('.')
            .try_fold(value, |value, key| value.as_table()?.get(key))
    }
}
