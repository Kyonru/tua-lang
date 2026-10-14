use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::Path;

use crate::scope::ScopeDeclarationKind;
use crate::source_text::{byte_for_column, line_at};
use crate::{
    AnalysisHost, AnalysisSnapshot, Diagnostic, DiagnosticSeverity, SourcePosition, SourceSpan,
};

pub const DEFAULT_MODEL_CONTEXT_BYTES: usize = 12 * 1024;
pub const MAX_MODEL_CONTEXT_BYTES: usize = 32 * 1024;
pub const MAX_MODEL_COMPLETION_BYTES: usize = 8 * 1024;
pub const MAX_MODEL_COMPLETION_LINES: usize = 64;
const LOCAL_MODEL_COMPLETION_BYTES: usize = 1024;
const LOCAL_MODEL_COMPLETION_LINES: usize = 8;
pub const LANGUAGE_CONTRACT: &str = include_str!("../../../llms.txt");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCompletionContext {
    pub context_id: String,
    pub analysis_revision: u64,
    pub context_bytes: usize,
    pub source_fingerprint: String,
    pub position: SourcePosition,
    pub prompt: String,
    pub suffix: String,
    pub insertion_span: SourceSpan,
    pub completion_scope: ModelCompletionScope,
    pub max_completion_bytes: usize,
    pub max_completion_lines: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelCompletionScope {
    Local,
    Function,
}

impl ModelCompletionScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Function => "function",
        }
    }

    fn max_bytes(self) -> usize {
        match self {
            Self::Local => LOCAL_MODEL_COMPLETION_BYTES,
            Self::Function => MAX_MODEL_COMPLETION_BYTES,
        }
    }

    fn max_lines(self) -> usize {
        match self {
            Self::Local => LOCAL_MODEL_COMPLETION_LINES,
            Self::Function => MAX_MODEL_COMPLETION_LINES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelCompletionStatus {
    Accepted,
    Rejected,
}

impl ModelCompletionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelCompletionRejectionReason {
    Stale,
    Empty,
    MarkdownFence,
    ForeignSyntax,
    TooManyLines,
    TooLarge,
    NewError,
}

impl ModelCompletionRejectionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stale => "stale",
            Self::Empty => "empty",
            Self::MarkdownFence => "markdown_fence",
            Self::ForeignSyntax => "foreign_syntax",
            Self::TooManyLines => "too_many_lines",
            Self::TooLarge => "too_large",
            Self::NewError => "new_error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCompletionValidation {
    pub status: ModelCompletionStatus,
    pub reason: Option<ModelCompletionRejectionReason>,
    pub normalized_insertion: Option<String>,
    pub insertion_span: SourceSpan,
}

#[derive(Clone, Copy, Debug)]
pub struct ModelCompletionValidationRequest<'a> {
    pub context_id: &'a str,
    pub analysis_revision: u64,
    pub context_bytes: usize,
    pub source_fingerprint: &'a str,
    pub position: SourcePosition,
    pub candidate: &'a str,
}

impl ModelCompletionValidation {
    fn accepted(insertion: String, insertion_span: SourceSpan) -> Self {
        Self {
            status: ModelCompletionStatus::Accepted,
            reason: None,
            normalized_insertion: Some(insertion),
            insertion_span,
        }
    }

    fn rejected(reason: ModelCompletionRejectionReason, insertion_span: SourceSpan) -> Self {
        Self {
            status: ModelCompletionStatus::Rejected,
            reason: Some(reason),
            normalized_insertion: None,
            insertion_span,
        }
    }
}

impl AnalysisSnapshot {
    pub fn model_completion_context(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
        requested_budget: usize,
    ) -> Option<ModelCompletionContext> {
        let path = path.as_ref();
        let source = self.source(path)?;
        let cursor = byte_offset_for_position(source, position)?;
        if cursor_is_in_comment_or_string(source, cursor) {
            return None;
        }

        let budget = requested_budget.clamp(1024, MAX_MODEL_CONTEXT_BYTES);
        let source_fingerprint = stable_hash(&(path, source));
        let insertion_span = SourceSpan {
            start: position,
            end: position,
        };

        let completion_scope = self.model_completion_scope(path, position, source, cursor);
        let semantic_preamble = self.model_completion_preamble(path, position, completion_scope);
        let suffix_budget = budget / 4;
        let suffix = utf8_prefix(&source[cursor..], suffix_budget).to_owned();
        let prompt_budget = budget.saturating_sub(suffix.len());
        let semantic_budget = prompt_budget.saturating_mul(2) / 3;
        let semantic_preamble = comment_preamble_prefix(&semantic_preamble, semantic_budget);
        let source_budget = prompt_budget.saturating_sub(semantic_preamble.len());
        let prefix = utf8_suffix(&source[..cursor], source_budget);
        let prompt = format!("{semantic_preamble}{prefix}");
        let context_id = stable_hash(&(
            self.revision(),
            &source_fingerprint,
            position.line,
            position.column,
            &prompt,
            &suffix,
            completion_scope.as_str(),
        ));

        Some(ModelCompletionContext {
            context_id,
            analysis_revision: self.revision(),
            context_bytes: budget,
            source_fingerprint,
            position,
            prompt,
            suffix,
            insertion_span,
            completion_scope,
            max_completion_bytes: completion_scope.max_bytes(),
            max_completion_lines: completion_scope.max_lines(),
        })
    }

    fn model_completion_scope(
        &self,
        path: &Path,
        position: SourcePosition,
        source: &str,
        cursor: usize,
    ) -> ModelCompletionScope {
        if self.containing_function(path, position).is_some()
            || textual_cursor_is_in_lua_function(&source[..cursor])
            || textual_cursor_is_in_tua_method(&source[..cursor])
        {
            ModelCompletionScope::Function
        } else {
            ModelCompletionScope::Local
        }
    }

