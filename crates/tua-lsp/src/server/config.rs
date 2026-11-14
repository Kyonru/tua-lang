use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};
use tua_core::config::ConfigSettingMetadata;
use tua_core::{
    Assist, AssistEdit, AssistKind, Diagnostic, DiagnosticCode, SourcePosition, SourceSpan,
    TextEdit,
};

use super::protocol::lsp_code_action_from_core;

pub(super) fn completion_response(
    source: &str,
    position: SourcePosition,
    config_path: &Path,
) -> Value {
    let line = source_line(source, position.line);
    let cursor = byte_for_utf16_column(line, position.column);
    let before = &line[..cursor];

    if before.trim_start().starts_with('[') {
        return Value::Array(
            tua_core::config::config_section_names()
                .into_iter()
                .map(config_section_completion_item)
                .collect(),
        );
    }

    if let Some(equals) = line.find('=') {
        if cursor > equals {
            let items = value_completion_items(source, position, config_path);

            if !items.is_empty() {
                return Value::Array(items);
            }
        }
    }

    let section = section_at_line(source, position.line.saturating_sub(1));
    let mut items = tua_core::config::config_settings_in_section(section.as_deref())
        .into_iter()
        .enumerate()
        .map(|(index, setting)| setting_completion_item(setting, index))
        .collect::<Vec<_>>();

    if section.is_none() {
        items.extend(
            tua_core::config::config_section_names()
                .into_iter()
                .enumerate()
                .map(|(index, section)| {
                    let mut item = config_section_completion_item(section);
                    item["sortText"] = json!(format!("z{index:04}"));
                    item
                }),
        );
    }

    Value::Array(items)
}

pub(super) fn hover_response(source: &str, position: SourcePosition) -> Option<Value> {
    if let Some(section) = section_token_at_position(source, position) {
        let setting_names = tua_core::config::config_settings_in_section(Some(&section.name))
            .into_iter()
            .map(|setting| format!("`{}`", setting.key))
            .collect::<Vec<_>>()
            .join(", ");

        return Some(json!({
            "contents": {
                "kind": "markdown",
                "value": format!(
                    "```toml\n[{}]\n```\n\nTua configuration section.\n\nSettings: {}",
                    section.name, setting_names
                )
            },
            "range": lsp_range(section.span)
        }));
    }

    let token = key_token_at_position(source, position)?;
    let setting = tua_core::config::config_setting_metadata(&token.path)?;

    Some(json!({
        "contents": {
            "kind": "markdown",
            "value": setting_markdown(&setting)
        },
        "range": lsp_range(token.span)
    }))
}

pub(super) fn code_action_response(
    path: &Path,
    diagnostics: Vec<Diagnostic>,
    requested_kinds: &[AssistKind],
    sources: &super::analysis::AnalysisSnapshot,
) -> Value {
    if !requested_kinds.is_empty() && !requested_kinds.contains(&AssistKind::QuickFix) {
        return json!([]);
    }

    let actions = diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::CONFIG_PARSE))
        .filter_map(|diagnostic| {
            let replacement = replacement_from_message(&diagnostic.message)?;
            let title = if diagnostic.message.contains("unknown config section") {
                format!("Rename config section to `[{replacement}]`")
            } else {
                format!("Rename config key to `{replacement}`")
            };
            let span = SourceSpan {
                start: SourcePosition {
                    line: diagnostic.line,
                    column: diagnostic.column,
                },
                end: SourcePosition {
                    line: diagnostic.end_line,
                    column: diagnostic.end_column,
                },
            };
            let edit = TextEdit {
                path: path.to_path_buf(),
                span,
                new_text: replacement,
            };
            let mut changes = BTreeMap::new();

            changes.insert(path.to_path_buf(), vec![edit]);

            Some(Assist {
                title,
                kind: AssistKind::QuickFix,
                edit: AssistEdit {
                    changes,
                    create_files: Vec::new(),
                },
            })
        })
        .map(|action| lsp_code_action_from_core(action, sources))
        .collect::<Vec<_>>();

    Value::Array(actions)
}

