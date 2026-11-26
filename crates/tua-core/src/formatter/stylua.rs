//! Authored-source formatting. The isolated parser fork never supplies compiler
//! semantics: the matching compiler parser validates both sides of every edit.
use super::{FormatOptions, IndentStyle, LineEnding, QuoteStyle};
use crate::{Diagnostic, DiagnosticCode, ParsedLanguage, ParsedSource};
use stylua_lib::{CallParenType, Config, IndentType, OutputVerification};

pub(super) fn format(
    parsed: &ParsedSource,
    options: &FormatOptions,
    range: Option<stylua_lib::Range>,
) -> Result<String, Vec<Diagnostic>> {
    let line_endings = match options.line_ending {
        LineEnding::Crlf => stylua_lib::LineEndings::Windows,
        LineEnding::Preserve if parsed.source.contains("\r\n") => stylua_lib::LineEndings::Windows,
        _ => stylua_lib::LineEndings::Unix,
    };
    let config = Config {
        syntax: syntax(parsed.raw.language),
        indent_type: match options.indent_style {
            IndentStyle::Spaces => IndentType::Spaces,
            IndentStyle::Tabs => IndentType::Tabs,
        },
        indent_width: options.indent_width,
        column_width: options.line_width,
        quote_style: match options.quote_style {
            QuoteStyle::Preserve => stylua_lib::QuoteStyle::Preserve,
            QuoteStyle::AutoDouble => stylua_lib::QuoteStyle::AutoPreferDouble,
            QuoteStyle::Double => stylua_lib::QuoteStyle::ForceDouble,
            QuoteStyle::Single => stylua_lib::QuoteStyle::ForceSingle,
        },
        line_endings,
        call_parentheses: CallParenType::Input,
        ..Config::default()
    };
    let mut output =
        stylua_lib::format_code(&parsed.source, config, range, OutputVerification::Full).map_err(
            |error| {
                vec![Diagnostic::error(
                    &parsed.path,
                    1,
                    1,
                    format!("cannot safely format source: {error}"),
                )
                .with_code(DiagnosticCode::SYNTAX_PARSE)]
            },
        )?;
    if !options.final_newline {
        output.truncate(output.trim_end_matches(['\r', '\n']).len());
    }
    super::parse_source(&parsed.path, &output, parsed.raw.language).map_err(|_| {
        vec![Diagnostic::error(
            &parsed.path,
            1,
            1,
            "formatter output failed source syntax validation; source was left unchanged",
        )
        .with_code(DiagnosticCode::SYNTAX_PARSE)]
    })?;
    if preserved_payloads(&parsed.source, parsed.raw.language)
        != preserved_payloads(&output, parsed.raw.language)
    {
        return Err(vec![Diagnostic::error(
            &parsed.path,
            1,
            1,
            "formatter changed a string value or comment; source was left unchanged",
        )
        .with_code(DiagnosticCode::SYNTAX_PARSE)]);
    }
    Ok(output)
}

/// Partial editor edits must preserve the same program and payloads as a full
/// format, even when line wrapping elsewhere happens to keep the line count.
pub(super) fn equivalent_sources(source: &str, edited: &str, language: ParsedLanguage) -> bool {
    preserved_payloads(source, language) == preserved_payloads(edited, language)
        && stylua_lib::verify_code(source, edited, syntax(language)).is_ok()
}

fn syntax(language: ParsedLanguage) -> stylua_lib::LuaVersion {
    match language {
        ParsedLanguage::Lua => stylua_lib::LuaVersion::LuaJIT,
        ParsedLanguage::Tua => stylua_lib::LuaVersion::Tua,
    }
}

fn preserved_payloads(source: &str, language: ParsedLanguage) -> Vec<(bool, Vec<u8>)> {
    use full_moon::{
        ast::LuaVersion,
        tokenizer::{Lexer, LexerResult, TokenReference, TokenType},
    };
    let version = match language {
        ParsedLanguage::Lua => crate::parser::lua_source_version(),
        ParsedLanguage::Tua => LuaVersion::luau(),
    };
    let LexerResult::Ok(tokens) = Lexer::new(source, version).collect() else {
        return vec![(false, source.as_bytes().to_vec())];
    };
    tokens
        .into_iter()
        .filter_map(|token| match token.token_type() {
            TokenType::StringLiteral { .. } => {
                let reference = TokenReference::new(Vec::new(), token.clone(), Vec::new());
                Some((
                    true,
                    crate::ast_utils::string_literal_runtime_bytes(&reference)
                        .unwrap_or_else(|| token.to_string().into_bytes()),
                ))
            }
            TokenType::SingleLineComment { comment }
            | TokenType::MultiLineComment { comment, .. } => Some((
                false,
                comment
                    .lines()
                    .map(str::trim_end)
                    .collect::<Vec<_>>()
                    .join("\n")
                    .into_bytes(),
            )),
            _ => None,
        })
        .collect()
}