    fn containing_function(&self, path: &Path, position: SourcePosition) -> Option<&str> {
        self.scope_map(path).and_then(|scope| {
            scope
                .declarations()
                .iter()
                .filter(|declaration| {
                    matches!(declaration.kind, ScopeDeclarationKind::Function)
                        && position_key(declaration.scope_span.start) <= position_key(position)
                        && position_key(position) <= position_key(declaration.scope_span.end)
                })
                .max_by_key(|declaration| position_key(declaration.scope_span.start))
                .map(|declaration| declaration.name.as_str())
        })
    }

    fn model_completion_preamble(
        &self,
        path: &Path,
        position: SourcePosition,
        completion_scope: ModelCompletionScope,
    ) -> String {
        let mut facts = Vec::new();
        let environment = self.type_environment(path);

        if let (Some(scope), Some(environment)) = (self.scope_map(path), environment) {
            let mut declarations = scope
                .declarations()
                .iter()
                .filter(|declaration| {
                    matches!(
                        declaration.kind,
                        ScopeDeclarationKind::Local
                            | ScopeDeclarationKind::Parameter
                            | ScopeDeclarationKind::Function
                            | ScopeDeclarationKind::ModuleAlias { .. }
                    ) && position_key(declaration.name_span.start) <= position_key(position)
                        && position_key(declaration.scope_span.start) <= position_key(position)
                        && position_key(position) <= position_key(declaration.scope_span.end)
                })
                .collect::<Vec<_>>();
            declarations.sort_by_key(|declaration| {
                (
                    std::cmp::Reverse(declaration.scope_span.start.line),
                    std::cmp::Reverse(declaration.scope_span.start.column),
                    declaration.name.clone(),
                )
            });

            let mut seen = BTreeMap::new();
            for declaration in declarations {
                if seen.insert(declaration.name.as_str(), ()).is_some() {
                    continue;
                }
                let type_text = environment
                    .local(&declaration.name)
                    .map(ToString::to_string)
                    .or_else(|| {
                        environment
                            .function(&declaration.name)
                            .and_then(|function| function.as_type())
                            .map(|ty| ty.to_string())
                    })
                    .unwrap_or_else(|| "any".to_owned());
                let import = match &declaration.kind {
                    ScopeDeclarationKind::ModuleAlias { module } => {
                        format!(" (import {module:?})")
                    }
                    _ => String::new(),
                };
                facts.push(format!("{}: {}{}", declaration.name, type_text, import));
                if facts.len() == 24 {
                    break;
                }
            }
        }

        let containing_function = self.containing_function(path, position);
        let parsed_containing_class = self.parse(path).and_then(Result::ok).and_then(|parsed| {
            parsed
                .class_declarations
                .iter()
                .filter(|class| {
                    position_key(class.span.start) <= position_key(position)
                        && position_key(position) <= position_key(class.span.end)
                })
                .max_by_key(|class| position_key(class.span.start))
                .map(|class| class.name.as_str())
        });
        let containing_class = parsed_containing_class.map(str::to_owned).or_else(|| {
            self.source(path)
                .and_then(|source| textual_containing_class(source, position))
        });
        let expected_type = self
            .source(path)
            .and_then(|source| expected_type_at_cursor(source, position, environment));

        let mut preamble = model_harness_preamble();
        match completion_scope {
            ModelCompletionScope::Local => preamble.push_str(
                "-- COMPLETION SCOPE: Finish only the current expression or nearby statement. \
                 Keep this completion local and concise.\n",
            ),
            ModelCompletionScope::Function => preamble.push_str(
                "-- COMPLETION SCOPE: Complete the remainder of the current function or method. \
                 Emit all useful body statements and required closing `end` or `}` tokens, but \
                 do not repeat closing code already present in the suffix. Multiline output up \
                 to 64 lines is allowed.\n",
            ),
        }
        preamble.push_str("-- Cursor semantics:\n");
        if let Some(function) = containing_function {
            preamble.push_str(&format!("-- Containing function: {function}\n"));
        }
        if let Some(class) = containing_class {
            preamble.push_str(&format!("-- Containing class: {class}\n"));
        }
        if let Some(expected_type) = expected_type {
            preamble.push_str(&format!("-- Expected type: {expected_type}\n"));
        }
        if facts.is_empty() {
            preamble.push_str("-- Visible typed bindings: none resolved\n");
        } else {
            preamble.push_str("-- Visible typed bindings:\n");
            for fact in facts {
                preamble.push_str("-- - ");
                preamble.push_str(&fact);
                preamble.push('\n');
            }
        }
        preamble.push_str("-- Tua language contract and feature inventory:\n");
        append_commented_language_contract(&mut preamble);
        preamble.push_str("-- End harness. Current Tua source begins below.\n");
        preamble
    }
}

