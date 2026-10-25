//! A corpus that checks every formatter guarantee for each case at once:
//! the expected layout, a stable second pass, indentation rebuilt from
//! flush-left source, an unchanged compiled program, and output that passes
//! the statement-layout lints. Add a case whenever a formatter bug is fixed.

use full_moon::{
    ast::LuaVersion,
    tokenizer::{Lexer, LexerResult, StringLiteralQuoteType, TokenType},
};

use super::*;
use crate::{emit_tua_source, DiagnosticCode};

struct Case {
    name: &'static str,
    source: &'static str,
    expected: Option<&'static str>,
    options: FormatOptions,
    rebuilds_indentation: bool,
}

impl Case {
    fn new(name: &'static str, source: &'static str) -> Self {
        Self {
            name,
            source,
            expected: None,
            options: FormatOptions::default(),
            rebuilds_indentation: true,
        }
    }

    /// Without this, the source is expected to be canonical already.
    fn formats_to(mut self, expected: &'static str) -> Self {
        self.expected = Some(expected);
        self
    }

    fn with(mut self, configure: impl FnOnce(&mut FormatOptions)) -> Self {
        configure(&mut self.options);
        self
    }

    /// For sources whose indentation is part of a multiline literal or
    /// comment, which the formatter must keep rather than rebuild.
    fn keep_indentation(mut self) -> Self {
        self.rebuilds_indentation = false;
        self
    }
}

#[test]
fn formatter_corpus_holds_every_guarantee() {
    // Compiling and linting each case needs the CLI/LSP analysis stack.
    let failures = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| cases().iter().flat_map(check).collect::<Vec<_>>())
        .unwrap()
        .join()
        .unwrap();

    assert!(
        failures.is_empty(),
        "{} formatter corpus failure(s):\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

fn check(case: &Case) -> Vec<String> {
    let format = |source: &str| format_tua_source("src/main.tua", source, &case.options);
    let expected = case.expected.unwrap_or(case.source);
    let mut problems = Vec::new();

    match format(case.source) {
        Ok(output) => {
            if output != expected {
                problems.push(format!("expected:\n{expected}\ngot:\n{output}"));
            }
            match format(&output) {
                Ok(again) if again == output => {}
                Ok(again) => problems.push(format!("second pass changed the output:\n{again}")),
                Err(diagnostics) => {
                    problems.push(format!("output does not parse: {diagnostics:?}"));
                }
            }
            if let Err(problem) = same_program(case.source, &output) {
                problems.push(problem);
            }
            let lints = layout_lints(&output);
            if !lints.is_empty() {
                problems.push(format!("output triggers layout lints: {lints:?}"));
            }
        }
        Err(diagnostics) => problems.push(format!("source does not format: {diagnostics:?}")),
    }

    if case.rebuilds_indentation {
        let flush_left = case
            .source
            .split('\n')
            .map(|line| line.trim_start_matches([' ', '\t']))
            .collect::<Vec<_>>()
            .join("\n");
        match format(&flush_left) {
            Ok(rebuilt) if rebuilt == expected => {}
            Ok(rebuilt) => problems.push(format!(
                "indentation is not rebuilt from flush-left source:\n{rebuilt}"
            )),
            Err(diagnostics) => {
                problems.push(format!(
                    "flush-left source does not format: {diagnostics:?}"
                ));
            }
        }
    }

    problems
        .into_iter()
        .map(|problem| format!("{}: {problem}", case.name))
        .collect()
}

fn same_program(source: &str, output: &str) -> Result<(), String> {
    let before = program_tokens(source).map_err(|error| format!("source: {error}"))?;
    let after = program_tokens(output).map_err(|error| format!("output: {error}"))?;

    if before == after {
        return Ok(());
    }
    let index = before
        .iter()
        .zip(&after)
        .position(|(before, after)| before != after)
        .unwrap_or(before.len().min(after.len()));
    Err(format!(
        "formatting changed the compiled program at token {index}: {:?} became {:?}",
        before.get(index),
        after.get(index)
    ))
}

/// Compiles the source and returns the Lua tokens that carry meaning. String
/// literals compare by content, since quote styles may change delimiters,
/// and a separator before `}` is dropped, since wrapping adds one.
fn program_tokens(source: &str) -> Result<Vec<String>, String> {
    let lua = emit_tua_source("src/main.tua", source)
        .map_err(|diagnostics| format!("does not compile: {diagnostics:?}"))?;
    let tokens = match Lexer::new(&lua, LuaVersion::lua51()).collect() {
        LexerResult::Ok(tokens) => tokens,
        LexerResult::Recovered(_, errors) | LexerResult::Fatal(errors) => {
            return Err(format!("emitted Lua does not tokenize: {errors:?}"));
        }
    };
    let mut texts = Vec::<String>::new();

    for token in tokens
        .iter()
        .filter(|token| !token.token_type().is_trivia())
    {
        let text = match token.token_type() {
            TokenType::Eof => continue,
            TokenType::StringLiteral {
                literal,
                quote_type: StringLiteralQuoteType::Brackets,
                ..
            } => format!("[[{}]]", literal.replace("\r\n", "\n")),
            TokenType::StringLiteral { literal, .. } => {
                format!("\"{}\"", unescape_quotes(literal))
            }
            _ => token.to_string(),
        };
        if text == "}" && texts.last().is_some_and(|last| last == "," || last == ";") {
            texts.pop();
        }
        texts.push(text);
    }

    Ok(texts)
}

fn unescape_quotes(literal: &str) -> String {
    let mut output = String::with_capacity(literal.len());
    let mut characters = literal.chars();

    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        match characters.next() {
            Some(quote @ ('"' | '\'')) => output.push(quote),
            Some(escaped) => {
                output.push('\\');
                output.push(escaped);
            }
            None => output.push('\\'),
        }
    }

    output
}

