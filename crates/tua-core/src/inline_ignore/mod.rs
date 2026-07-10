use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::profile::{self, DiagnosticsTraceCategory};
use crate::{
    Diagnostic, DiagnosticCategory, DiagnosticCode, DiagnosticSeverity, DiagnosticsConfig,
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum IgnoreTarget {
    Line(usize),
    File,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum IgnoreMatcher {
    All,
    Code(DiagnosticCode),
    Category(DiagnosticCategory),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InlineIgnore {
    directive: String,
    line: usize,
    target: IgnoreTarget,
    matchers: Vec<IgnoreMatcher>,
}

pub(crate) fn apply_inline_ignores(
    path: &Path,
    source: &str,
    diagnostics: Vec<Diagnostic>,
    config: &DiagnosticsConfig,
) -> Vec<Diagnostic> {
    let _trace = profile::diagnostics_span(DiagnosticsTraceCategory::InlineIgnores);

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
        for matcher_index in matcher_indices(ignore) {
            if used.contains_key(&(index, matcher_index)) {
                continue;
            }

            warnings.push(unused_ignore_diagnostic(path, ignore, matcher_index));
        }
    }

    kept.extend(warnings);
    sort_diagnostics(kept)
}

fn parse_inline_ignores(
    path: &Path,
    source: &str,
    config: &DiagnosticsConfig,
    allowed_scopes: &BTreeSet<&str>,
) -> (Vec<InlineIgnore>, Vec<Diagnostic>) {
    let mut ignores = Vec::new();
    let mut warnings = Vec::new();

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
        if tokens.iter().any(|token| *token == "[]" || *token == "[") {
            warnings.push(invalid_ignore_diagnostic(
                path,
                line_number,
                format!("`{directive}` has an empty ignore list"),
            ));
            continue;
        }

        let matchers = if tokens.is_empty() {
            vec![IgnoreMatcher::All]
        } else {
            let mut matchers = Vec::new();

            for token in tokens {
                match parse_matcher(token) {
                    Ok(matcher) => matchers.push(matcher),
                    Err(message) => warnings.push(invalid_ignore_diagnostic(
                        path,
                        line_number,
                        format!("`{directive}` {message}"),
                    )),
                }
            }

            if matchers.is_empty() {
                continue;
            }

            matchers
        };

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
            matchers,
        });
    }

    (ignores, warnings)
}

fn line_comment(line: &str) -> Option<&str> {
    let index = line_comment_start(line)?;
    let comment = &line[index + 2..];

    (!comment.starts_with("[[")).then(|| comment.trim())
}

