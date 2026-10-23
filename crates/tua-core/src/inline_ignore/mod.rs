use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::profile::{self, DiagnosticsTraceCategory};
use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity, DiagnosticsConfig};

#[derive(Clone, Debug, Eq, PartialEq)]
enum IgnoreTarget {
    Line(usize),
    File,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InlineIgnore {
    directive: String,
    line: usize,
    target: IgnoreTarget,
    codes: Vec<DiagnosticCode>,
}

pub(crate) fn apply_inline_ignores(
    path: &Path,
    source: &str,
    diagnostics: Vec<Diagnostic>,
    config: &DiagnosticsConfig,
) -> Vec<Diagnostic> {
    let _trace = profile::diagnostics_span(DiagnosticsTraceCategory::InlineIgnores);

    let mut diagnostics = diagnostics;
    crate::diagnostic::expand_identifier_ranges(source, &mut diagnostics);
    let diagnostics = apply_inline_ignores_before_policy(path, source, diagnostics, config);
    config.apply_severity_policy(diagnostics)
}

fn apply_inline_ignores_before_policy(
    path: &Path,
    source: &str,
    diagnostics: Vec<Diagnostic>,
    config: &DiagnosticsConfig,
) -> Vec<Diagnostic> {
    if !config.inline_ignores {
        return diagnostics;
    }

    let allowed_scopes = config
        .inline_ignore_scopes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let (ignores, mut warnings) = parse_inline_ignores(path, source, config, &allowed_scopes);

    if ignores.is_empty() {
        if warnings.is_empty() {
            return diagnostics;
        }

        warnings.extend(diagnostics);
        return sort_diagnostics(warnings);
    }

    let mut used = BTreeMap::<(usize, usize), bool>::new();
    let mut kept = Vec::new();

    for diagnostic in diagnostics {
        let Some((index, matcher_index)) = matching_ignore(&ignores, &diagnostic) else {
            kept.push(diagnostic);
            continue;
        };

        used.insert((index, matcher_index), true);
    }

    for (index, ignore) in ignores.iter().enumerate() {
        for code_index in 0..ignore.codes.len() {
            if used.contains_key(&(index, code_index)) {
                continue;
            }

            warnings.push(unused_ignore_diagnostic(path, ignore, code_index));
        }
    }

    kept.extend(warnings);
    sort_diagnostics(kept)
}

pub(crate) fn active_inline_ignore_count(
    path: &Path,
    source: &str,
    config: &DiagnosticsConfig,
) -> usize {
    if !config.inline_ignores {
        return 0;
    }

    let allowed_scopes = config
        .inline_ignore_scopes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    parse_inline_ignores(path, source, config, &allowed_scopes)
        .0
        .len()
}

fn parse_inline_ignores(
    path: &Path,
    source: &str,
    config: &DiagnosticsConfig,
    allowed_scopes: &BTreeSet<&str>,
) -> (Vec<InlineIgnore>, Vec<Diagnostic>) {
    let mut ignores = Vec::new();
    let mut warnings = Vec::new();
    let first_content_line = source
        .lines()
        .position(|line| !line.trim().is_empty())
        .map(|index| index + 1);

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;
        let Some(comment) = line_comment(line) else {
            continue;
        };
        let mut parts = comment.split_whitespace();
        let Some(directive) = parts.next() else {
            continue;
        };

        if !matches!(
            directive,
            "tua-ignore-line" | "tua-ignore-next-line" | "tua-ignore-file"
        ) {
            continue;
        }

        if !allowed_scopes.contains(directive) {
            continue;
        }

        if directive == "tua-ignore-file" && first_content_line != Some(line_number) {
            warnings.push(invalid_ignore_diagnostic(
                path,
                line_number,
                "`tua-ignore-file` must be the first nonblank line in the file".to_owned(),
            ));
            continue;
        }

        if config.inline_ignore_max_per_file > 0
            && ignores.len() >= config.inline_ignore_max_per_file
        {
            warnings.push(invalid_ignore_diagnostic(
                path,
                line_number,
                format!(
                    "`{directive}` exceeds inline_ignore_max_per_file ({})",
                    config.inline_ignore_max_per_file
                ),
            ));
            continue;
        }

        let tokens = parts.collect::<Vec<_>>();
        if tokens.is_empty() {
            warnings.push(invalid_ignore_diagnostic(
                path,
                line_number,
                format!("`{directive}` requires at least one exact diagnostic code"),
            ));
            continue;
        }

        if tokens.iter().any(|token| *token == "[]" || *token == "[") {
            warnings.push(invalid_ignore_diagnostic(
                path,
                line_number,
                format!("`{directive}` has an empty ignore list"),
            ));
            continue;
        }

        let mut codes = Vec::new();

        for token in tokens {
            match parse_code(token) {
                Ok(code) => codes.push(code),
                Err(message) => warnings.push(invalid_ignore_diagnostic(
                    path,
                    line_number,
                    format!("`{directive}` {message}"),
                )),
            }
        }

        if codes.is_empty() {
            continue;
        }

        let target = match directive {
            "tua-ignore-line" => IgnoreTarget::Line(line_number),
            "tua-ignore-next-line" => IgnoreTarget::Line(line_number + 1),
            "tua-ignore-file" => IgnoreTarget::File,
            _ => continue,
        };

        ignores.push(InlineIgnore {
            directive: directive.to_owned(),
            line: line_number,
            target,
            codes,
        });
    }

