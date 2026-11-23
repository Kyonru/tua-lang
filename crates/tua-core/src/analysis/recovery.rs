use super::*;

impl AnalysisSnapshot {
    pub(super) fn semantic_query_environment(
        &self,
        parsed: &ParsedSource,
        degraded_facts: Option<&DegradedSemanticFacts>,
    ) -> TypeEnvironment {
        if let Some(facts) = degraded_facts {
            return facts.environment.clone();
        }

        self.semantic_facts_for_parsed(parsed).environment.clone()
    }

    pub(super) fn degraded_semantic_facts(
        &self,
        path: &Path,
        file: &AnalysisFile,
    ) -> Option<Arc<DegradedSemanticFacts>> {
        let path = normalize_path(path);

        if let Some(facts) = self
            .degraded_facts
            .lock()
            .expect("degraded semantic facts lock poisoned")
            .get(&path)
            .cloned()
        {
            return Some(facts);
        }

        let facts = self
            .degraded_current_semantic_facts(file)
            .map(Arc::new)
            .or_else(|| self.last_good_semantic_facts(&path))?;
        self.degraded_facts
            .lock()
            .expect("degraded semantic facts lock poisoned")
            .insert(path, Arc::clone(&facts));

        Some(facts)
    }

    pub(super) fn degraded_current_semantic_facts(
        &self,
        file: &AnalysisFile,
    ) -> Option<DegradedSemanticFacts> {
        let degraded = file.degraded_semantics.as_ref()?;
        let parsed = degraded.parsed.clone();
        let hir = degraded.hir.clone();
        let project_inference = self.project_local_inference(&parsed);
        let environment = self.project_type_environment_for_parsed(
            &parsed,
            degraded.environment.clone(),
            &project_inference,
        );
        let semantic_index = SemanticIndex::build(&parsed, Some(&hir), &environment);

        Some(DegradedSemanticFacts {
            parsed,
            hir,
            environment,
            semantic_index,
            project_inference,
        })
    }

    pub(super) fn last_good_semantic_facts(
        &self,
        path: &Path,
    ) -> Option<Arc<DegradedSemanticFacts>> {
        let path = normalize_path(path);
        if let Some(cached) = self
            .last_good_facts
            .lock()
            .expect("last-good semantic facts lock poisoned")
            .get(&path)
            .cloned()
        {
            return Some(cached);
        }

        let last_good = self.last_good_files.get(&path)?.clone();
        let parsed = last_good.parsed.as_ref().ok()?;
        let semantics = last_good.semantics()?;
        let hir = last_good.hir()?;
        let project_inference = self.project_local_inference(parsed);
        let environment = self.project_type_environment_for_parsed(
            parsed,
            semantics.environment.clone(),
            &project_inference,
        );
        let semantic_index = SemanticIndex::build(parsed, Some(hir), &environment);

        let facts = Arc::new(DegradedSemanticFacts {
            parsed: parsed.as_ref().clone(),
            hir: hir.clone(),
            environment,
            semantic_index,
            project_inference,
        });
        let mut cache = self
            .last_good_facts
            .lock()
            .expect("last-good semantic facts lock poisoned");
        Some(Arc::clone(
            cache.entry(path).or_insert_with(|| Arc::clone(&facts)),
        ))
    }
}

pub(super) fn build_degraded_file_semantics(
    path: &Path,
    source: &str,
    kind: AnalysisSourceKind,
    config: &ProjectConfig,
    busted_test_source: bool,
) -> Option<DegradedFileSemantics> {
    if kind == AnalysisSourceKind::Dialogue {
        return None;
    }
    let parsed = parse_degraded_analysis_source(path, source, kind).ok()?;
    let hir = lower_parsed_source(&parsed);
    let (environment, environment_diagnostics) =
        build_analysis_type_environment_with_busted_globals(&parsed, config, busted_test_source);

    let scope = if busted_test_source {
        crate::scope_map_parsed_source_with_globals(
            &parsed,
            crate::busted::GLOBAL_NAMES.iter().copied(),
        )
    } else {
        scope_map_parsed_source(&parsed)
    };

    Some(DegradedFileSemantics {
        environment_diagnostics,
        scope,
        parsed,
        hir,
        environment,
    })
}