impl AnalysisHost {
    pub fn validate_model_completion(
        &self,
        path: impl AsRef<Path>,
        request: ModelCompletionValidationRequest<'_>,
    ) -> ModelCompletionValidation {
        let path = path.as_ref();
        let insertion_span = SourceSpan {
            start: request.position,
            end: request.position,
        };
        let snapshot = self.snapshot();
        let Some(context) =
            snapshot.model_completion_context(path, request.position, request.context_bytes)
        else {
            return ModelCompletionValidation::rejected(
                ModelCompletionRejectionReason::Stale,
                insertion_span,
            );
        };
        if context.context_id != request.context_id
            || context.analysis_revision != request.analysis_revision
            || context.source_fingerprint != request.source_fingerprint
        {
            return ModelCompletionValidation::rejected(
                ModelCompletionRejectionReason::Stale,
                insertion_span,
            );
        }

        let normalized = normalize_candidate(request.candidate, &context.suffix);
        if normalized.is_empty() {
            return ModelCompletionValidation::rejected(
                ModelCompletionRejectionReason::Empty,
                insertion_span,
            );
        }
        if normalized.contains("```") {
            return ModelCompletionValidation::rejected(
                ModelCompletionRejectionReason::MarkdownFence,
                insertion_span,
            );
        }
        if normalized.len() > context.max_completion_bytes {
            return ModelCompletionValidation::rejected(
                ModelCompletionRejectionReason::TooLarge,
                insertion_span,
            );
        }
        if normalized.lines().count() > context.max_completion_lines {
            return ModelCompletionValidation::rejected(
                ModelCompletionRejectionReason::TooManyLines,
                insertion_span,
            );
        }
        if contains_foreign_syntax(
            snapshot.source(path).unwrap_or_default(),
            request.position,
            &normalized,
        ) {
            return ModelCompletionValidation::rejected(
                ModelCompletionRejectionReason::ForeignSyntax,
                insertion_span,
            );
        }

        let Some(source) = snapshot.source(path) else {
            return ModelCompletionValidation::rejected(
                ModelCompletionRejectionReason::Stale,
                insertion_span,
            );
        };
        let Some(cursor) = byte_offset_for_position(source, request.position) else {
            return ModelCompletionValidation::rejected(
                ModelCompletionRejectionReason::Stale,
                insertion_span,
            );
        };
        let mut edited = String::with_capacity(source.len() + normalized.len());
        edited.push_str(&source[..cursor]);
        edited.push_str(&normalized);
        edited.push_str(&source[cursor..]);

        let baseline = baseline_error_multiset(snapshot.diagnostics(path));
        let mut candidate_host = self.detached_clone_for_ephemeral_analysis();
        if snapshot.is_open_document(path) {
            candidate_host.open_document(path.to_path_buf(), edited);
        } else {
            candidate_host.set_file(path.to_path_buf(), edited);
        }
        let candidate_errors = candidate_error_multiset(
            candidate_host.snapshot().diagnostics(path),
            request.position,
            &normalized,
        );
        if candidate_errors
            .iter()
            .any(|(signature, count)| *count > baseline.get(signature).copied().unwrap_or(0))
        {
            return ModelCompletionValidation::rejected(
                ModelCompletionRejectionReason::NewError,
                insertion_span,
            );
        }

        ModelCompletionValidation::accepted(normalized, insertion_span)
    }
}

pub fn language_contract() -> &'static str {
    LANGUAGE_CONTRACT
}

fn model_harness_preamble() -> String {
    let mut preamble = String::with_capacity(2 * 1024);
    preamble.push_str(
        "-- TUA INLINE COMPLETION HARNESS. Continue this as ordinary Lua 5.1/LuaJIT code \
         with optional Tua additions, never as TypeScript, JavaScript, Luau, Python, or \
         Markdown.\n",
    );
    preamble.push_str(
        "-- Output only the exact code inserted at the cursor; do not explain, fence, or \
         repeat suffix text.\n",
    );
    preamble.push_str(
        "-- INFERENCE: write `local score = 0`, not `local score: number = 0`; annotate a \
         local only when inference cannot express its intended wider or empty-container \
         contract.\n",
    );
    preamble.push_str(
        "-- LOVE: when game context needs it, use the LOVE 11.x `love.*` API, callbacks, \
         and colon calls such as `image:setFilter(...)`.\n",
    );
    preamble.push_str(
        "-- FORBIDDEN: const, let, =>, interface, null, undefined, console.*, JS object \
         syntax, and } as a Lua function terminator.\n",
    );
    preamble.push_str(
        "-- Ordinary Tua function example: function add(a: number, b: number): number\n\
         --   return a + b\n\
         -- end\n",
    );
    preamble.push_str(
        "-- Native class method example: update(dt: number) { self.x += dt }\n\
         -- Inferred LOVE local example: local image = love.graphics.newImage(\"player.png\")\n",
    );
    preamble
}

fn append_commented_language_contract(preamble: &mut String) {
    for line in LANGUAGE_CONTRACT.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("# ") {
            continue;
        }
        preamble.push_str("-- ");
        preamble.push_str(line.strip_prefix("- ").unwrap_or(line));
        preamble.push('\n');
    }
}

fn normalize_candidate(candidate: &str, suffix: &str) -> String {
    let mut normalized = candidate
        .replace("<|fim_prefix|>", "")
        .replace("<|fim_suffix|>", "")
        .replace("<|fim_middle|>", "")
        .replace("<|endoftext|>", "")
        .replace("<|im_end|>", "");

    let max_overlap = normalized.len().min(suffix.len());
    let overlap = (1..=max_overlap)
        .rev()
        .find(|length| {
            normalized.is_char_boundary(normalized.len() - length)
                && suffix.is_char_boundary(*length)
                && normalized[normalized.len() - length..] == suffix[..*length]
        })
        .unwrap_or(0);
    normalized.truncate(normalized.len() - overlap);
    normalized.trim_end().to_owned()
}

fn contains_foreign_syntax(source: &str, position: SourcePosition, candidate: &str) -> bool {
    let code = code_without_strings_and_comments(candidate);
    let has_foreign_operator = ["=>", "===", "!==", "??", "?."]
        .iter()
        .any(|operator| code.contains(operator));
    if has_foreign_operator {
        return true;
    }

    let has_foreign_statement = code.lines().any(|line| {
        let line = line.trim_start();
        [
            "const ",
            "let ",
            "interface ",
            "namespace ",
            "export default ",
            "async function ",
        ]
        .iter()
        .any(|prefix| line.starts_with(prefix))
            || line.starts_with("import {")
            || line.contains("function ") && line.trim_end().ends_with('{')
    });
    if has_foreign_statement {
        return true;
    }

    let tokens = ascii_identifiers(&code);
    if tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "null" | "undefined" | "None" | "True" | "False"
        )
    }) || code.contains("console.")
        || code.contains("this.")
    {
        return true;
    }

    let Some(cursor) = byte_offset_for_position(source, position) else {
        return false;
    };
    let prefix = code_without_strings_and_comments(&source[..cursor]);
    let current_line = prefix.rsplit('\n').next().unwrap_or(&prefix).trim_start();
    let open_brace_depth = prefix
        .chars()
        .fold(0usize, |depth, character| match character {
            '{' => depth + 1,
            '}' => depth.saturating_sub(1),
            _ => depth,
        });

    (current_line.starts_with("function ") && code.trim_start().starts_with('{'))
        || (open_brace_depth == 0 && code.lines().any(|line| line.trim() == "}"))
}

