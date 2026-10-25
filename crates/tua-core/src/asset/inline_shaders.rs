//! Literal long-string shader sources and their original-source coordinates.
use super::*;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct InlineShader {
    pub call_span: SourceSpan,
    pub origin: SourcePosition,
    pub source: ShaderSource,
}

pub(super) fn inline_shader_token(call: &FunctionCall) -> Option<&TokenReference> {
    let (parts, args) = field_chain_before_first_call(call)?;
    if !parts_match(&parts, &["love", "graphics", "newShader"]) {
        return None;
    }
    let token = match args {
        Call::AnonymousCall(FunctionArgs::String(token)) => token,
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            let Expression::String(token) = arguments.iter().next()? else {
                return None;
            };
            token
        }
        _ => return None,
    };
    use full_moon::tokenizer::{StringLiteralQuoteType, TokenType};
    let TokenType::StringLiteral {
        literal,
        quote_type: StringLiteralQuoteType::Brackets,
        ..
    } = token.token().token_type()
    else {
        return None;
    };
    let body = literal.as_str();
    if !body.contains(['\n', '\r', ';', '{', '}']) && looks_like_asset_path(body) {
        return None;
    }
    Some(token)
}

pub(crate) fn inline_shader_call(call: &FunctionCall) -> Option<InlineShader> {
    let token = inline_shader_token(call)?;
    if call.suffixes().any(|suffix| matches!(suffix, Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })) if arguments.iter().count() != 1)) { return None; }
    let raw = token.token().to_string();
    let open = raw[1..].find('[')? + 2;
    let body = raw.get(open..raw.len().checked_sub(open)?)?;
    let start = node_span(token)?.start;
    let leading_newline = if body.starts_with("\r\n") || body.starts_with("\n\r") {
        2
    } else if body.starts_with(['\n', '\r']) {
        1
    } else {
        0
    };
    let origin = if leading_newline > 0 {
        SourcePosition {
            line: start.line + 1,
            column: 1,
        }
    } else {
        SourcePosition {
            line: start.line,
            column: start.column + open,
        }
    };
    let body = body[leading_newline..]
        .replace("\r\n", "\n")
        .replace("\n\r", "\n")
        .replace('\r', "\n");
    let mut source = ShaderSource::from_source(body, None)?;
    if let Some(model) = &mut source.uniforms {
        for uniform in Arc::make_mut(model).uniforms.values_mut() {
            for span in &mut uniform.declarations {
                span.start = map_position(origin, span.start);
                span.end = map_position(origin, span.end);
            }
        }
    }
    Some(InlineShader {
        call_span: node_span(call)?,
        origin,
        source,
    })
}

pub(crate) fn map_position(origin: SourcePosition, position: SourcePosition) -> SourcePosition {
    SourcePosition {
        line: origin.line + position.line.saturating_sub(1),
        column: position.column
            + if position.line == 1 {
                origin.column - 1
            } else {
                0
            },
    }
}

pub(crate) fn inline_shaders(parsed: &ParsedSource) -> Vec<InlineShader> {
    #[derive(Default)]
    struct Collect(Vec<InlineShader>);
    impl Visitor for Collect {
        fn visit_function_call(&mut self, call: &FunctionCall) {
            if let Some(shader) = inline_shader_call(call) {
                self.0.push(shader);
            }
        }
    }
    let mut collect = Collect::default();
    collect.visit_ast(&parsed.ast);
    collect.0
}