fn layout_lints(output: &str) -> Vec<Diagnostic> {
    let Ok(parsed) = parse_tua_source("src/main.tua", output.to_owned()) else {
        return Vec::new();
    };
    let Ok(environment) = crate::types::build_type_environment(&parsed) else {
        return Vec::new();
    };

    crate::linter::lint_parsed_source(
        &parsed,
        &crate::DiagnosticsConfig::default(),
        &crate::config::JitConfig::default(),
        &environment,
    )
    .into_iter()
    .filter(|diagnostic| {
        matches!(
            diagnostic.code,
            Some(
                DiagnosticCode::LINT_SAME_LINE_STATEMENT
                    | DiagnosticCode::LINT_MULTI_LINE_STATEMENT
                    | DiagnosticCode::LINT_NEWLINE_CALL
                    | DiagnosticCode::LINT_NEWFIELD_CALL
            )
        )
    })
    .collect()
}

fn cases() -> Vec<Case> {
    vec![
        Case::new("comment_blocks_outer_table_wrap", "return {f(a, b), f --[[ note ]] (a, b)}\n")
            .formats_to("return { f(a, b), f --[[ note ]] (a, b)}\n")
            .with(|options| options.line_width = 15),
        Case::new("nested_comment_blocks_outer_table_wrap", "local x = {f(a, b), {a, --[[ note ]] b}} -- comment\n")
            .formats_to("local x = { f(a, b), { a, --[[ note ]] b}} -- comment\n")
            .with(|options| options.line_width = 25),
        Case::new("binary_minus_before_inline_table", "return a\n  - f({ b, c })\n"),
        Case::new("wrap_negative_table_value", "return {a, - n}\n")
            .formats_to("return {\n  a,\n  -n,\n}\n")
            .with(|options| options.line_width = 15),
        Case::new("wrap_first_negative_table_value", "return {- n, a}\n")
            .formats_to("return {\n  -n,\n  a,\n}\n")
            .with(|options| options.line_width = 15),
        Case::new("table_unary_and_binary_minus", "return {\n  -a,\n\n  -- Negative value after a comment.\n  -b;\n  a\n    - b,\n  value =\n    -c,\n}\n"),
        Case::new("nested_negative_table_value", "return outer({a, -b}, {c, -d})\n")
            .formats_to("return outer(\n  { a, -b },\n  { c, -d }\n)\n")
            .with(|options| options.line_width = 20),
        Case::new(
            "spacing_assign",
            r#"local x=1
print(x)
"#,
        )
        .formats_to(r#"local x = 1
print(x)
"#),
        Case::new(
            "spacing_arith",
            r#"local y=1+2*3-4/5%6^2
print(y)
"#,
        )
        .formats_to(r#"local y = 1 + 2 * 3 - 4 / 5 % 6 ^ 2
print(y)
"#),
        Case::new(
            "spacing_compare",
            r#"local a=1
local b=a==1 and a~=2 or a<=3 or a>=4 or a<5 or a>6
print(b)
"#,
        )
        .formats_to(r#"local a = 1
local b = a == 1 and a ~= 2 or a <= 3 or a >= 4 or a < 5 or a > 6
print(b)
"#),
        Case::new(
            "spacing_concat",
            r#"local s="a".."b"
print(s)
"#,
        )
        .formats_to(r#"local s = "a" .. "b"
print(s)
"#),
        Case::new(
            "spacing_concat_numbers",
            r#"local s=1 ..2
print(s)
"#,
        )
        .formats_to(r#"local s = 1 .. 2
print(s)
"#),
        Case::new(
            "spacing_comma",
            r#"print(1,2,3)
"#,
        )
        .formats_to(r#"print(1, 2, 3)
"#),
        Case::new(
            "spacing_varargs",
            r##"local function v(...)
  return select("#",...)
end
print(v())
"##,
        )
        .formats_to(r##"local function v(...)
  return select("#", ...)
end
print(v())
"##),
        Case::new(
            "spacing_table_literal",
            r#"local t={1,2,x=3}
print(t)
"#,
        )
        .formats_to(r#"local t = { 1, 2, x = 3 }
print(t)
"#),
        Case::new(
            "spacing_compound",
            r#"local x = 1
x+=1
x-=1
x*=2
x/=2
print(x)
"#,
        )
        .formats_to(r#"local x = 1
x += 1
x -= 1
x *= 2
x /= 2
print(x)
"#),
        Case::new(
            "spacing_sci_notation",
            r#"local e = 1e-5
local f = 2E+3
local g = 1.5e10
print(e, f, g)
"#,
        ),
        Case::new(
            "spacing_numbers",
            r#"local h = 0xFF
local d = .5
local t = 5.
print(h, d, t)
"#,
        ),
        Case::new(
            "spacing_field_method",
            r#"local t = { a = { b = {} } }
function t.a.b:c() return 1 end
print(t.a.b:c())
"#,
        ),
        Case::new(
            "spacing_index_spaces",
            r#"local t = {}
print(t[ 1 ], t [2])
"#,
        )
        .formats_to(r#"local t = {}
print(t[1], t[2])
"#),
        Case::new(
            "spacing_call_paren_spaces",
            r#"print( 1 )
print (2)
"#,
        )
        .formats_to(r#"print(1)
print(2)
"#),
        Case::new(
            "unary_minus_compact",
            r#"local n = 1
local m = - n
print(m)
"#,
        )
        .formats_to(
            r#"local n = 1
local m = -n
print(m)
"#,
        ),
        Case::new(
            "unary_minus_minus",
            r#"local a = 1
local b = a - -1
print(b)
"#,
        ),
        Case::new(
            "unary_minus_minus_messy",
            r#"local a = 1
local b = a- -1
print(b)
"#,
        )
        .formats_to(r#"local a = 1
local b = a - -1
print(b)
"#),
        Case::new(
            "unary_double_negation",
            r#"local c = - -1
print(c)
"#,
        ),
        Case::new(
            "unary_double_negation_paren",
            r#"local c = -(-1)
print(c)
"#,
        ),
        Case::new(
            "unary_not_len",
            r#"local s = "abc"
print(not true, #s, - #s)
"#,
        )
        .formats_to(
            r#"local s = "abc"
print(not true, #s, -#s)
"#,
        ),
        Case::new(
            "unary_minus_after_paren",
            r#"local a = 2
print((a)-1, a*-1, a^-1)
"#,
        )
        .formats_to(r#"local a = 2
print((a) - 1, a * -1, a ^ -1)
"#),
        Case::new(
            "string_operator_chars",
            r#"local s = "a=b+c,d..e"
local t = 'x==y'
print(s, t)
"#,
        ),
        Case::new(
            "string_escapes",
            r#"local s = "tab\tq\"\\n"
local t = 'it\'s'
print(s, t)
"#,
        ),
        Case::new(
            "string_keywords",
            r#"local function f()
  print("if x then")
  print("function end")
  print('while do repeat until')
end
f()
"#,
        ),
        Case::new(
            "string_comment_marker",
            r#"local s = "-- not a comment"
local t = "a" -- real
print(s, t)
"#,
        ),
        Case::new(
            "string_brackets",
            r#"local s = "({[ unbalanced"
local t = "]})"
print(s, t)
"#,
        ),
        Case::new(
            "string_unicode",
            r#"local s = "héllo — ü 漢字"
print(s)
"#,
        ),
        Case::new(
            "string_escaped_newline",
            r#"local s = "a\
  b"
print(s)
"#,
        )
        .keep_indentation(),
        Case::new(
            "string_call_syntax",
            r#"local m = require"string"
print"hi"
print(m)
"#,
        ),
        Case::new(
            "string_table_call_syntax",
            r#"local function f(t) return t end
print(f{1, 2})
"#,
        )
        .formats_to(
            r#"local function f(t) return t end
print(f{ 1, 2 })
"#,
        ),
        Case::new(
            "comment_operator_chars",
            r#"-- a=b+c,d
local x = 1 -- x=1,2
print(x)
"#,
        ),
        Case::new(
            "comment_keywords",
            r#"local function f()
  -- if this then do that end
  return 1 -- function
end
print(f())
"#,
        ),
        Case::new(
            "comment_block_comment",
            r#"--[[ a=b
  c+d
    if then ]]
local x = 1
print(x)
"#,
        )
        .keep_indentation(),
        Case::new(
            "comment_block_comment_inline",
            r#"local x = --[[ c=1 ]] 1
print(x)
"#,
        ),
        Case::new(
            "comment_block_level",
            r#"--[==[ ]] still comment ]==]
local x = 1
print(x)
"#,
        ),
        Case::new(
            "comment_indented_in_block",
            r#"local function f()
  -- explains
  local y = 1
  --[[ block
  ]]
  return y
end
print(f())
"#,
        )
        .keep_indentation(),
        Case::new(
            "comment_before_end",
            r#"local function f()
  local y = 1
  return y
  -- trailing note
end
print(f())
"#,
        ),
        Case::new(
            "comment_in_table",
            r#"local t = {
  -- first
  a = 1, -- one
  -- last
}
print(t)
"#,
        ),
        Case::new(
            "comment_between_if_then",
            r#"local a = 1
if a == 1 -- check
then
  print(a)
end
"#,
        ),
        Case::new(
            "long_string_contents",
            "local s = [[\n a=b\n    c  \n]]\nprint(s)\n",
        )
        .keep_indentation(),
        Case::new(
            "long_string_level",
            r#"local s = [==[ ]] a=b ]==]
print(s)
"#,
        ),
        Case::new(
            "long_string_keywords_inside_function",
            r#"local function f()
  return [[
function x()
  if y then
end
]]
end
print(f())
"#,
        )
        .keep_indentation(),
        Case::new(
            "long_string_in_call",
            r#"print([[
line one
  line two]], 1)
"#,
        )
        .keep_indentation(),
        Case::new(
            "quote_double",
            r#"local s = 'plain'
local t = 'say "hi"'
local u = 'it\'s'
print(s, t, u)
"#,
        )
        .formats_to(r#"local s = "plain"
local t = "say \"hi\""
local u = "it's"
print(s, t, u)
"#)
        .with(|options| options.quote_style = QuoteStyle::Double),
        Case::new(
            "quote_single",
            r#"local s = "plain"
local t = "it's"
local u = "say \"hi\""
print(s, t, u)
"#,
        )
        .formats_to(r#"local s = 'plain'
local t = 'it\'s'
local u = 'say "hi"'
print(s, t, u)
"#)
        .with(|options| options.quote_style = QuoteStyle::Single),
        Case::new(
            "quote_double_escapes",
            r#"local s = 'a\\b\n\''
print(s)
"#,
        )
        .formats_to(r#"local s = "a\\b\n'"
print(s)
"#)
        .with(|options| options.quote_style = QuoteStyle::Double),
        Case::new(
            "quote_preserve_long",
            r#"local s = [[it's "x"]]
print(s)
"#,
        )
        .with(|options| options.quote_style = QuoteStyle::Double),
        Case::new(
            "blank_lines_collapse",
            r#"local a = 1



local b = 2
print(a, b)
"#,
        )
        .formats_to(r#"local a = 1

local b = 2
print(a, b)
"#),
        Case::new(
            "blank_lines_leading",
            r#"

local a = 1
print(a)
"#,
        )
        .formats_to(r#"local a = 1
print(a)
"#),
        Case::new(
            "blank_lines_trailing",
            r#"local a = 1
print(a)


"#,
        )
        .formats_to(r#"local a = 1
print(a)
"#),
        Case::new(
            "blank_lines_after_block_open",
            r#"local function f()

  return 1

end
print(f())
"#,
        ),
        Case::new(
            "whitespace_trailing",
            "local a = 1   \nprint(a)\t\n",
        )
        .formats_to(r#"local a = 1
print(a)
"#),
        Case::new(
            "whitespace_final_newline",
            r#"local a = 1
print(a)"#,
        )
        .formats_to(r#"local a = 1
print(a)
"#),
        Case::new(
            "whitespace_no_final_newline_opt",
            r#"local a = 1
print(a)
"#,
        )
        .formats_to(r#"local a = 1
print(a)"#)
        .with(|options| options.final_newline = false),
        Case::new(
            "whitespace_semicolons",
            r#"local a = 1;
local b = 2; print(a, b)
"#,
        ),
        Case::new(
            "line_ending_crlf_to_lf",
            "local a = 1\r\nprint(a)\r\n",
        )
        .formats_to(r#"local a = 1
print(a)
"#),
        Case::new(
            "line_ending_crlf_opt",
            r#"local a = 1
print(a)
"#,
        )
        .formats_to("local a = 1\r\nprint(a)\r\n")
        .with(|options| options.line_ending = LineEnding::Crlf),
        Case::new(
            "line_ending_preserve_crlf",
            "local a = 1\r\nprint(a)\r\n",
        )
        .with(|options| options.line_ending = LineEnding::Preserve),
        Case::new(
            "line_ending_crlf_long_string",
            "local s = [[a\r\nb]]\r\nprint(s)\r\n",
        )
        .formats_to(r#"local s = [[a
b]]
print(s)
"#)
        .keep_indentation(),
        Case::new(
            "indent_tabs",
            "local function f()\n\tif true then\n\t\treturn 1\n\tend\nend\nprint(f())\n",
        )
        .with(|options| options.indent_style = IndentStyle::Tabs),
        Case::new(
            "indent_width4",
            r#"local function f()
    if true then
        return 1
    end
end
print(f())
"#,
        )
        .with(|options| options.indent_width = 4),
        Case::new(
            "wrap_long_call",
            r#"local alpha_value_one, beta_value_two, gamma_value_three = 1, 2, 3
print(alpha_value_one, beta_value_two, gamma_value_three)
"#,
        )
        .formats_to(r#"local alpha_value_one, beta_value_two, gamma_value_three = 1, 2, 3
print(
  alpha_value_one,
  beta_value_two,
  gamma_value_three
)
"#)
        .with(|options| options.line_width = 40),
        Case::new(
            "wrap_string_with_commas",
            r#"print("a, b, c, d, e, f, g, h, i, j, k, l", 1)
"#,
        )
        .formats_to(r#"print(
  "a, b, c, d, e, f, g, h, i, j, k, l",
  1
)
"#)
        .with(|options| options.line_width = 30),
        Case::new(
            "wrap_long_table",
            r#"local t = { alpha = 1, beta = 2, gamma = 3, delta = 4 }
print(t)
"#,
        )
        .formats_to(r#"local t = {
  alpha = 1,
  beta = 2,
  gamma = 3,
  delta = 4,
}
print(t)
"#)
        .with(|options| options.line_width = 40),
        Case::new(
            "wrap_nested_call",
            r#"local first_value, second_value, third_value = 1, 2, 3
print(string.format("%s %s %s", first_value, second_value), third_value)
"#,
        )
        .formats_to(r#"local first_value, second_value, third_value = 1, 2, 3
print(
  string.format(
    "%s %s %s",
    first_value,
    second_value
  ),
  third_value
)
"#)
        .with(|options| options.line_width = 50),
        Case::new(
            "wrap_trailing_comment",
            r#"local alpha_value_one, beta_value_two, gamma_value_three = 1, 2, 3
print(alpha_value_one, beta_value_two, gamma_value_three) -- note
"#,
        )
        .formats_to(r#"local alpha_value_one, beta_value_two, gamma_value_three = 1, 2, 3
print(
  alpha_value_one,
  beta_value_two,
  gamma_value_three
) -- note
"#)
        .with(|options| options.line_width = 40),
        Case::new(
            "wrap_callback_arg",
            r#"local function each(t, f) end
each({ 1, 2, 3 }, function(value) print(value, value, value) end)
"#,
        )
        .formats_to(r#"local function each(t, f) end
each(
  { 1, 2, 3 },
  function(value) print(value, value, value) end
)
"#)
        .with(|options| options.line_width = 40),
        Case::new(
            "wrap_unicode_width",
            r#"print("ééééééééééééé", "üüüüüüüü")
"#,
        )
        .with(|options| options.line_width = 40),
        Case::new(
            "function_table_nested_table",
            r#"local t = {
  a = {
    b = { 1, 2 },
  },
  c = function()
    return 1
  end,
}
print(t)
"#,
        ),
        Case::new(
            "function_table_table_in_call",
            r#"print({
  a = 1,
}, 2)
"#,
        ),
        Case::new(
            "function_table_callbacks",
            r#"local function on(name, f) f() end
on("a", function()
  on("b", function()
    print("x")
  end)
end)
"#,
        ),
        Case::new(
            "function_table_return_function",
            r#"local function make()
  return function()
    return 1
  end
end
print(make()())
"#,
        ),
        Case::new(
            "function_table_one_line_function",
            r#"local function f() return 1 end
print(f())
"#,
        ),
        Case::new(
            "function_table_one_line_if",
            r#"local a = 1
if a then print(a) end
"#,
        ),
        Case::new(
            "function_table_method_def",
            r#"local M = {}
function M:greet(name)
  return self, name
end
print(M:greet("x"))
"#,
        ),
        Case::new(
            "function_table_empty_function",
            r#"local function f()
end
f()
"#,
        ),
        Case::new(
            "function_table_paren_expr",
            r#"local a, b = nil, { x = 1 }
print((a or b).x)
"#,
        ),
        Case::new(
            "control_flow_if_chain",
            r#"local a = 1
if a == 1 then
  print(1)
elseif a == 2 then
  print(2)
else
  print(3)
end
"#,
        ),
        Case::new(
            "control_flow_loops",
            r#"for i = 1, 3 do
  print(i)
end
for k, v in pairs({}) do
  print(k, v)
end
local n = 0
while n < 2 do
  n = n + 1
end
repeat
  n = n - 1
until n == 0
do
  local scoped = 1
  print(scoped)
end
"#,
        ),
        Case::new(
            "control_flow_nested_deep",
            r#"local function f(a)
  if a then
    for i = 1, 2 do
      while a do
        repeat
          a = false
        until true
      end
    end
  end
end
f(true)
"#,
        ),
        Case::new(
            "control_flow_break_return",
            r#"for i = 1, 3 do
  if i == 2 then
    break
  end
end
local function f()
  do
    return 1
  end
end
print(f())
"#,
        ),
        Case::new(
            "tua_annotations",
            r#"local score: number = 0
local title: string = "Arena"
local values: (number | string)[] = {}
print(score, title, values)
"#,
        ),
        Case::new(
            "tua_annotation_messy",
            r#"local score:number=0
print(score)
"#,
        )
        .formats_to(r#"local score:number = 0
print(score)
"#),
        Case::new(
            "tua_type_table",
            r#"type Player = {
  name: string,
  hp: number,
}
local p: Player = { name = "a", hp = 1 }
print(p)
"#,
        ),
        Case::new(
            "tua_function_type",
            r#"type Predicate = (value: number) -> boolean
local check: Predicate = function(value: number): boolean
  return value > 0
end
print(check(1))
"#,
        ),
        Case::new(
            "tua_arrow_messy",
            r#"type Predicate = (value: number)->boolean
local p: Predicate = function(v: number): boolean return v > 0 end
print(p(1))
"#,
        )
        .formats_to(r#"type Predicate = (value: number) -> boolean
local p: Predicate = function(v: number): boolean return v > 0 end
print(p(1))
"#),
        Case::new(
            "tua_enum",
            r#"enum MovementState
  "idle"
  "run"
end
local m: MovementState = "idle"
print(m)
"#,
        ),
        Case::new(
            "tua_class",
            r#"open class Entity {
  x: number

  init(x: number) {
    self.x = x
  }

  update(dt: number) {
    self.x += dt
  }
}

local e = Entity.new(1)
e:update(0.5)
"#,
        ),
        Case::new(
            "tua_class_extends",
            r#"open class Base {
  init() {
  }
}

final class Child extends Base {
  speed: number = 120

  init() {
    super()
  }
}

print(Child.new())
"#,
        ),
        Case::new(
            "tua_default_param",
            r#"local function heal(amount: number = 5)
  return amount
end
print(heal())
"#,
        ),
        Case::new(
            "tua_multiline_params",
            r#"local function move(
  x: number,
  y: number
)
  return x + y
end
print(move(1, 2))
"#,
        ),
        Case::new(
            "keyword_identifiers",
            r#"local endpoint, doit, ending, functional, iffy = 1, 2, 3, 4, 5
local t = { ["end"] = 1, then_value = 2, do_thing = 3 }
print(endpoint, doit, ending, functional, iffy, t["end"], t.then_value, t.do_thing)
"#,
        ),
        Case::new(
            "keyword_field_names",
            r#"local t = { repeat_count = 1, until_done = 2, elseif_case = 3 }
local function f()
  return t.repeat_count + t.until_done + t.elseif_case
end
print(f())
"#,
        ),
        Case::new(
            "mixed_string_block_comment_marker",
            r#"local s = "--[["
local function f()
  return s
end
print(f())
"#,
        ),
        Case::new(
            "mixed_string_long_marker",
            r#"local s = "[["
local function f()
  return s
end
print(f())
"#,
        ),
        Case::new(
            "mixed_comment_close_bracket",
            r#"-- ]] not special
local function f()
  return 1
end
print(f())
"#,
        ),
        Case::new(
            "mixed_single_quote_backslash_end",
            r#"local s = 'a\\'
local function f()
  return s
end
print(f())
"#,
        ),
        Case::new(
            "mixed_concat_negative",
            r#"local a = 1
print(a .. -1)
"#,
        ),
        Case::new(
            "mixed_mixed_tabs_spaces",
            "local function f()\n\t  if true then\n  \t\treturn 1\n\tend\nend\nprint(f())\n",
        )
        .formats_to(r#"local function f()
  if true then
    return 1
  end
end
print(f())
"#)
        .keep_indentation(),
        Case::new(
            "mixed_tab_width_counts",
            "local function f(alpha, beta)\n\tprint(alpha, beta, alpha, beta)\nend\nf(1, 2)\n",
        )
        .with(|options| options.indent_style = IndentStyle::Tabs)
        .with(|options| options.line_width = 40),
        Case::new(
            "mixed_long_params",
            r#"local function configure(first_option, second_option, third_option)
  return first_option, second_option, third_option
end
print(configure(1, 2, 3))
"#,
        )
        .formats_to(r#"local function configure(
  first_option,
  second_option,
  third_option
)
  return first_option, second_option, third_option
end
print(configure(1, 2, 3))
"#)
        .with(|options| options.line_width = 50),
        Case::new(
            "mixed_long_method_call",
            r#"local first_argument_value, second_argument_value = 1, 2
local obj = { run = function(self, a, b, c) return a end }
print(obj:run(first_argument_value, second_argument_value, 3))
"#,
        )
        .with(|options| options.line_width = 50),
        Case::new(
            "mixed_long_string_arg",
            r#"print("this single string argument is longer than the configured width")
"#,
        )
        .with(|options| options.line_width = 30),
        Case::new(
            "mixed_only_semicolon",
            r#"local a = 1
;
print(a)
"#,
        ),
        Case::new(
            "mixed_return_table",
            r#"local function f()
  return {
    a = 1,
  }
end
print(f())
"#,
        ),
        Case::new(
            "mixed_table_of_functions",
            r#"local handlers = {
  load = function()
    return 1
  end,
  save = function()
    return 2
  end,
}
print(handlers)
"#,
        ),
        Case::new(
            "mixed_closing_paren_alone",
            r#"print(
  1,
  2
)
"#,
        ),
        Case::new(
            "mixed_closing_brace_paren",
            r#"print({
  a = 1,
})
"#,
        ),
        Case::new(
            "mixed_index_assign",
            r#"local t = {}
t["key"] = 1
t[1] = 2
print(t)
"#,
        ),
        Case::new(
            "mixed_string_keys_in_table",
            r#"local t = { ["a b"] = 1, ["c"] = 2 }
print(t)
"#,
        ),
        Case::new(
            "mixed_long_concat",
            r#"local s = "aaaaaaaaaaaaaaaaaaaa" .. "bbbbbbbbbbbbbbbbbbbbbbbb" .. "cccccccccccccccccc"
print(s)
"#,
        )
        .with(|options| options.line_width = 40),
        Case::new(
            "mixed_elseif_one_line",
            r#"local a = 1
if a then print(1) elseif a then print(2) else print(3) end
"#,
        ),
        Case::new(
            "mixed_function_in_table_call",
            r#"local function g(t) return t end
print(g({
  cb = function()
    return 1
  end,
}))
"#,
        ),
        Case::new(
            "mixed_deep_parens",
            r#"local a = 1
print(((a + 1) * (a - 1)) / ((a)))
"#,
        ),
        Case::new(
            "mixed_require_multiline",
            r#"local m = require(
  "string"
)
print(m)
"#,
        ),
        Case::new(
            "mixed_vararg_table",
            r#"local function f(...)
  local t = { ... }
  return #t
end
print(f(1, 2))
"#,
        ),
        Case::new(
            "mixed_class_method_body",
            r#"class Counter {
  count: number = 0

  init() {
  }

  bump(by: number) {
    if by > 0 then
      self.count += by
    end
  }
}

local c = Counter.new()
c:bump(1)
"#,
        ),
        Case::new(
            "mixed_class_table_field",
            r#"class Bag {
  items: number[] = {}

  init() {
    self.items = { 1, 2 }
  }
}

print(Bag.new())
"#,
        ),
        Case::new("mixed_byte_order_mark", "\u{feff}local a = 1\nprint(a)\n")
            .formats_to("local a = 1\nprint(a)\n"),
        Case::new("comment_touching_code", "local a = 1-- one\nprint(a)--[[ two ]]\n")
            .formats_to("local a = 1 -- one\nprint(a) --[[ two ]]\n"),
        Case::new(
            "wrap_hugged_nested_call",
            "local function f(...) return ... end\nprint(f(\n  1,\n  2\n))\n",
        ),
    ]
}