fn code_without_strings_and_comments(value: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        SingleQuote(bool),
        DoubleQuote(bool),
        LineComment,
        LongString(usize),
        LongComment(usize),
    }

    let bytes = value.as_bytes();
    let mut output = String::with_capacity(value.len());
    let mut state = State::Code;
    let mut index = 0;
    while index < bytes.len() {
        let current = bytes[index];
        let next = bytes.get(index + 1).copied();
        match state {
            State::Code if current == b'-' && next == Some(b'-') => {
                output.push_str("  ");
                index += 2;
                if let Some((depth, width)) = long_bracket_open(bytes, index) {
                    output.extend(std::iter::repeat_n(' ', width));
                    index += width;
                    state = State::LongComment(depth);
                } else {
                    state = State::LineComment;
                }
            }
            State::Code if current == b'[' => {
                if let Some((depth, width)) = long_bracket_open(bytes, index) {
                    output.extend(std::iter::repeat_n(' ', width));
                    index += width;
                    state = State::LongString(depth);
                } else {
                    output.push('[');
                    index += 1;
                }
            }
            State::Code if current == b'\'' => {
                output.push(' ');
                index += 1;
                state = State::SingleQuote(false);
            }
            State::Code if current == b'"' => {
                output.push(' ');
                index += 1;
                state = State::DoubleQuote(false);
            }
            State::SingleQuote(escaped) => {
                output.push(if current == b'\n' { '\n' } else { ' ' });
                index += 1;
                state = if current == b'\'' && !escaped {
                    State::Code
                } else if escaped {
                    State::SingleQuote(false)
                } else {
                    State::SingleQuote(current == b'\\')
                };
            }
            State::DoubleQuote(escaped) => {
                output.push(if current == b'\n' { '\n' } else { ' ' });
                index += 1;
                state = if current == b'"' && !escaped {
                    State::Code
                } else if escaped {
                    State::DoubleQuote(false)
                } else {
                    State::DoubleQuote(current == b'\\')
                };
            }
            State::LineComment => {
                output.push(if current == b'\n' { '\n' } else { ' ' });
                index += 1;
                if current == b'\n' {
                    state = State::Code;
                }
            }
            State::LongString(depth) | State::LongComment(depth) => {
                if long_bracket_close(bytes, index, depth) {
                    let width = depth + 2;
                    output.extend(std::iter::repeat_n(' ', width));
                    index += width;
                    state = State::Code;
                } else if current.is_ascii() {
                    output.push(if current == b'\n' { '\n' } else { ' ' });
                    index += 1;
                } else {
                    let character = value[index..].chars().next().expect("valid UTF-8");
                    output.push(' ');
                    index += character.len_utf8();
                }
            }
            State::Code if current.is_ascii() => {
                output.push(current as char);
                index += 1;
            }
            State::Code => {
                let character = value[index..].chars().next().expect("valid UTF-8");
                output.push(character);
                index += character.len_utf8();
            }
        }
    }
    output
}

fn ascii_identifiers(value: &str) -> Vec<String> {
    value
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|token| {
            token.starts_with(|character: char| character.is_ascii_alphabetic() || character == '_')
        })
        .map(ToOwned::to_owned)
        .collect()
}

fn textual_cursor_is_in_lua_function(prefix: &str) -> bool {
    #[derive(Clone, Copy)]
    enum Block {
        Function,
        Other,
        Repeat,
    }

    let code = code_without_strings_and_comments(prefix);
    let mut blocks = Vec::new();
    let mut pending_loop_do = 0usize;

    for token in ascii_identifiers(&code) {
        match token.as_str() {
            "function" => blocks.push(Block::Function),
            "if" => blocks.push(Block::Other),
            "for" | "while" => {
                blocks.push(Block::Other);
                pending_loop_do += 1;
            }
            "repeat" => blocks.push(Block::Repeat),
            "do" if pending_loop_do > 0 => pending_loop_do -= 1,
            "do" => blocks.push(Block::Other),
            "end" => {
                blocks.pop();
            }
            "until" => {
                if let Some(index) = blocks
                    .iter()
                    .rposition(|block| matches!(block, Block::Repeat))
                {
                    blocks.truncate(index);
                }
            }
            _ => {}
        }
    }

    blocks.iter().any(|block| matches!(block, Block::Function))
}

fn textual_cursor_is_in_tua_method(prefix: &str) -> bool {
    let code = code_without_strings_and_comments(prefix);
    let mut blocks = Vec::new();
    let mut segment_start = 0usize;

    for (index, character) in code.char_indices() {
        match character {
            '{' => {
                let header = code[segment_start..index].trim();
                blocks.push(looks_like_tua_method_header(header));
                segment_start = index + character.len_utf8();
            }
            '}' => {
                blocks.pop();
                segment_start = index + character.len_utf8();
            }
            '\n' => segment_start = index + character.len_utf8(),
            _ => {}
        }
    }

    blocks.iter().any(|is_method| *is_method)
}

fn looks_like_tua_method_header(header: &str) -> bool {
    if header.is_empty() || header.contains('=') {
        return false;
    }
    let Some(open_parenthesis) = header.find('(') else {
        return false;
    };
    let Some(close_parenthesis) = header.rfind(')') else {
        return false;
    };
    if close_parenthesis < open_parenthesis {
        return false;
    }

    let before = header[..open_parenthesis].trim();
    let name = before.split_whitespace().last().unwrap_or_default();
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && !matches!(
            name,
            "if" | "for" | "while" | "function" | "return" | "local"
        )
}

