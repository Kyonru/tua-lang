//! Canonical, source-free identities for offline completion-ranking experiments.
//!
//! This module intentionally exposes no persisted or protocol identity yet.
//! The evaluator first has to prove that a ranker can use these structural
//! keys. A later trusted edge may HMAC the canonical bytes before persistence.

use std::collections::BTreeMap;

use full_moon::ast::LuaVersion;
use full_moon::tokenizer::{Lexer, LexerResult, TokenType};

use super::InlineCompletionSource;

const IDENTITY_SCHEMA_VERSION: u16 = 1;
const MAX_IDENTITY_TOKENS: usize = 24;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StructuralPatternKey {
    schema: u16,
    tokens: Vec<IdentityToken>,
}

impl StructuralPatternKey {
    /// Collision-safe canonical bytes for a future keyed digest. Length
    /// prefixes keep adjacent variable-width tokens unambiguous.
    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.schema.to_be_bytes());
        for token in &self.tokens {
            let (tag, text) = match token {
                IdentityToken::Symbol(text) => (1, Some(text.as_bytes())),
                IdentityToken::Concrete(text) => (2, Some(text.as_bytes())),
                IdentityToken::Slot(slot) => {
                    bytes.push(3);
                    bytes.extend_from_slice(&2_u32.to_be_bytes());
                    bytes.extend_from_slice(&slot.to_be_bytes());
                    continue;
                }
                IdentityToken::Number => (4, None),
                IdentityToken::String => (5, None),
                IdentityToken::Newline => (6, None),
            };
            bytes.push(tag);
            let text = text.unwrap_or_default();
            bytes.extend_from_slice(&(text.len() as u32).to_be_bytes());
            bytes.extend_from_slice(text);
        }
        bytes
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum IdentityToken {
    Symbol(String),
    Concrete(String),
    Slot(u16),
    Number,
    String,
    Newline,
}

#[derive(Clone, Debug)]
enum RawToken {
    Symbol(String),
    Concrete(String),
    Slot(String),
    Number,
    String,
    Newline,
}

/// Builds an alpha-normalized identity for learned-pattern candidates.
///
/// Retrieval tier and source origin are deliberately absent: the same shape
/// recalled from the current file, another project file, or the bundled corpus
/// is one pattern. Semantic snippets keep fixed priority and are not ranked by
/// personal history until they have explicit, stable rule IDs.
pub(crate) fn structural_pattern_key(
    source: InlineCompletionSource,
    document: &str,
    offset: usize,
    insertion: &str,
) -> Option<StructuralPatternKey> {
    if source == InlineCompletionSource::Semantic
        || offset > document.len()
        || !document.is_char_boundary(offset)
        || insertion.contains("${")
    {
        return None;
    }

    let prefix = &document[..offset];
    let (before_current, current_line) = prefix.rsplit_once('\n').unwrap_or(("", prefix));
    let previous_line = before_current
        .rsplit_once('\n')
        .map_or(before_current, |(_, line)| line);
    let completed = if previous_line.is_empty() {
        format!("{current_line}{insertion}")
    } else {
        format!("{previous_line}\n{current_line}{insertion}")
    };
    let raw = tokenize(&completed)?;
    let mut slots = BTreeMap::<String, u16>::new();
    let mut next_slot = 1_u16;
    let mut tokens = raw
        .into_iter()
        .map(|token| match token {
            RawToken::Symbol(text) => IdentityToken::Symbol(text),
            RawToken::Concrete(text) => IdentityToken::Concrete(text),
            RawToken::Slot(name) => {
                let slot = *slots.entry(name).or_insert_with(|| {
                    let slot = next_slot;
                    next_slot = next_slot.saturating_add(1);
                    slot
                });
                IdentityToken::Slot(slot)
            }
            RawToken::Number => IdentityToken::Number,
            RawToken::String => IdentityToken::String,
            RawToken::Newline => IdentityToken::Newline,
        })
        .collect::<Vec<_>>();
    if tokens.len() > MAX_IDENTITY_TOKENS {
        tokens.drain(..tokens.len() - MAX_IDENTITY_TOKENS);
    }
    (!tokens.is_empty()).then_some(StructuralPatternKey {
        schema: IDENTITY_SCHEMA_VERSION,
        tokens,
    })
}