pub(super) fn degraded_completion_fallback(
    path: &Path,
    source: &str,
    position: SourcePosition,
) -> Vec<Completion> {
    complete_tua_source(path, source.to_owned(), position).unwrap_or_default()
}

pub(super) fn degraded_signature_help_fallback(
    path: &Path,
    source: &str,
    position: SourcePosition,
) -> Option<SignatureHelp> {
    signature_help_tua_source(path, source.to_owned(), position)
        .ok()
        .flatten()
}

pub(super) fn degraded_semantic_tokens(path: &Path, file: &AnalysisFile) -> Vec<SemanticToken> {
    file.degraded_semantics
        .as_ref()
        .and_then(|degraded| semantic_tokens_parsed_source(&degraded.parsed).ok())
        .unwrap_or_else(|| {
            semantic_tokens_tua_source(path, file.source.clone()).unwrap_or_default()
        })
}

pub(super) fn parse_degraded_analysis_source(
    path: &Path,
    source: &str,
    kind: AnalysisSourceKind,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    if kind == AnalysisSourceKind::Dialogue {
        return parse_analysis_source(path, source.to_owned(), kind);
    }
    let protocol_header_ranges = if kind == AnalysisSourceKind::Tua && source.contains("protocol") {
        crate::classes::editor_protocol_header_ranges(source)
    } else {
        Vec::new()
    };
    let source = source_without_recoverable_incomplete_lines_preserving_headers(
        source,
        &protocol_header_ranges,
    );
    let recovered = if kind == AnalysisSourceKind::Tua {
        crate::classes::source_with_closed_editor_declarations(&source)
    } else {
        None
    };
    let source = recovered.unwrap_or(source);
    let parsed = match kind {
        AnalysisSourceKind::Tua => {
            crate::parser::parse_tua_source_for_analysis_recovery(path, source.clone())
        }
        AnalysisSourceKind::Lua | AnalysisSourceKind::Dialogue => {
            parse_analysis_source(path, source.clone(), kind)
        }
    };
    let Err(diagnostics) = parsed else {
        return parsed;
    };

    // A cold open has no last-good snapshot. Retry only the lines the parser
    // identified as broken, leaving intact declarations available to editor
    // queries. The original source still supplies published diagnostics.
    let mut recovered = source;
    let mut errors = diagnostics;
    for _ in 0..3 {
        let Some(next) = source_without_diagnostic_error_lines(&recovered, &errors) else {
            break;
        };
        recovered = next;
        match kind {
            AnalysisSourceKind::Tua => {
                match crate::parser::parse_tua_source_for_analysis_recovery(path, recovered.clone())
                {
                    Ok(parsed) => return Ok(parsed),
                    Err(next_errors) => errors = next_errors,
                }
            }
            AnalysisSourceKind::Lua | AnalysisSourceKind::Dialogue => {
                match parse_analysis_source(path, recovered.clone(), kind) {
                    Ok(parsed) => return Ok(parsed),
                    Err(next_errors) => errors = next_errors,
                }
            }
        }
    }
    Err(errors)
}

fn source_without_diagnostic_error_lines(source: &str, errors: &[Diagnostic]) -> Option<String> {
    let lines = source.split_inclusive('\n').collect::<Vec<_>>();
    let mut masked = std::collections::BTreeSet::new();
    for error in errors {
        let line_index = error.line.saturating_sub(1);
        let Some(line) = lines.get(line_index) else {
            continue;
        };
        let trimmed = line.trim_start();
        if trimmed.starts_with("local function ") {
            let next_is_body = lines.get(line_index + 1).is_some_and(|next| {
                next.chars()
                    .take_while(|character| character.is_whitespace())
                    .count()
                    > line
                        .chars()
                        .take_while(|character| character.is_whitespace())
                        .count()
            });
            if !next_is_body
                && (trimmed.trim_end().ends_with(':') || trimmed.trim_end().ends_with('('))
            {
                masked.insert(line_index);
            }
        } else if trimmed.starts_with("local ") && local_statement_has_incomplete_shape(trimmed) {
            // Unclosed blocks and long comments own following lines. Masking
            // their opener would invent top-level bindings and targets.
            masked.insert(line_index);
        }
    }
    if masked.is_empty() {
        return None;
    }
    let mut result = String::with_capacity(source.len());
    for (index, line) in lines.into_iter().enumerate() {
        if masked.contains(&index) {
            for character in line.chars() {
                result.push(if character == '\n' || character == '\r' {
                    character
                } else {
                    ' '
                });
            }
        } else {
            result.push_str(line);
        }
    }
    Some(result)
}

