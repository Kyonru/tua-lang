//! Authored-source formatting. The isolated parser fork never supplies compiler
//! semantics: production Tua parsing validates both sides of every edit.
use super::{FormatOptions, IndentStyle, LineEnding, QuoteStyle};
use crate::{parse_tua_source, Diagnostic, DiagnosticCode, ParsedSource};
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
        syntax: stylua_lib::LuaVersion::Tua,
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
    parse_tua_source(&parsed.path, &output).map_err(|_| {
        vec![Diagnostic::error(
            &parsed.path,
            1,
            1,
            "formatter output failed Tua syntax validation; source was left unchanged",
        )
        .with_code(DiagnosticCode::SYNTAX_PARSE)]
    })?;
    if preserved_payloads(&parsed.source) != preserved_payloads(&output) {
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

fn preserved_payloads(source: &str) -> Vec<(bool, Vec<u8>)> {
    use full_moon::{
        ast::LuaVersion,
        tokenizer::{Lexer, LexerResult, TokenReference, TokenType},
    };
    let LexerResult::Ok(tokens) = Lexer::new(source, LuaVersion::luau()).collect() else {
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