fn expected_type_at_cursor(
    source: &str,
    position: SourcePosition,
    environment: Option<&crate::TypeEnvironment>,
) -> Option<String> {
    let line = line_at(source, position.line)?;
    let cursor = byte_for_column(line, position.column).min(line.len());
    let before = &line[..cursor];
    let equals = before.rfind('=')?;
    let left = before[..equals].trim();

    if let Some(colon) = left.rfind(':') {
        let annotated = left[colon + 1..].trim();
        if !annotated.is_empty() {
            return Some(annotated.to_owned());
        }
    }

    let name = left
        .strip_prefix("local ")
        .unwrap_or(left)
        .split_whitespace()
        .last()?;
    environment?.local(name).map(ToString::to_string)
}

fn textual_containing_class(source: &str, position: SourcePosition) -> Option<String> {
    let mut brace_depth = 0usize;
    let mut classes = Vec::<(String, usize)>::new();

    for line in source.lines().take(position.line) {
        let code = line.split("--").next().unwrap_or(line);
        if let Some(class_index) = code.find("class ") {
            let before = code[..class_index].trim();
            if before.is_empty() || matches!(before, "open" | "final") {
                let name = code[class_index + "class ".len()..]
                    .split(|character: char| {
                        character.is_whitespace() || matches!(character, '{' | ':')
                    })
                    .next()
                    .unwrap_or("");
                if !name.is_empty() && code.contains('{') {
                    classes.push((name.to_owned(), brace_depth + 1));
                }
            }
        }

        brace_depth = brace_depth.saturating_add(code.matches('{').count());
        brace_depth = brace_depth.saturating_sub(code.matches('}').count());
        while classes
            .last()
            .is_some_and(|(_, class_depth)| brace_depth < *class_depth)
        {
            classes.pop();
        }
    }

    classes.last().map(|(name, _)| name.clone())
}

type DiagnosticSignature = (Option<u16>, String, bool, usize, usize, usize, usize);

fn baseline_error_multiset(diagnostics: Vec<Diagnostic>) -> BTreeMap<DiagnosticSignature, usize> {
    let mut errors = BTreeMap::new();
    for diagnostic in diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        *errors
            .entry((
                diagnostic.code.map(|code| code.as_u16()),
                diagnostic.message,
                false,
                diagnostic.line,
                diagnostic.column,
                diagnostic.end_line,
                diagnostic.end_column,
            ))
            .or_insert(0) += 1;
    }
    errors
}

fn candidate_error_multiset(
    diagnostics: Vec<Diagnostic>,
    insertion_start: SourcePosition,
    insertion: &str,
) -> BTreeMap<DiagnosticSignature, usize> {
    let insertion_end = insertion_end_position(insertion_start, insertion);
    let mut errors = BTreeMap::new();
    for diagnostic in diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        let start = SourcePosition {
            line: diagnostic.line,
            column: diagnostic.column,
        };
        let end = SourcePosition {
            line: diagnostic.end_line,
            column: diagnostic.end_column,
        };
        let intersects_insertion = position_key(start) < position_key(insertion_end)
            && position_key(insertion_start) < position_key(end);
        let mapped_start =
            map_position_before_insertion(start, insertion_start, insertion_end).unwrap_or(start);
        let mapped_end =
            map_position_before_insertion(end, insertion_start, insertion_end).unwrap_or(end);
        *errors
            .entry((
                diagnostic.code.map(|code| code.as_u16()),
                diagnostic.message,
                intersects_insertion,
                mapped_start.line,
                mapped_start.column,
                mapped_end.line,
                mapped_end.column,
            ))
            .or_insert(0) += 1;
    }
    errors
}

fn insertion_end_position(start: SourcePosition, insertion: &str) -> SourcePosition {
    let line_count = insertion.bytes().filter(|byte| *byte == b'\n').count();
    if line_count == 0 {
        SourcePosition {
            line: start.line,
            column: start.column + insertion.chars().count(),
        }
    } else {
        SourcePosition {
            line: start.line + line_count,
            column: insertion
                .rsplit('\n')
                .next()
                .unwrap_or_default()
                .chars()
                .count()
                + 1,
        }
    }
}

fn map_position_before_insertion(
    position: SourcePosition,
    insertion_start: SourcePosition,
    insertion_end: SourcePosition,
) -> Option<SourcePosition> {
    if position_key(position) < position_key(insertion_start) {
        return Some(position);
    }
    if position_key(position) < position_key(insertion_end) {
        return None;
    }

    if insertion_start.line == insertion_end.line {
        return Some(if position.line == insertion_end.line {
            SourcePosition {
                line: position.line,
                column: insertion_start.column
                    + position.column.saturating_sub(insertion_end.column),
            }
        } else {
            position
        });
    }

    Some(if position.line == insertion_end.line {
        SourcePosition {
            line: insertion_start.line,
            column: insertion_start.column + position.column.saturating_sub(insertion_end.column),
        }
    } else {
        SourcePosition {
            line: position
                .line
                .saturating_sub(insertion_end.line - insertion_start.line),
            column: position.column,
        }
    })
}

fn byte_offset_for_position(source: &str, position: SourcePosition) -> Option<usize> {
    let mut offset = 0;
    for (index, line_with_newline) in source.split_inclusive('\n').enumerate() {
        let line_number = index + 1;
        let line = line_with_newline
            .strip_suffix('\n')
            .unwrap_or(line_with_newline)
            .strip_suffix('\r')
            .unwrap_or_else(|| {
                line_with_newline
                    .strip_suffix('\n')
                    .unwrap_or(line_with_newline)
            });
        if line_number == position.line {
            return Some(offset + byte_for_column(line, position.column).min(line.len()));
        }
        offset += line_with_newline.len();
    }

    if position.line == source.lines().count().saturating_add(1) && source.ends_with('\n') {
        return Some(source.len());
    }
    if position.line == 1 && source.is_empty() {
        return Some(0);
    }
    line_at(source, position.line).map(|line| {
        source.len().saturating_sub(line.len()) + byte_for_column(line, position.column)
    })
}