fn value_completion_items(
    source: &str,
    position: SourcePosition,
    config_path: &Path,
) -> Vec<Value> {
    let Some(token) = key_token_at_position(source, position) else {
        return Vec::new();
    };
    let Some(setting) = tua_core::config::config_setting_metadata(&token.path) else {
        return Vec::new();
    };

    if setting.path == "i18n.catalogs.path" {
        return catalog_path_candidates(source, config_path)
            .into_iter()
            .map(|value| quoted_config_value_item(source, position, &value, "Lua catalog path"))
            .collect();
    }
    if matches!(
        setting.path.as_str(),
        "i18n.catalogs.locale" | "i18n.default_locale" | "i18n.fallback_locale"
    ) {
        let mut locales = std::collections::BTreeSet::from(["en".to_owned()]);
        for line in source.lines() {
            if !line.trim_start().starts_with("locale =")
                && !line.trim_start().starts_with("default_locale =")
                && !line.trim_start().starts_with("fallback_locale =")
            {
                continue;
            }
            if let Some((_, value)) = line.split_once('=') {
                let value = value.trim().trim_matches(['\'', '"']);
                if !value.is_empty()
                    && value.len() <= 32
                    && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                {
                    locales.insert(value.to_owned());
                }
            }
        }
        return locales
            .into_iter()
            .map(|value| quoted_config_value_item(source, position, &value, "configured locale"))
            .collect();
    }

    if !setting.enum_values.is_empty() {
        return setting
            .enum_values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                completion_item(
                    format!("\"{value}\""),
                    Some(format!("\"{value}\"")),
                    format!("enum value for {}", setting.path),
                    setting_markdown(&setting),
                    20,
                    format!("a{index:04}"),
                )
            })
            .collect();
    }

    match setting.type_name.as_str() {
        "boolean" => ["true", "false"]
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                completion_item(
                    value,
                    Some(value.to_owned()),
                    format!("boolean value for {}", setting.path),
                    setting_markdown(&setting),
                    12,
                    format!("a{index:04}"),
                )
            })
            .collect(),
        "string" | "integer" | "string-array" => {
            let value = toml_value(&setting.default);

            vec![completion_item(
                value.clone(),
                Some(value),
                format!("default value for {}", setting.path),
                setting_markdown(&setting),
                12,
                "a0000".to_owned(),
            )]
        }
        _ => Vec::new(),
    }
}

fn quoted_config_value_item(
    source: &str,
    position: SourcePosition,
    value: &str,
    detail: &str,
) -> Value {
    let line = source_line(source, position.line);
    let equals = line.find('=').unwrap_or(0);
    let start = equals + 1 + line[equals + 1..].len() - line[equals + 1..].trim_start().len();
    let end = quoted_value_end(line, start);
    let quoted = format!("{value:?}");
    let mut item = completion_item(value, None, detail, detail, 12, format!("a{value}"));
    item["textEdit"] = json!({"range": {"start": {"line": position.line - 1, "character": utf16_column_for_byte(line, start) - 1},
        "end": {"line": position.line - 1, "character": utf16_column_for_byte(line, end) - 1}}, "newText": quoted});
    item
}

fn quoted_value_end(line: &str, start: usize) -> usize {
    let Some(quote @ (b'\'' | b'"')) = line.as_bytes().get(start).copied() else {
        return line[start..]
            .find('#')
            .map_or(line.len(), |offset| start + offset);
    };
    let mut escaped = false;
    for (offset, byte) in line.as_bytes().iter().enumerate().skip(start + 1) {
        if quote == b'"' && !escaped && *byte == b'\\' {
            escaped = true;
            continue;
        }
        if !escaped && *byte == quote {
            return offset + 1;
        }
        escaped = false;
    }
    line.len()
}

