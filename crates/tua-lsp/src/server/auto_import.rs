use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use tua_core::{
    Completion, CompletionKind, ModuleExportField, SourcePosition, SourceSpan, TextEdit, Type,
};

pub(super) struct AutoImportCandidate<'a> {
    pub(super) current_path: &'a Path,
    pub(super) current_source: &'a str,
    pub(super) module_path: &'a Path,
    pub(super) module: &'a str,
    pub(super) alias: &'a str,
    pub(super) already_imported: bool,
    pub(super) name: &'a str,
    pub(super) field: &'a ModuleExportField,
}

pub(super) fn auto_import_completion(candidate: AutoImportCandidate<'_>) -> Completion {
    let additional_text_edits = if candidate.already_imported {
        Vec::new()
    } else {
        vec![TextEdit {
            path: candidate.current_path.to_path_buf(),
            span: import_insert_span(candidate.current_source),
            new_text: format!(
                "local {} = require(\"{}\")\n",
                candidate.alias, candidate.module
            ),
        }]
    };

    Completion {
        label: candidate.name.to_owned(),
        kind: completion_kind_for_export(&candidate.field.ty),
        detail: Some(format!(
            "auto import from {}: {}",
            candidate.module, candidate.field.ty
        )),
        documentation: candidate.field.documentation.clone(),
        insert_text: Some(format!("{}.{}", candidate.alias, candidate.name)),
        is_snippet: false,
        module_path: Some(PathBuf::from(candidate.module_path)),
        source_module: Some(candidate.module.to_owned()),
        additional_text_edits,
    }
}

pub(super) fn identifier_prefix_at(source: &str, position: SourcePosition) -> Option<String> {
    let line = line_at(source, position.line)?;
    let cursor = byte_for_column(line, position.column);

    if inside_string_literal(line, cursor) {
        return None;
    }

    let mut start = cursor;

    while start > 0 && is_identifier_continue(line.as_bytes()[start - 1]) {
        start -= 1;
    }

    if start > 0 && matches!(line.as_bytes()[start - 1], b'.' | b':') {
        return None;
    }

    let prefix = &line[start..cursor];

    if prefix.is_empty() {
        return Some(String::new());
    }

    is_identifier_start(prefix.as_bytes()[0]).then(|| prefix.to_owned())
}

pub(super) fn unique_import_alias(module: &str, reserved: &BTreeSet<String>) -> String {
    let base = default_import_alias(module);

    if !reserved.contains(&base) {
        return base;
    }

    let module_alias = format!("{base}Module");

    if !reserved.contains(&module_alias) {
        return module_alias;
    }

    for index in 2.. {
        let candidate = format!("{module_alias}{index}");

        if !reserved.contains(&candidate) {
            return candidate;
        }
    }

    unreachable!("finite reserved aliases always leave a numeric suffix available")
}

fn completion_kind_for_export(ty: &Type) -> CompletionKind {
    match ty {
        Type::Function(_) => CompletionKind::Function,
        _ => CompletionKind::Value,
    }
}

fn inside_string_literal(line: &str, cursor: usize) -> bool {
    let mut quote = None;
    let mut escaped = false;

    for character in line[..cursor.min(line.len())].chars() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        match quote {
            Some(active) if character == active => quote = None,
            Some(_) => {}
            None if character == '"' || character == '\'' => quote = Some(character),
            None => {}
        }
    }

    quote.is_some()
}

fn import_insert_span(source: &str) -> SourceSpan {
    let mut insert_line = 1;

    for (index, line) in source.split('\n').enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with("--") || is_local_require_line(trimmed) {
            insert_line = line_number + 1;
            continue;
        }

        break;
    }

    SourceSpan {
        start: SourcePosition {
            line: insert_line,
            column: 1,
        },
        end: SourcePosition {
            line: insert_line,
            column: 1,
        },
    }
}

fn is_local_require_line(trimmed: &str) -> bool {
    trimmed.starts_with("local ")
        && trimmed.contains(" = require(")
        && (trimmed.contains('"') || trimmed.contains('\''))
}

fn default_import_alias(module: &str) -> String {
    let segment = module
        .split('.')
        .next_back()
        .filter(|segment| !segment.is_empty())
        .unwrap_or("module");
    let mut output = String::new();
    let mut uppercase_next = true;

    for character in segment.chars() {
        if !character.is_ascii_alphanumeric() {
            uppercase_next = true;
            continue;
        }

        if output.is_empty() && character.is_ascii_digit() {
            output.push('M');
        }

        if uppercase_next {
            output.push(character.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            output.push(character);
        }
    }

    if output.is_empty() {
        "Module".to_owned()
    } else {
        output
    }
}

fn line_at(source: &str, line: usize) -> Option<&str> {
    source
        .split('\n')
        .nth(line.checked_sub(1)?)
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

fn byte_for_column(line: &str, column: usize) -> usize {
    let target = column.saturating_sub(1);

    line.char_indices()
        .nth(target)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

fn is_identifier_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}
