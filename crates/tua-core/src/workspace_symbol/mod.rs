use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    load_config_from_dir, parse_tua_source, Diagnostic, DiagnosticCode, SourcePosition, SourceSpan,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSymbol {
    pub name: String,
    pub kind: WorkspaceSymbolKind,
    pub path: PathBuf,
    pub range: SourceSpan,
    pub selection_range: SourceSpan,
    pub container_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceSymbolKind {
    TypeAlias,
    Function,
    Module,
    Local,
    Parameter,
    Field,
    Export,
}

pub fn workspace_symbols_from_dir(
    dir: impl AsRef<Path>,
    query: &str,
) -> Result<Vec<WorkspaceSymbol>, Vec<Diagnostic>> {
    let dir = dir.as_ref();
    let loaded = load_config_from_dir(dir).map_err(|error| vec![error.to_diagnostic()])?;
    let source_dir = dir.join(&loaded.config.source);
    let source_files = discover_source_files(&source_dir, &loaded.config.extension)?;

    workspace_symbols_from_files(&source_dir, source_files, query)
}

pub fn workspace_symbols_from_files(
    source_dir: impl AsRef<Path>,
    source_files: Vec<PathBuf>,
    query: &str,
) -> Result<Vec<WorkspaceSymbol>, Vec<Diagnostic>> {
    let source_dir = source_dir.as_ref();
    let mut symbols = Vec::new();
    let mut diagnostics = Vec::new();

    for path in source_files {
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(source) => {
                diagnostics.push(io_diagnostic(&path, source));
                continue;
            }
        };
        let parsed = match parse_tua_source(&path, source) {
            Ok(parsed) => parsed,
            Err(mut source_diagnostics) => {
                diagnostics.append(&mut source_diagnostics);
                continue;
            }
        };
        let module_name = module_name(source_dir, &path);
        let module_span = parsed.span.unwrap_or_else(default_span);

        symbols.push(WorkspaceSymbol {
            name: module_name.clone(),
            kind: WorkspaceSymbolKind::Module,
            path: path.clone(),
            range: module_span,
            selection_range: module_span,
            container_name: None,
        });

        symbols.extend(parsed.type_aliases.iter().filter_map(|alias| {
            let range = alias.span?;
            let selection_range = alias.name_span.unwrap_or(range);

            Some(WorkspaceSymbol {
                name: alias.name.clone(),
                kind: WorkspaceSymbolKind::TypeAlias,
                path: path.clone(),
                range,
                selection_range,
                container_name: Some(module_name.clone()),
            })
        }));

        symbols.extend(parsed.function_annotations.iter().filter_map(|function| {
            let name = function.name.clone()?;
            let range = function.span?;
            let selection_range = function.name_span.unwrap_or(range);

            Some(WorkspaceSymbol {
                name,
                kind: WorkspaceSymbolKind::Function,
                path: path.clone(),
                range,
                selection_range,
                container_name: Some(module_name.clone()),
            })
        }));
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut matches = symbols
        .into_iter()
        .filter_map(|symbol| {
            symbol_score(&symbol, query).map(|score| ScoredWorkspaceSymbol { score, symbol })
        })
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| {
        left.score
            .cmp(&right.score)
            .then_with(|| left.symbol.path.cmp(&right.symbol.path))
            .then_with(|| {
                position_key(left.symbol.selection_range.start)
                    .cmp(&position_key(right.symbol.selection_range.start))
            })
            .then_with(|| left.symbol.name.cmp(&right.symbol.name))
    });

    Ok(matches.into_iter().map(|matched| matched.symbol).collect())
}

struct ScoredWorkspaceSymbol {
    score: usize,
    symbol: WorkspaceSymbol,
}

fn symbol_score(symbol: &WorkspaceSymbol, query: &str) -> Option<usize> {
    let query = query.trim();

    if query.is_empty() {
        return Some(0);
    }

    let qualified = symbol
        .container_name
        .as_ref()
        .map(|container| format!("{container}.{}", symbol.name));

    fuzzy_score(query, &symbol.name).or_else(|| {
        qualified
            .as_deref()
            .and_then(|qualified| fuzzy_score(query, qualified).map(|score| score + 25))
    })
}

fn fuzzy_score(query: &str, candidate: &str) -> Option<usize> {
    let query = query.to_ascii_lowercase();
    let candidate = candidate.to_ascii_lowercase();

    if let Some(index) = candidate.find(&query) {
        return Some(index);
    }

    let mut last_match = None;
    let mut score = 100usize;
    let mut search_start = 0usize;

    for query_char in query.chars() {
        let (offset, _) = candidate[search_start..]
            .char_indices()
            .find(|(_, candidate_char)| *candidate_char == query_char)?;
        let index = search_start + offset;

        if let Some(last_match) = last_match {
            score += index.saturating_sub(last_match + 1);
        } else {
            score += index;
        }

        last_match = Some(index);
        search_start = index + query_char.len_utf8();
    }

    Some(score)
}

fn discover_source_files(
    source_dir: &Path,
    extension: &str,
) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    let mut files = Vec::new();

    discover_source_files_into(source_dir, extension, &mut files)?;
    files.sort();

    Ok(files)
}

fn discover_source_files_into(
    dir: &Path,
    extension: &str,
    files: &mut Vec<PathBuf>,
) -> Result<(), Vec<Diagnostic>> {
    let entries = fs::read_dir(dir).map_err(|source| vec![io_diagnostic(dir, source)])?;

    for entry in entries {
        let entry = entry.map_err(|source| vec![io_diagnostic(dir, source)])?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| vec![io_diagnostic(&path, source)])?;

        if file_type.is_dir() {
            discover_source_files_into(&path, extension, files)?;
        } else if file_type.is_file() && has_extension(&path, extension) {
            files.push(path);
        }
    }

    Ok(())
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value == extension)
}

fn module_name(source_dir: &Path, source_path: &Path) -> String {
    let relative = source_path.strip_prefix(source_dir).unwrap_or(source_path);
    let without_extension = relative.with_extension("");

    without_extension
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join(".")
}

fn io_diagnostic(path: &Path, source: std::io::Error) -> Diagnostic {
    Diagnostic::error(path, 1, 1, source.to_string()).with_code(DiagnosticCode::EMIT_IO)
}

fn default_span() -> SourceSpan {
    SourceSpan {
        start: SourcePosition { line: 1, column: 1 },
        end: SourcePosition { line: 1, column: 1 },
    }
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests;
