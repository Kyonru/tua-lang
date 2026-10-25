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

        let facts = Arc::new(
            self.degraded_current_semantic_facts(file)
                .or_else(|| self.last_good_semantic_facts(&path))?,
        );
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

    pub(super) fn last_good_semantic_facts(&self, path: &Path) -> Option<DegradedSemanticFacts> {
        let last_good = self.last_good_files.get(&normalize_path(path))?.clone();
        let project_inference = self.project_local_inference(&last_good.parsed);
        let environment = self.project_type_environment_for_parsed(
            &last_good.parsed,
            last_good.environment.clone(),
            &project_inference,
        );
        let semantic_index =
            SemanticIndex::build(&last_good.parsed, Some(&last_good.hir), &environment);

        Some(DegradedSemanticFacts {
            parsed: last_good.parsed.clone(),
            hir: last_good.hir.clone(),
            environment,
            semantic_index,
            project_inference,
        })
    }
}

pub(super) fn build_degraded_file_semantics(
    path: &Path,
    source: &str,
    kind: AnalysisSourceKind,
    config: &ProjectConfig,
) -> Option<DegradedFileSemantics> {
    let parsed = parse_degraded_analysis_source(path, source, kind).ok()?;
    let hir = lower_parsed_source(&parsed);
    let (environment, _) = build_analysis_type_environment_with_diagnostics(&parsed, config);

    Some(DegradedFileSemantics {
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
    parse_analysis_source(
        path,
        source_without_recoverable_incomplete_lines(source),
        kind,
    )
}

pub(super) fn source_without_recoverable_incomplete_lines(source: &str) -> String {
    let mut recovered = String::with_capacity(source.len());

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
        if line_is_recoverable_incomplete_edit(trimmed) || unfinished_argument {
            recovered.push('\n');
        } else {
            recovered.push_str(line);
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