fn catalog_path_candidates(source: &str, config_path: &Path) -> Vec<String> {
    let Some(root) = config_path.parent() else {
        return Vec::new();
    };
    let source_dir = source
        .lines()
        .find_map(|line| {
            let line = line.trim();
            let value = line
                .strip_prefix("source")?
                .trim_start()
                .strip_prefix('=')?
                .trim();
            (!value.is_empty()).then(|| value.trim_matches(['\'', '"']).to_owned())
        })
        .unwrap_or_else(|| "src".to_owned());
    if source_dir.contains("..") || source_dir.starts_with('/') {
        return Vec::new();
    }
    let source_root = root.join(source_dir);
    if std::fs::symlink_metadata(&source_root).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Vec::new();
    }
    let mut dirs = vec![(source_root.clone(), 0usize)];
    let mut paths = Vec::new();
    let mut seen = 0usize;
    while let Some((dir, depth)) = dirs.pop() {
        if depth > 6 || seen >= 512 {
            break;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            seen += 1;
            if seen >= 512 {
                break;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                if !entry.file_name().to_string_lossy().starts_with('.') {
                    dirs.push((path, depth + 1));
                }
            } else if kind.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("lua")
            {
                if let Ok(relative) = path.strip_prefix(&source_root) {
                    paths.push(relative.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    paths.sort();
    paths.truncate(200);
    paths
}

fn setting_completion_item(setting: ConfigSettingMetadata, index: usize) -> Value {
    let insert_text = format!("{} = {}", setting.key, toml_value(&setting.default));

    completion_item(
        setting.key.clone(),
        Some(insert_text),
        format!("tua.toml {} setting", setting.type_name),
        setting_markdown(&setting),
        5,
        format!("a{index:04}"),
    )
}

fn config_section_completion_item(section: String) -> Value {
    let setting_names = tua_core::config::config_settings_in_section(Some(&section))
        .into_iter()
        .map(|setting| format!("`{}`", setting.key))
        .collect::<Vec<_>>()
        .join(", ");

    completion_item(
        format!("[{section}]"),
        Some(format!("[{section}]")),
        "tua.toml section",
        format!("Tua configuration section.\n\nSettings: {setting_names}"),
        6,
        format!("b{section}"),
    )
}

fn completion_item(
    label: impl Into<String>,
    insert_text: Option<String>,
    detail: impl Into<String>,
    documentation: impl Into<String>,
    kind: u8,
    sort_text: String,
) -> Value {
    let mut item = json!({
        "label": label.into(),
        "kind": kind,
        "detail": detail.into(),
        "documentation": {
            "kind": "markdown",
            "value": documentation.into()
        },
        "sortText": sort_text
    });

    if let Some(insert_text) = insert_text {
        item["insertText"] = json!(insert_text);
    }

    item
}

fn setting_markdown(setting: &ConfigSettingMetadata) -> String {
    let mut markdown = format!(
        "```toml\n{} = {}\n```\n\n{}",
        setting.path,
        toml_value(&setting.default),
        setting.description
    );

    markdown.push_str(&format!(
        "\n\n- Type: `{}`\n- Default: `{}`",
        setting.type_name,
        toml_value(&setting.default)
    ));

    if !setting.enum_values.is_empty() {
        markdown.push_str(&format!(
            "\n- Allowed values: {}",
            setting
                .enum_values
                .iter()
                .map(|value| format!("`{value}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if let Some(minimum) = setting.minimum {
        markdown.push_str(&format!("\n- Minimum: `{minimum}`"));
    }

    markdown
}

fn toml_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => format!("{value:?}"),
        serde_json::Value::Array(values) => format!(
            "[{}]",
            values.iter().map(toml_value).collect::<Vec<_>>().join(", ")
        ),
        serde_json::Value::Object(values) if values.is_empty() => "{}".to_owned(),
        _ => String::new(),
    }
}

fn replacement_from_message(message: &str) -> Option<String> {
    let (_, suffix) = message.split_once("did you mean `")?;
    let (suggestion, _) = suffix.split_once('`')?;

    Some(
        suggestion
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned(),
    )
}

#[derive(Clone, Debug)]
struct ConfigToken {
    path: String,
    span: SourceSpan,
}

#[derive(Clone, Debug)]
struct ConfigSectionToken {
    name: String,
    span: SourceSpan,
}

fn key_token_at_position(source: &str, position: SourcePosition) -> Option<ConfigToken> {
    let line = source_line(source, position.line);
    let equals = line.find('=')?;
    let key_prefix = &line[..equals];
    let key_start = key_prefix
        .char_indices()
        .find(|(_, character)| !character.is_whitespace())
        .map(|(index, _)| index)?;
    let key_end = key_prefix.trim_end().len();
    let name = key_prefix[key_start..key_end].trim();

    if name.is_empty() {
        return None;
    }

    let path = if name.contains('.') {
        name.to_owned()
    } else if let Some(section) = section_at_line(source, position.line.saturating_sub(1)) {
        format!("{section}.{name}")
    } else {
        name.to_owned()
    };

    Some(ConfigToken {
        path,
        span: SourceSpan {
            start: SourcePosition {
                line: position.line,
                column: utf16_column_for_byte(line, key_start),
            },
            end: SourcePosition {
                line: position.line,
                column: utf16_column_for_byte(line, key_end),
            },
        },
    })
}

fn section_token_at_position(source: &str, position: SourcePosition) -> Option<ConfigSectionToken> {
    let line = source_line(source, position.line);
    let cursor = byte_for_utf16_column(line, position.column);
    let (name, start, end) = section_name_from_line(line)?;

    if cursor < start || cursor > end {
        return None;
    }

    Some(ConfigSectionToken {
        name: name.to_owned(),
        span: SourceSpan {
            start: SourcePosition {
                line: position.line,
                column: utf16_column_for_byte(line, start),
            },
            end: SourcePosition {
                line: position.line,
                column: utf16_column_for_byte(line, end),
            },
        },
    })
}

fn section_at_line(source: &str, line_before: usize) -> Option<String> {
    source
        .lines()
        .take(line_before)
        .filter_map(|line| section_name_from_line(line).map(|(name, _, _)| name.to_owned()))
        .last()
}

fn section_name_from_line(line: &str) -> Option<(&str, usize, usize)> {
    let line_start = line.len() - line.trim_start().len();
    let trimmed = &line[line_start..];

    if !trimmed.starts_with('[') {
        return None;
    }

    let array = trimmed.starts_with("[[");
    let close = if array {
        trimmed.find("]]")?
    } else {
        trimmed.find(']')?
    };
    let raw_start = line_start + if array { 2 } else { 1 };
    let raw_end = line_start + close;
    let raw = &line[raw_start..raw_end];
    let name_start = raw_start + raw.len() - raw.trim_start().len();
    let name_end = raw_end - raw.len() + raw.trim_end().len();
    let name = &line[name_start..name_end];

    (!name.is_empty()).then_some((name, name_start, name_end))
}

fn source_line(source: &str, line: usize) -> &str {
    source.lines().nth(line.saturating_sub(1)).unwrap_or("")
}

fn byte_for_utf16_column(line: &str, column: usize) -> usize {
    let target = column.saturating_sub(1);
    let mut width = 0;

    for (byte, character) in line.char_indices() {
        if width >= target {
            return byte;
        }
        width += character.len_utf16();
    }

    line.len()
}

fn utf16_column_for_byte(line: &str, byte: usize) -> usize {
    line[..byte.min(line.len())].encode_utf16().count() + 1
}

fn lsp_range(span: SourceSpan) -> Value {
    json!({
        "start": {
            "line": span.start.line.saturating_sub(1),
            "character": span.start.column.saturating_sub(1)
        },
        "end": {
            "line": span.end.line.saturating_sub(1),
            "character": span.end.column.saturating_sub(1)
        }
    })
}

#[cfg(test)]
mod localization_section_tests {
    use super::*;

    #[test]
    fn catalog_array_section_has_its_own_keys() {
        let source = "[i18n]\nenabled = true\n[[i18n.catalogs]]\nlo";
        assert_eq!(section_at_line(source, 3).as_deref(), Some("i18n.catalogs"));
        let items = completion_response(
            source,
            SourcePosition { line: 4, column: 3 },
            Path::new("/tmp/tua.toml"),
        );
        assert!(items
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["label"] == "locale")));
    }
}