fn line_comment_start(line: &str) -> Option<usize> {
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

fn parse_matcher(token: &str) -> Result<IgnoreMatcher, String> {
    let normalized = token.trim_matches(|ch| ch == ',' || ch == '[' || ch == ']');

    if normalized.is_empty() {
        return Err("has an empty ignore code".to_owned());
    }

    match normalized {
        "syntax" => return Ok(IgnoreMatcher::Category(DiagnosticCategory::Syntax)),
        "type" | "types" => return Ok(IgnoreMatcher::Category(DiagnosticCategory::Type)),
        "lowering" | "emit" | "emission" => {
            return Ok(IgnoreMatcher::Category(
                DiagnosticCategory::LoweringEmission,
            ));
        }
        "config" | "project" => {
            return Ok(IgnoreMatcher::Category(DiagnosticCategory::ProjectConfig));
        }
        "lint" | "style" => return Ok(IgnoreMatcher::Category(DiagnosticCategory::Lint)),
        "plugin" | "plugins" => return Ok(IgnoreMatcher::Category(DiagnosticCategory::Plugin)),
        _ => {}
    }

    let Some(number) = normalized
        .strip_prefix("TL")
        .or_else(|| normalized.strip_prefix("tl"))
        .and_then(|value| value.parse::<u16>().ok())
    else {
        return Err(format!("contains unknown diagnostic code `{normalized}`"));
    };

    if let Some(category) = group_category(number) {
        return Ok(IgnoreMatcher::Category(category));
    }

    let Some(code) = DiagnosticCode::new(number) else {
        return Err(format!("contains unknown diagnostic code `{normalized}`"));
    };

    if !code.is_known() {
        return Err(format!("contains unknown diagnostic code `{normalized}`"));
    }

    Ok(IgnoreMatcher::Code(code))
}

fn group_category(number: u16) -> Option<DiagnosticCategory> {
    match number {
        0 => Some(DiagnosticCategory::Syntax),
        1000 => Some(DiagnosticCategory::Type),
        2000 => Some(DiagnosticCategory::LoweringEmission),
        3000 => Some(DiagnosticCategory::ProjectConfig),
        4000 => Some(DiagnosticCategory::Lint),
        5000 => Some(DiagnosticCategory::Plugin),
        _ => None,
    }
}

fn matching_ignore(ignores: &[InlineIgnore], diagnostic: &Diagnostic) -> Option<(usize, usize)> {
    ignores.iter().enumerate().find_map(|(index, ignore)| {
        if !target_matches(&ignore.target, diagnostic.line) {
            return None;
        }

        ignore
            .matchers
            .iter()
            .enumerate()
            .find_map(|(matcher_index, matcher)| {
                matcher_matches(matcher, diagnostic).then_some((index, matcher_index))
            })
    })
}

fn target_matches(target: &IgnoreTarget, line: usize) -> bool {
    match target {
        IgnoreTarget::Line(target_line) => *target_line == line,
        IgnoreTarget::File => true,
    }
}

fn matcher_matches(matcher: &IgnoreMatcher, diagnostic: &Diagnostic) -> bool {
    match matcher {
        IgnoreMatcher::All => diagnostic
            .code
            .is_some_and(|code| code.category() != DiagnosticCategory::Syntax),
        IgnoreMatcher::Code(code) => diagnostic.code == Some(*code),
        IgnoreMatcher::Category(category) => diagnostic
            .code
            .is_some_and(|code| code.category() == *category),
    }
}

fn matcher_indices(ignore: &InlineIgnore) -> Vec<usize> {
    if ignore.matchers.contains(&IgnoreMatcher::All) {
        vec![0]
    } else {
        (0..ignore.matchers.len()).collect()
    }
}

fn invalid_ignore_diagnostic(path: &Path, line: usize, message: String) -> Diagnostic {
    Diagnostic::new(path, line, 1, DiagnosticSeverity::Warning, message)
        .with_code(DiagnosticCode::LINT_INLINE_IGNORE_INVALID)
}

fn unused_ignore_diagnostic(
    path: &Path,
    ignore: &InlineIgnore,
    matcher_index: usize,
) -> Diagnostic {
    let target = match ignore.target {
        IgnoreTarget::Line(line) => format!("line {line}"),
        IgnoreTarget::File => "file".to_owned(),
    };
    let matcher = ignore
        .matchers
        .get(matcher_index)
        .map(format_matcher)
        .unwrap_or_else(|| "diagnostic".to_owned());

    Diagnostic::new(
        path,
        ignore.line,
        1,
        DiagnosticSeverity::Warning,
        format!(
            "`{}` did not suppress matching {matcher} diagnostics on {target}",
            ignore.directive
        ),
    )
    .with_code(DiagnosticCode::LINT_INLINE_IGNORE_UNUSED)
}

fn format_matcher(matcher: &IgnoreMatcher) -> String {
    match matcher {
        IgnoreMatcher::All => "non-syntax".to_owned(),
        IgnoreMatcher::Code(code) => code.to_string(),
        IgnoreMatcher::Category(category) => match category {
            DiagnosticCategory::Syntax => "syntax".to_owned(),
            DiagnosticCategory::Type => "type".to_owned(),
            DiagnosticCategory::LoweringEmission => "lowering/emission".to_owned(),
            DiagnosticCategory::ProjectConfig => "project config".to_owned(),
            DiagnosticCategory::Lint => "lint".to_owned(),
            DiagnosticCategory::Plugin => "plugin".to_owned(),
        },
    }
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