    (ignores, warnings)
}

fn line_comment(line: &str) -> Option<&str> {
    let index = line_comment_start(line)?;
    let comment = &line[index + 2..];

    (!comment.starts_with("[[")).then(|| comment.trim())
}

pub(crate) fn line_comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut quote = None;
    let mut escaped = false;

    while index + 1 < bytes.len() {
        let ch = bytes[index] as char;

        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }

            index += 1;
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '-' if bytes[index + 1] == b'-' => return Some(index),
            _ => {}
        }

        index += 1;
    }

    None
}

pub(crate) fn parse_code(token: &str) -> Result<DiagnosticCode, String> {
    let normalized = token.trim_matches(|ch| ch == ',' || ch == '[' || ch == ']');

    if normalized.is_empty() {
        return Err("has an empty ignore code".to_owned());
    }

    let Some(number) = normalized
        .strip_prefix("TL")
        .or_else(|| normalized.strip_prefix("tl"))
        .and_then(|value| value.parse::<u16>().ok())
    else {
        return Err(format!("contains unknown diagnostic code `{normalized}`"));
    };

    let Some(code) = DiagnosticCode::new(number) else {
        return Err(format!("contains unknown diagnostic code `{normalized}`"));
    };

    if !code.is_known() {
        return Err(format!("contains unknown diagnostic code `{normalized}`"));
    }

    Ok(code)
}

fn matching_ignore(ignores: &[InlineIgnore], diagnostic: &Diagnostic) -> Option<(usize, usize)> {
    ignores.iter().enumerate().find_map(|(index, ignore)| {
        if !target_matches(&ignore.target, diagnostic.line) {
            return None;
        }

        ignore
            .codes
            .iter()
            .enumerate()
            .find_map(|(code_index, code)| {
                (diagnostic.code == Some(*code)).then_some((index, code_index))
            })
    })
}

fn target_matches(target: &IgnoreTarget, line: usize) -> bool {
    match target {
        IgnoreTarget::Line(target_line) => *target_line == line,
        IgnoreTarget::File => true,
    }
}

fn invalid_ignore_diagnostic(path: &Path, line: usize, message: String) -> Diagnostic {
    Diagnostic::new(path, line, 1, DiagnosticSeverity::Warning, message)
        .with_code(DiagnosticCode::LINT_INLINE_IGNORE_INVALID)
}

fn unused_ignore_diagnostic(path: &Path, ignore: &InlineIgnore, code_index: usize) -> Diagnostic {
    let target = match ignore.target {
        IgnoreTarget::Line(line) => format!("line {line}"),
        IgnoreTarget::File => "file".to_owned(),
    };
    let code = ignore
        .codes
        .get(code_index)
        .map(ToString::to_string)
        .unwrap_or_else(|| "diagnostic".to_owned());

    Diagnostic::new(
        path,
        ignore.line,
        1,
        DiagnosticSeverity::Warning,
        format!(
            "`{}` did not suppress matching {code} diagnostics on {target}",
            ignore.directive
        ),
    )
    .with_code(DiagnosticCode::LINT_INLINE_IGNORE_UNUSED)
}

fn sort_diagnostics(mut diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics.sort_by_key(|diagnostic| {
        (
            diagnostic.file.clone(),
            diagnostic.line,
            diagnostic.column,
            diagnostic.code.map(DiagnosticCode::as_u16).unwrap_or(0),
            diagnostic.message.clone(),
        )
    });
    diagnostics
}

#[cfg(test)]
mod tests;