fn local_statement_has_incomplete_shape(line: &str) -> bool {
    let code = code_before_short_line_comment(line).trim_end();
    code.ends_with(',')
        || code.ends_with('=')
        || ["= )", "= ]", "= }"]
            .iter()
            .any(|suffix| code.ends_with(suffix))
        || has_unclosed_short_quote(code)
}

fn code_before_short_line_comment(line: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
        } else if character == '\\' && quote.is_some() {
            escaped = true;
        } else if quote == Some(character) {
            quote = None;
        } else if quote.is_none() && (character == '\'' || character == '"') {
            quote = Some(character);
        } else if quote.is_none() && line[index..].starts_with("--") {
            return &line[..index];
        }
    }
    line
}

fn has_unclosed_short_quote(line: &str) -> bool {
    let mut quote = None;
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            escaped = false;
        } else if character == '\\' && quote.is_some() {
            escaped = true;
        } else if quote == Some(character) {
            quote = None;
        } else if quote.is_none() && (character == '\'' || character == '"') {
            quote = Some(character);
        }
    }
    quote.is_some()
}

#[cfg(test)]
pub(super) fn source_without_recoverable_incomplete_lines(source: &str) -> String {
    source_without_recoverable_incomplete_lines_preserving_headers(source, &[])
}

fn source_without_recoverable_incomplete_lines_preserving_headers(
    source: &str,
    protected_ranges: &[std::ops::Range<usize>],
) -> String {
    let source = source_without_unfinished_member_access(source);
    let class_body_ranges = crate::classes::class_body_ranges(&source);
    let mut recovered = String::with_capacity(source.len());
    let mut line_start = 0;

    for line in source.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let content = content.strip_suffix('\r').unwrap_or(content);
        let trimmed = code_before_line_comment(content).trim();

        // A partially typed string argument cannot parse yet. Retain the
        // surrounding bindings and line numbers for completion queries.
        let unfinished_argument = matches!(
            crate::completion::context::completion_context(
                content,
                SourcePosition {
                    line: 1,
                    column: content.chars().count() + 1
                },
            ),
            Some(crate::completion::context::CompletionContext::StringArgument { .. })
        );
        let line_end = line_start + content.len();
        let protected_header = protected_ranges
            .iter()
            .any(|range| line_start < range.end && line_end > range.start);
        let next_line = source
            .get(line_start + line.len()..)
            .and_then(|rest| rest.split('\n').next())
            .unwrap_or("");
        let function_header_with_body = (trimmed.starts_with("local function ")
            || trimmed.starts_with("function "))
            && next_line
                .chars()
                .take_while(|character| character.is_whitespace())
                .count()
                > content
                    .chars()
                    .take_while(|character| character.is_whitespace())
                    .count();
        if !protected_header
            && !function_header_with_body
            && (line_is_recoverable_incomplete_edit(trimmed) || unfinished_argument)
        {
            if trimmed.ends_with(':')
                && !trimmed.starts_with("local function ")
                && !trimmed.starts_with("function ")
                && !content.contains("--")
                && !class_body_ranges
                    .iter()
                    .any(|range| range.contains(&line_start))
            {
                // A trailing method separator is a common cold-completion
                // state. Keep its receiver and surrounding scope in the
                // temporary parse; the original source still supplies the
                // completion context and parser diagnostics.
                recovered.push_str(content);
                recovered.push_str("__tua_completion_placeholder()");
            } else if class_body_ranges
                .iter()
                .any(|range| range.contains(&line_start))
            {
                let indentation = content.len() - content.trim_start().len();
                let indentation_chars = content[..indentation].chars().count();
                recovered.push_str(&content[..indentation]);
                recovered.push_str("do end");
                recovered.push_str(
                    &" ".repeat(
                        content
                            .chars()
                            .count()
                            .saturating_sub(indentation_chars + 6),
                    ),
                );
            } else {
                // Outside class bodies, preserve the established behavior:
                // deleting an incomplete expression keeps callback and table
                // context parseable.
                recovered.push_str(&" ".repeat(content.chars().count()));
            }
            if line.ends_with("\r\n") {
                recovered.push_str("\r\n");
            } else if line.ends_with('\n') {
                recovered.push('\n');
            }
        } else {
            recovered.push_str(line);
        }
        line_start += line.len();
    }

    recovered
}

