use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use tua_core::{
    Completion, CompletionKind, ModuleExportField, ParsedSource, SourcePosition, SourceSpan,
    TextEdit, Type,
};

pub(super) struct AutoImportCandidate<'a> {
    pub(super) current_path: &'a Path,
    pub(super) current_source: &'a str,
    pub(super) module_path: &'a Path,
    pub(super) module: &'a str,
    pub(super) import_specifier: &'a str,
    pub(super) alias: &'a str,
    pub(super) already_imported: bool,
    pub(super) name: &'a str,
    pub(super) field: &'a ModuleExportField,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TypeAutoImportPlan {
    pub(super) insert_text: String,
    pub(super) edit: Option<TextEdit>,
}

pub(super) fn type_auto_import_plan(
    current_path: &Path,
    parsed: &ParsedSource,
    import_specifier: &str,
    equivalent_specifiers: &[String],
    name: &str,
) -> TypeAutoImportPlan {
    let matches_module = |module: &str| {
        module == import_specifier || equivalent_specifiers.iter().any(|known| known == module)
    };
    for import in parsed
        .type_imports
        .iter()
        .filter(|import| !import.reexport && matches_module(&import.module))
    {
        if let Some(binding) = import
            .bindings
            .iter()
            .find(|binding| binding.imported == name)
        {
            return TypeAutoImportPlan {
                insert_text: binding.local.clone(),
                edit: None,
            };
        }
    }

    if let Some(import) = parsed.type_imports.iter().find(|import| {
        !import.reexport && matches_module(&import.module) && !import.namespace.is_empty()
    }) {
        return TypeAutoImportPlan {
            insert_text: format!("{}.{}", import.namespace, name),
            edit: None,
        };
    }

    if let Some(import) = parsed.type_imports.iter().find(|import| {
        !import.reexport && matches_module(&import.module) && !import.bindings.is_empty()
    }) {
        if let Some(position) = closing_brace_position(&parsed.source, import) {
            let multiline = position.line > import.span.start.line;
            return TypeAutoImportPlan {
                insert_text: name.to_owned(),
                edit: Some(TextEdit {
                    path: current_path.to_path_buf(),
                    span: SourceSpan {
                        start: position,
                        end: position,
                    },
                    new_text: if multiline {
                        format!("  {name},\n")
                    } else {
                        format!(", {name}")
                    },
                }),
            };
        }
    }

    TypeAutoImportPlan {
        insert_text: name.to_owned(),
        edit: Some(TextEdit {
            path: current_path.to_path_buf(),
            span: type_import_insert_span(parsed),
            new_text: format!("import type {{ {name} }} from \"{import_specifier}\"\n"),
        }),
    }
}

pub(super) fn auto_import_completion(candidate: AutoImportCandidate<'_>) -> Completion {
    let additional_text_edits = if candidate.already_imported {
        Vec::new()
    } else {
        vec![TextEdit {
            path: candidate.current_path.to_path_buf(),
            span: import_insert_span(candidate.current_source),
            new_text: format!(
                "local {} = import(\"{}\")\n",
                candidate.alias, candidate.import_specifier
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
        deprecation: candidate.field.deprecation.clone(),
        insert_text: Some(format!("{}.{}", candidate.alias, candidate.name)),
        filter_text: None,
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

fn type_import_insert_span(parsed: &ParsedSource) -> SourceSpan {
    if let Some(line) = parsed
        .type_imports
        .iter()
        .filter(|import| !import.reexport)
        .map(|import| import.span.end.line)
        .max()
    {
        let position = SourcePosition {
            line: line.saturating_add(1),
            column: 1,
        };
        return SourceSpan {
            start: position,
            end: position,
        };
    }

    let mut insert_line = 1;
    for (index, line) in parsed.source.split('\n').enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            insert_line = line_number + 1;
            continue;
        }
        break;
    }
    let position = SourcePosition {
        line: insert_line,
        column: 1,
    };
    SourceSpan {
        start: position,
        end: position,
    }
}

fn closing_brace_position(source: &str, import: &tua_core::TypeImport) -> Option<SourcePosition> {
    let start = source_offset(source, import.span.start)?;
    let module_start = source_offset(source, import.module_span.start)?;
    let offset = source.get(start..module_start)?.rfind('}')? + start;
    position_for_offset(source, offset)
}

fn source_offset(source: &str, position: SourcePosition) -> Option<usize> {
    let mut offset = 0;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        if index + 1 == position.line {
            let line_without_newline = line.strip_suffix('\n').unwrap_or(line);
            return Some(offset + byte_for_column(line_without_newline, position.column));
        }
        offset += line.len();
    }
    (position.line == source.lines().count().saturating_add(1) && position.column == 1)
        .then_some(source.len())
}

fn position_for_offset(source: &str, target: usize) -> Option<SourcePosition> {
    if target > source.len() || !source.is_char_boundary(target) {
        return None;
    }
    let before = &source[..target];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |offset| offset + 1);
    let column = source[line_start..target].chars().count() + 1;
    Some(SourcePosition { line, column })
}

fn is_local_require_line(trimmed: &str) -> bool {
    trimmed.starts_with("local ")
        && (trimmed.contains(" = require(") || trimmed.contains(" = import("))
        && (trimmed.contains('"') || trimmed.contains('\''))
}

fn default_import_alias(module: &str) -> String {
    let segment = module
        .split(['.', '/'])
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

#[cfg(test)]
mod tests {
    use super::*;
    use tua_core::parse_tua_source;

    #[test]
    fn type_import_plan_extends_multiline_imports() {
        let source = r#"import type {
  Room,
} from "./types"
local player: Player = nil
"#;
        let parsed = parse_tua_source("src/main.tua", source).expect("source should parse");
        let plan =
            type_auto_import_plan(Path::new("src/main.tua"), &parsed, "./types", &[], "Player");

        assert_eq!(plan.insert_text, "Player");
        let edit = plan.edit.expect("multiline import should be extended");
        assert_eq!(edit.new_text, "  Player,\n");
        assert_eq!(edit.span.start, SourcePosition { line: 3, column: 1 });
    }

    #[test]
    fn type_import_plan_reuses_aliased_and_equivalent_imports() {
        let source = r#"import type { Player as Actor } from "types"
local player: Player = nil
"#;
        let parsed = parse_tua_source("src/main.tua", source).expect("source should parse");
        let plan = type_auto_import_plan(
            Path::new("src/main.tua"),
            &parsed,
            "./types",
            &["types".to_owned()],
            "Player",
        );

        assert_eq!(plan.insert_text, "Actor");
        assert!(plan.edit.is_none());
    }

    #[test]
    fn type_import_plan_preserves_leading_file_comments() {
        let source = "-- Game state.\n\nlocal player: Player = nil\n";
        let parsed = parse_tua_source("src/main.tua", source).expect("source should parse");
        let plan =
            type_auto_import_plan(Path::new("src/main.tua"), &parsed, "./types", &[], "Player");

        let edit = plan.edit.expect("a new import should be inserted");
        assert_eq!(edit.span.start, SourcePosition { line: 3, column: 1 });
        assert_eq!(edit.new_text, "import type { Player } from \"./types\"\n");
    }
}