fn cursor_is_in_comment_or_string(source: &str, cursor: usize) -> bool {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        SingleQuote(bool),
        DoubleQuote(bool),
        LineComment,
        LongString(usize),
        LongComment(usize),
    }

    let bytes = source.as_bytes();
    let mut state = State::Code;
    let mut index = 0;
    while index < cursor {
        let current = bytes[index];
        let next = bytes.get(index + 1).copied();
        state = match state {
            State::Code if current == b'-' && next == Some(b'-') => {
                if let Some((depth, width)) = long_bracket_open(bytes, index + 2) {
                    index += width + 1;
                    State::LongComment(depth)
                } else {
                    index += 1;
                    State::LineComment
                }
            }
            State::Code if current == b'[' => {
                if let Some((depth, width)) = long_bracket_open(bytes, index) {
                    index += width - 1;
                    State::LongString(depth)
                } else {
                    State::Code
                }
            }
            State::Code if current == b'\'' => State::SingleQuote(false),
            State::Code if current == b'"' => State::DoubleQuote(false),
            State::SingleQuote(escaped) if current == b'\'' && !escaped => State::Code,
            State::SingleQuote(true) => State::SingleQuote(false),
            State::SingleQuote(false) => State::SingleQuote(current == b'\\'),
            State::DoubleQuote(escaped) if current == b'"' && !escaped => State::Code,
            State::DoubleQuote(true) => State::DoubleQuote(false),
            State::DoubleQuote(false) => State::DoubleQuote(current == b'\\'),
            State::LineComment if current == b'\n' => State::Code,
            State::LongString(depth) if long_bracket_close(bytes, index, depth) => {
                index += depth + 1;
                State::Code
            }
            State::LongComment(depth) if long_bracket_close(bytes, index, depth) => {
                index += depth + 1;
                State::Code
            }
            other => other,
        };
        index += 1;
    }

    !matches!(state, State::Code)
}

fn long_bracket_open(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    if bytes.get(start) != Some(&b'[') {
        return None;
    }
    let mut index = start + 1;
    while bytes.get(index) == Some(&b'=') {
        index += 1;
    }
    (bytes.get(index) == Some(&b'[')).then_some((index - start - 1, index - start + 1))
}

fn long_bracket_close(bytes: &[u8], start: usize, depth: usize) -> bool {
    bytes.get(start) == Some(&b']')
        && bytes
            .get(start + 1..start + 1 + depth)
            .is_some_and(|middle| middle.iter().all(|byte| *byte == b'='))
        && bytes.get(start + depth + 1) == Some(&b']')
}

