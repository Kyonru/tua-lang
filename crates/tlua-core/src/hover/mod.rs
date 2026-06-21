use std::path::Path;

use crate::semantic::resolve_field_chain_type;
use crate::source_text::dotted_chain_at_position;
use crate::types::{build_type_environment, Type, TypeEnvironment, TypedFunction};
use crate::{parse_tlua_source, Diagnostic, ParsedSource, SourcePosition, SourceSpan};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hover {
    pub contents: String,
    pub span: Option<SourceSpan>,
}

pub fn hover_tlua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    position: SourcePosition,
) -> Result<Option<Hover>, Vec<Diagnostic>> {
    let parsed = parse_tlua_source(path, source)?;

    hover_parsed_source(&parsed, position)
}

pub fn hover_parsed_source(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Result<Option<Hover>, Vec<Diagnostic>> {
    let environment = build_type_environment(parsed)?;
    let resolver = HoverResolver {
        parsed,
        environment: &environment,
    };

    Ok(resolver.hover(position))
}

struct HoverResolver<'a> {
    parsed: &'a ParsedSource,
    environment: &'a TypeEnvironment,
}

impl<'a> HoverResolver<'a> {
    fn hover(&self, position: SourcePosition) -> Option<Hover> {
        let chain = dotted_chain_at_position(&self.parsed.source, position)?;
        let current_part = chain.parts.get(chain.cursor_index)?;

        if chain.cursor_index > 0 {
            let field_parts = chain
                .parts
                .iter()
                .take(chain.cursor_index + 1)
                .map(|part| part.text.clone())
                .collect::<Vec<_>>();

            if let Some(ty) = self.resolve_field_chain_type(&field_parts) {
                return Some(Hover {
                    contents: format!("(field) {}: {ty}", current_part.text),
                    span: Some(current_part.span),
                });
            }
        }

        if let Some(ty) = self.environment.alias(&current_part.text) {
            return Some(Hover {
                contents: format!("type {} = {ty}", current_part.text),
                span: Some(current_part.span),
            });
        }

        if let Some(ty) = self.environment.local(&current_part.text) {
            return Some(Hover {
                contents: format!("local {}: {ty}", current_part.text),
                span: Some(current_part.span),
            });
        }

        self.environment
            .function(&current_part.text)
            .map(|function| Hover {
                contents: format_function(function),
                span: Some(current_part.span),
            })
    }

    fn resolve_field_chain_type(&self, parts: &[String]) -> Option<Type> {
        resolve_field_chain_type(self.environment, parts)
    }
}

fn format_function(function: &TypedFunction) -> String {
    let name = function.name.as_deref().unwrap_or("<anonymous>");
    let mut parameters = function
        .parameters
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name, parameter.ty))
        .collect::<Vec<_>>();

    if function.has_varargs {
        parameters.push("...".to_owned());
    }

    format!(
        "function {name}({}): {}",
        parameters.join(", "),
        function.return_type_or_any()
    )
}

#[cfg(test)]
mod tests;
