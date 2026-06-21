use std::path::Path;

use crate::lua;
use crate::module_index::{
    required_module_export_type_for_fields, required_module_for_receiver, resolve_required_module,
};
use crate::module_link::require_module_at_position;
use crate::semantic::resolve_field_chain_type;
use crate::source_text::dotted_chain_at_position;
use crate::types::{build_type_environment, FunctionType, Type, TypeEnvironment, TypedFunction};
use crate::{parse_tua_source, Diagnostic, ParsedSource, SourcePosition, SourceSpan};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hover {
    pub contents: String,
    pub span: Option<SourceSpan>,
}

pub fn hover_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    position: SourcePosition,
) -> Result<Option<Hover>, Vec<Diagnostic>> {
    let parsed = parse_tua_source(path, source)?;

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
        if let Some(hover) = self.required_module_literal_hover(position) {
            return Some(hover);
        }

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

            if let Some(hover) = self.required_module_field_hover(&field_parts, current_part.span) {
                return Some(hover);
            }

            if self.environment.local(&field_parts[0]).is_none() {
                if let Some(item) = lua::hover_item(&field_parts) {
                    return Some(Hover {
                        contents: item.detail.to_owned(),
                        span: Some(current_part.span),
                    });
                }
            }
        }

        if let Some(hover) =
            self.required_module_binding_hover(&current_part.text, current_part.span)
        {
            return Some(hover);
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
            .or_else(|| {
                lua::hover_item(std::slice::from_ref(&current_part.text)).map(|item| Hover {
                    contents: item.detail.to_owned(),
                    span: Some(current_part.span),
                })
            })
    }

    fn resolve_field_chain_type(&self, parts: &[String]) -> Option<Type> {
        resolve_field_chain_type(self.environment, parts)
    }

    fn required_module_literal_hover(&self, position: SourcePosition) -> Option<Hover> {
        let module = require_module_at_position(&self.parsed.source, position)?;
        let path = resolve_required_module(&self.parsed.path, &module)?;

        Some(Hover {
            contents: format!("(module) {module} -> {}", path.display()),
            span: None,
        })
    }

    fn required_module_binding_hover(&self, receiver: &str, span: SourceSpan) -> Option<Hover> {
        let required = required_module_for_receiver(self.parsed, receiver)?;

        Some(Hover {
            contents: format!(
                "(module) {receiver} = require(\"{}\") -> {}",
                required.module,
                required.path.display()
            ),
            span: Some(span),
        })
    }

    fn required_module_field_hover(&self, parts: &[String], span: SourceSpan) -> Option<Hover> {
        let (receiver, fields) = parts.split_first()?;
        let ty = required_module_export_type_for_fields(self.parsed, receiver, fields)?;

        Some(Hover {
            contents: format_export_hover(parts, &ty),
            span: Some(span),
        })
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

fn format_export_hover(parts: &[String], ty: &Type) -> String {
    let qualified_name = parts.join(".");

    match ty {
        Type::Function(function) => format_export_function(&qualified_name, function),
        _ => format!("(field) {qualified_name}: {ty}"),
    }
}

fn format_export_function(name: &str, function: &FunctionType) -> String {
    let parameters = function
        .parameters
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    format!("function {name}({parameters}): {}", function.return_type)
}

#[cfg(test)]
mod tests;