fn utf8_prefix(value: &str, max_bytes: usize) -> &str {
    let mut end = value.len().min(max_bytes);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn comment_preamble_prefix(value: &str, max_bytes: usize) -> &str {
    let prefix = utf8_prefix(value, max_bytes);
    prefix
        .rfind('\n')
        .map(|last_newline| &prefix[..=last_newline])
        .unwrap_or_default()
}

fn utf8_suffix(value: &str, max_bytes: usize) -> &str {
    let mut start = value.len().saturating_sub(max_bytes);
    while !value.is_char_boundary(start) {
        start += 1;
    }
    &value[start..]
}

fn stable_hash(value: &impl Hash) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn host(source: &str) -> AnalysisHost {
        let mut host = AnalysisHost::default();
        host.open_document(PathBuf::from("main.tua"), source.to_owned());
        host
    }

    #[test]
    fn context_is_deterministic_and_includes_visible_types() {
        let host = host("local speed: number = 10\nlocal x: number = 0\nx += ");
        let snapshot = host.snapshot();
        let position = SourcePosition { line: 3, column: 6 };
        let first = snapshot
            .model_completion_context("main.tua", position, DEFAULT_MODEL_CONTEXT_BYTES)
            .unwrap();
        let second = snapshot
            .model_completion_context("main.tua", position, DEFAULT_MODEL_CONTEXT_BYTES)
            .unwrap();

        assert_eq!(first, second);
        assert_eq!(first.completion_scope, ModelCompletionScope::Local);
        assert_eq!(first.max_completion_lines, LOCAL_MODEL_COMPLETION_LINES);
        assert!(first.prompt.starts_with("-- TUA INLINE COMPLETION HARNESS"));
        assert!(first.prompt.contains("never as TypeScript"));
        assert!(first.prompt.contains("write `local score = 0`"));
        assert!(first.prompt.contains("LOVE 11.x"));
        assert!(first.prompt.contains("Signals, stores, state machines"));
        assert!(first.prompt.contains(
            "-- Ordinary Tua function example: function add(a: number, b: number): number"
        ));
        assert!(first.prompt.contains("speed: number"));
        assert!(first.prompt.ends_with("x += "));
    }

    #[test]
    fn context_respects_budget_and_utf8_boundaries() {
        let source = format!("local label = {:?}\n{}", "é".repeat(10_000), "x = ");
        let host = host(&source);
        let snapshot = host.snapshot();
        let position = SourcePosition { line: 2, column: 5 };
        let context = snapshot
            .model_completion_context("main.tua", position, 1024)
            .unwrap();

        assert!(context.prompt.len() + context.suffix.len() <= 1024);
        assert!(context.prompt.is_char_boundary(context.prompt.len()));
        assert!(comment_preamble_prefix("-- first\n-- truncated", 14).ends_with('\n'));
    }

    #[test]
    fn bounded_context_keeps_lua_love_feature_index_and_source() {
        let source = format!("local image = {}", "\n-- suffix context".repeat(1_000));
        let host = host(&source);
        let context = host
            .snapshot()
            .model_completion_context(
                "main.tua",
                SourcePosition {
                    line: 1,
                    column: 15,
                },
                DEFAULT_MODEL_CONTEXT_BYTES,
            )
            .unwrap();

        assert!(context.prompt.contains("Tua is Lua first"));
        assert!(context.prompt.contains("LOVE 11.x foundation"));
        assert!(context.prompt.contains("Baton input,"));
        assert!(context
            .prompt
            .contains("object pools, ECS, and `audioSystem`"));
        assert!(context.prompt.ends_with("local image = "));
        assert!(!context.suffix.is_empty());
    }

    #[test]
    fn language_contract_lists_lua_tua_and_love_features() {
        let contract = language_contract();

        for expected in [
            "Tua is Lua first",
            "Locals infer one stable widened type",
            "LuaDoc/LuaCATS",
            "LOVE 11.x foundation",
            "Tua types and syntax",
            "spriteSheet({...})",
            "stateMachine({...})",
            "objectPool({...})",
            "Built-in `ecs`",
            "Never emit",
        ] {
            assert!(
                contract.contains(expected),
                "language contract should describe {expected:?}"
            );
        }
    }

    #[test]
    fn context_is_not_returned_inside_comments_or_strings() {
        let quoted_host = host("-- comment\nlocal value = \"text\"");
        let snapshot = quoted_host.snapshot();

        assert!(snapshot
            .model_completion_context(
                "main.tua",
                SourcePosition { line: 1, column: 5 },
                DEFAULT_MODEL_CONTEXT_BYTES,
            )
            .is_none());
        assert!(snapshot
            .model_completion_context(
                "main.tua",
                SourcePosition {
                    line: 2,
                    column: 18,
                },
                DEFAULT_MODEL_CONTEXT_BYTES,
            )
            .is_none());

        let host = host("local text = [=[long value]=]\n--[=[long comment]=]");
        let snapshot = host.snapshot();
        assert!(snapshot
            .model_completion_context(
                "main.tua",
                SourcePosition {
                    line: 1,
                    column: 20,
                },
                DEFAULT_MODEL_CONTEXT_BYTES,
            )
            .is_none());
        assert!(snapshot
            .model_completion_context(
                "main.tua",
                SourcePosition {
                    line: 2,
                    column: 12,
                },
                DEFAULT_MODEL_CONTEXT_BYTES,
            )
            .is_none());
    }

    #[test]
    fn context_reports_expected_type_and_incomplete_containing_class() {
        let host = host(
            "final class Counter {\n  value: number\n  update() {\n    local next: number = \n  }\n}\n",
        );
        let context = host
            .snapshot()
            .model_completion_context(
                "main.tua",
                SourcePosition {
                    line: 4,
                    column: 26,
                },
                DEFAULT_MODEL_CONTEXT_BYTES,
            )
            .unwrap();
        assert!(host.cached_snapshot().is_some());

        assert!(context.prompt.contains("Containing class: Counter"));
        assert!(context.prompt.contains("Expected type: number"));
        assert_eq!(context.completion_scope, ModelCompletionScope::Function);
    }

    #[test]
    fn validation_rejects_stale_and_new_errors_without_mutating_host() {
        let host = host("local value: number = ");
        let before_revision = host.revision();
        let position = SourcePosition {
            line: 1,
            column: 23,
        };
        let context = host
            .snapshot()
            .model_completion_context("main.tua", position, DEFAULT_MODEL_CONTEXT_BYTES)
            .unwrap();

        let stale = host.validate_model_completion(
            "main.tua",
            ModelCompletionValidationRequest {
                context_id: "wrong",
                analysis_revision: context.analysis_revision,
                context_bytes: context.context_bytes,
                source_fingerprint: &context.source_fingerprint,
                position,
                candidate: "1",
            },
        );
        assert_eq!(stale.reason, Some(ModelCompletionRejectionReason::Stale));

        let invalid = host.validate_model_completion(
            "main.tua",
            ModelCompletionValidationRequest {
                context_id: &context.context_id,
                analysis_revision: context.analysis_revision,
                context_bytes: context.context_bytes,
                source_fingerprint: &context.source_fingerprint,
                position,
                candidate: "\"text\"",
            },
        );
        assert_eq!(
            invalid.reason,
            Some(ModelCompletionRejectionReason::NewError)
        );
        assert_eq!(host.revision(), before_revision);
        assert!(host.cached_snapshot().is_some());
        assert_eq!(
            host.snapshot().source("main.tua"),
            Some("local value: number = ")
        );
    }

    #[test]
    fn validation_normalizes_fim_tokens_and_suffix_duplication() {
        let host = host("local value = \nprint(value)");
        let position = SourcePosition {
            line: 1,
            column: 15,
        };
        let context = host
            .snapshot()
            .model_completion_context("main.tua", position, DEFAULT_MODEL_CONTEXT_BYTES)
            .unwrap();
        let result = host.validate_model_completion(
            "main.tua",
            ModelCompletionValidationRequest {
                context_id: &context.context_id,
                analysis_revision: context.analysis_revision,
                context_bytes: context.context_bytes,
                source_fingerprint: &context.source_fingerprint,
                position,
                candidate: "<|fim_middle|>1\nprint(value)<|endoftext|>",
            },
        );

        assert_eq!(result.status, ModelCompletionStatus::Accepted);
        assert_eq!(result.normalized_insertion.as_deref(), Some("1"));
    }

    #[test]
    fn validation_rejects_typescript_in_an_incomplete_function() {
        let host = host("function add(a: number, b: number): number\n  ");
        let position = SourcePosition { line: 2, column: 3 };
        let context = host
            .snapshot()
            .model_completion_context("main.tua", position, DEFAULT_MODEL_CONTEXT_BYTES)
            .unwrap();

        for candidate in [
            "const result: number = a + b;\nreturn result;\n}",
            "return a + b;\n}",
            "(value: number) => value + 1",
        ] {
            let result = host.validate_model_completion(
                "main.tua",
                ModelCompletionValidationRequest {
                    context_id: &context.context_id,
                    analysis_revision: context.analysis_revision,
                    context_bytes: context.context_bytes,
                    source_fingerprint: &context.source_fingerprint,
                    position,
                    candidate,
                },
            );

            assert_eq!(
                result.reason,
                Some(ModelCompletionRejectionReason::ForeignSyntax),
                "candidate should be rejected: {candidate:?}"
            );
        }

        let valid = host.validate_model_completion(
            "main.tua",
            ModelCompletionValidationRequest {
                context_id: &context.context_id,
                analysis_revision: context.analysis_revision,
                context_bytes: context.context_bytes,
                source_fingerprint: &context.source_fingerprint,
                position,
                candidate: "return a + b\nend",
            },
        );
        assert_eq!(valid.status, ModelCompletionStatus::Accepted);
    }

    #[test]
    fn function_context_accepts_a_complete_multiline_body() {
        let host = host("function update(dt: number)\n  ");
        let position = SourcePosition { line: 2, column: 3 };
        let context = host
            .snapshot()
            .model_completion_context("main.tua", position, DEFAULT_MODEL_CONTEXT_BYTES)
            .unwrap();

        assert_eq!(context.completion_scope, ModelCompletionScope::Function);
        assert_eq!(context.max_completion_lines, MAX_MODEL_COMPLETION_LINES);
        assert_eq!(context.max_completion_bytes, MAX_MODEL_COMPLETION_BYTES);
        assert!(context
            .prompt
            .contains("Complete the remainder of the current function"));

        let mut candidate = String::new();
        for index in 0..10 {
            candidate.push_str(&format!("local value{index} = dt + {index}\n"));
        }
        candidate.push_str("print(value9)\nend");

        let result = host.validate_model_completion(
            "main.tua",
            ModelCompletionValidationRequest {
                context_id: &context.context_id,
                analysis_revision: context.analysis_revision,
                context_bytes: context.context_bytes,
                source_fingerprint: &context.source_fingerprint,
                position,
                candidate: &candidate,
            },
        );

        assert_eq!(candidate.lines().count(), 12);
        assert_eq!(result.status, ModelCompletionStatus::Accepted);
        assert_eq!(result.normalized_insertion.as_deref(), Some(&*candidate));
    }

    #[test]
    fn local_context_keeps_long_unrelated_completions_bounded() {
        let host = host("");
        let position = SourcePosition { line: 1, column: 1 };
        let context = host
            .snapshot()
            .model_completion_context("main.tua", position, DEFAULT_MODEL_CONTEXT_BYTES)
            .unwrap();
        let candidate = (0..=LOCAL_MODEL_COMPLETION_LINES)
            .map(|index| format!("local value{index} = {index}"))
            .collect::<Vec<_>>()
            .join("\n");

        let result = host.validate_model_completion(
            "main.tua",
            ModelCompletionValidationRequest {
                context_id: &context.context_id,
                analysis_revision: context.analysis_revision,
                context_bytes: context.context_bytes,
                source_fingerprint: &context.source_fingerprint,
                position,
                candidate: &candidate,
            },
        );

        assert_eq!(context.completion_scope, ModelCompletionScope::Local);
        assert_eq!(
            result.reason,
            Some(ModelCompletionRejectionReason::TooManyLines)
        );
    }

    #[test]
    fn foreign_syntax_guard_ignores_strings_comments_and_long_strings() {
        let source = "local value = ";
        let position = SourcePosition {
            line: 1,
            column: 15,
        };

        assert!(!contains_foreign_syntax(
            source,
            position,
            "\"const null => console.log\""
        ));
        assert!(!contains_foreign_syntax(
            source,
            position,
            "[=[const null => console.log]=]"
        ));
        assert!(!contains_foreign_syntax(
            source,
            position,
            "1 -- const null => console.log"
        ));

        let native_method =
            "local text = \"function fake()\"\nfinal class Example {\n  run() {\n    ";
        assert!(!contains_foreign_syntax(
            native_method,
            SourcePosition { line: 4, column: 5 },
            "}"
        ));
    }

    #[test]
    fn candidate_diagnostics_inside_insertion_cannot_hide_behind_baseline_error() {
        let initial_host = host("local value = ");
        let position = SourcePosition {
            line: 1,
            column: 15,
        };
        let context = initial_host
            .snapshot()
            .model_completion_context("main.tua", position, DEFAULT_MODEL_CONTEXT_BYTES)
            .unwrap();
        let result = initial_host.validate_model_completion(
            "main.tua",
            ModelCompletionValidationRequest {
                context_id: &context.context_id,
                analysis_revision: context.analysis_revision,
                context_bytes: context.context_bytes,
                source_fingerprint: &context.source_fingerprint,
                position,
                candidate: "{",
            },
        );

        assert_eq!(
            result.reason,
            Some(ModelCompletionRejectionReason::NewError)
        );

        let shifted_host = host("local value = 0\nlocal bad: number = \"text\"");
        let shifted_position = SourcePosition {
            line: 1,
            column: 16,
        };
        let context = shifted_host
            .snapshot()
            .model_completion_context("main.tua", shifted_position, DEFAULT_MODEL_CONTEXT_BYTES)
            .unwrap();
        let shifted_existing_error = shifted_host.validate_model_completion(
            "main.tua",
            ModelCompletionValidationRequest {
                context_id: &context.context_id,
                analysis_revision: context.analysis_revision,
                context_bytes: context.context_bytes,
                source_fingerprint: &context.source_fingerprint,
                position: shifted_position,
                candidate: "\nlocal other = 2",
            },
        );
        assert_eq!(
            shifted_existing_error.status,
            ModelCompletionStatus::Accepted
        );
    }
}