fn source_without_unfinished_member_access(source: &str) -> String {
    use full_moon::ast::LuaVersion;
    use full_moon::tokenizer::{Lexer, LexerResult, Symbol, TokenType};

    let tokens = match Lexer::new(source, LuaVersion::luau()).collect() {
        LexerResult::Ok(tokens) | LexerResult::Recovered(tokens, _) => tokens,
        LexerResult::Fatal(_) => return source.to_owned(),
    };
    let significant = tokens
        .iter()
        .filter(|token| {
            !matches!(
                token.token_type(),
                TokenType::Whitespace { .. }
                    | TokenType::SingleLineComment { .. }
                    | TokenType::MultiLineComment { .. }
            )
        })
        .collect::<Vec<_>>();
    let mut recovered = source.to_owned();
    for pair in significant.windows(2) {
        if matches!(
            pair[0].token_type(),
            TokenType::Symbol {
                symbol: Symbol::Dot
            }
        ) && matches!(
            pair[1].token_type(),
            TokenType::Symbol {
                symbol: Symbol::RightParen
                    | Symbol::RightBracket
                    | Symbol::RightBrace
                    | Symbol::Comma
                    | Symbol::TwoDots
            }
        ) {
            // Preserve all source positions and the surrounding callback scope.
            // Lexing prevents punctuation inside strings/comments being repaired.
            let start = pair[0].start_position().bytes();
            recovered.replace_range(start..start + 1, " ");
        }
    }
    recovered
}

pub(super) fn line_is_recoverable_incomplete_edit(trimmed_code: &str) -> bool {
    if trimmed_code.is_empty() {
        return false;
    }

    trimmed_code.ends_with('.')
        || trimmed_code.ends_with(':')
        || trimmed_code.ends_with('(')
        || is_bare_dotted_field_chain(trimmed_code)
        || is_bare_identifier_expression(trimmed_code)
}

pub(super) fn is_bare_identifier_expression(value: &str) -> bool {
    is_simple_identifier(value) && !is_lua_keyword(value)
}

fn is_lua_keyword(value: &str) -> bool {
    matches!(
        value,
        "and"
            | "break"
            | "do"
            | "else"
            | "elseif"
            | "end"
            | "false"
            | "for"
            | "function"
            | "goto"
            | "if"
            | "in"
            | "local"
            | "nil"
            | "not"
            | "or"
            | "repeat"
            | "return"
            | "then"
            | "true"
            | "until"
            | "while"
    )
}

pub(super) fn is_bare_dotted_field_chain(value: &str) -> bool {
    let mut saw_dot = false;
    let mut parts = value.split('.');

    let Some(first) = parts.next() else {
        return false;
    };

    if !is_simple_identifier(first) {
        return false;
    }

    for part in parts {
        saw_dot = true;

        if !is_simple_identifier(part) {
            return false;
        }
    }

    saw_dot
}

pub(super) fn is_simple_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };

    if !(first == b'_' || first.is_ascii_alphabetic()) {
        return false;
    }

    bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

pub(super) fn code_before_line_comment(line: &str) -> &str {
    line.split_once("--").map(|(code, _)| code).unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_recovery_only_masks_the_broken_local_statement() {
        let source = "local broken = call(1,\nlocal healthy = \"x--y\"\n";
        let error = Diagnostic::error("src/main.tua", 1, 22, "trailing comma");
        let recovered = source_without_diagnostic_error_lines(source, &[error]).unwrap();
        assert_eq!(recovered.lines().count(), 2);
        assert_eq!(recovered.lines().nth(1), Some("local healthy = \"x--y\""));
        assert!(recovered.lines().next().unwrap().trim().is_empty());

        let next_line_error = Diagnostic::error("src/main.tua", 2, 1, "unexpected token");
        assert!(source_without_diagnostic_error_lines(source, &[next_line_error]).is_none());
    }
}