fn tokenize(source: &str) -> Option<Vec<RawToken>> {
    let tokens = match Lexer::new(source, LuaVersion::luau()).collect() {
        LexerResult::Ok(tokens) | LexerResult::Recovered(tokens, _) => tokens,
        LexerResult::Fatal(_) => return None,
    };
    let mut result = Vec::new();
    for token in tokens {
        match token.token_type() {
            TokenType::Eof => {}
            TokenType::Whitespace { characters } => {
                if characters.contains('\n') && result.last().is_some() {
                    result.push(RawToken::Newline);
                }
            }
            TokenType::SingleLineComment { .. } | TokenType::Shebang { .. } => {}
            TokenType::MultiLineComment { comment, .. } => {
                if comment.contains('\n') && result.last().is_some() {
                    result.push(RawToken::Newline);
                }
            }
            TokenType::StringLiteral { .. } | TokenType::InterpolatedString { .. } => {
                result.push(RawToken::String)
            }
            TokenType::Number { .. } => result.push(RawToken::Number),
            TokenType::Identifier { identifier } => {
                let member = matches!(result.last(), Some(RawToken::Symbol(symbol)) if symbol == "." || symbol == ":");
                let name = identifier.to_string();
                result.push(if member || known_global(&name) {
                    RawToken::Concrete(name)
                } else {
                    RawToken::Slot(name)
                });
            }
            TokenType::Symbol { .. } => result.push(RawToken::Symbol(token.to_string())),
            _ => return None,
        }
    }
    Some(result)
}

fn known_global(name: &str) -> bool {
    matches!(
        name,
        "self"
            | "love"
            | "math"
            | "string"
            | "table"
            | "coroutine"
            | "io"
            | "os"
            | "utf8"
            | "pairs"
            | "ipairs"
            | "next"
            | "print"
            | "require"
            | "type"
            | "tostring"
            | "tonumber"
            | "assert"
            | "error"
            | "pcall"
            | "xpcall"
            | "select"
            | "setmetatable"
            | "getmetatable"
            | "rawget"
            | "rawset"
            | "rawequal"
            | "rawlen"
            | "collectgarbage"
            | "dofile"
            | "load"
            | "loadfile"
            | "warn"
            | "_G"
            | "package"
            | "debug"
            | "jit"
            | "bit"
            | "vec2"
            | "rgb"
            | "rgba"
            | "clamp"
            | "tau"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(document: &str, insertion: &str) -> StructuralPatternKey {
        structural_pattern_key(
            InlineCompletionSource::Project,
            document,
            document.len(),
            insertion,
        )
        .expect("structural identity")
    }

    #[test]
    fn local_renames_formatting_literals_and_retrieval_origins_are_stable() {
        let first = key("local player = world:get(1)\nplayer.", "position.x + 42");
        let second = structural_pattern_key(
            InlineCompletionSource::CurrentFile,
            "local entity=world:get(255)\nentity.",
            "local entity=world:get(255)\nentity.".len(),
            "position.x+9000",
        )
        .expect("structural identity");
        assert_eq!(first, second);
    }

    #[test]
    fn member_shapes_and_global_apis_remain_distinct() {
        assert_ne!(key("player.", "position.x"), key("player.", "position.y"));
        assert_ne!(
            key("", "math.floor(value)"),
            key("", "love.math.random(value)")
        );
    }

    #[test]
    fn the_previous_line_is_part_of_the_context() {
        assert_ne!(
            key("love.update()\nplayer.", "position.x"),
            key("love.draw()\nplayer.", "position.x")
        );
    }

    #[test]
    fn comments_and_quote_contents_do_not_leak_into_identity() {
        assert_eq!(
            key("-- private note\nprint(", "'secret')"),
            key("-- another note\nprint( ", "\"different\")")
        );
    }

    #[test]
    fn semantic_and_snippet_candidates_wait_for_named_rule_ids() {
        assert!(structural_pattern_key(
            InlineCompletionSource::Semantic,
            "loc",
            3,
            "al ${1:name} = ${0:nil}"
        )
        .is_none());
    }

    #[test]
    fn canonical_bytes_are_deterministic_and_length_delimited() {
        let identity = key("player.", "position.x");
        assert_eq!(identity.canonical_bytes(), identity.canonical_bytes());
        assert_ne!(
            identity.canonical_bytes(),
            key("player.", "position.xy").canonical_bytes()
        );
    }
}
