//! Keeps top-level LÖVE frame callbacks in the order `love.run` calls them.

use full_moon::ast::Stmt;

use super::{node_span, token_text};
use crate::ast_utils::plain_field_chain_from_var;
use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity, ParsedSource, SourceSpan};

/// `love.run` calls `love.load` once, then `love.update` and `love.draw` every frame.
const CALLBACK_ORDER: [&str; 3] = ["load", "update", "draw"];

pub(super) fn lint(parsed: &ParsedSource) -> Vec<Diagnostic> {
    // Only top-level definitions: callbacks installed conditionally or inside
    // functions (scene switches, for example) have no meaningful file order.
    let mut definitions: Vec<(usize, SourceSpan)> = Vec::new();
    for stmt in parsed.ast.nodes().stmts() {
        let targets: Vec<_> = match stmt {
            Stmt::FunctionDeclaration(function) if function.name().method_name().is_none() => {
                let names: Vec<_> = function.name().names().iter().collect();
                match names.as_slice() {
                    [love, callback] if token_text(love) == "love" => {
                        vec![(token_text(callback), node_span(function.name()))]
                    }
                    _ => Vec::new(),
                }
            }
            Stmt::Assignment(assignment) => assignment
                .variables()
                .iter()
                .filter_map(
                    |variable| match plain_field_chain_from_var(variable)?.as_slice() {
                        [love, callback] if love == "love" => {
                            Some((callback.clone(), node_span(variable)))
                        }
                        _ => None,
                    },
                )
                .collect(),
            _ => Vec::new(),
        };
        for (callback, span) in targets {
            let Some(rank) = CALLBACK_ORDER.iter().position(|name| *name == callback) else {
                continue;
            };
            // A later redefinition replaces the callback; the first one sets its place.
            if let Some(span) = span.filter(|_| definitions.iter().all(|(seen, _)| *seen != rank)) {
                definitions.push((rank, span));
            }
        }
    }

    definitions
        .iter()
        .enumerate()
        .filter_map(|(index, (rank, span))| {
            let (earlier, _) = definitions[..index]
                .iter()
                .find(|(earlier, _)| earlier > rank)?;
            Some(callback_order_diagnostic(
                parsed,
                *span,
                CALLBACK_ORDER[*rank],
                CALLBACK_ORDER[*earlier],
            ))
        })
        .collect()
}

fn callback_order_diagnostic(
    parsed: &ParsedSource,
    span: SourceSpan,
    callback: &str,
    defined_before: &str,
) -> Diagnostic {
    Diagnostic::new(
        &parsed.path,
        span.start.line,
        span.start.column,
        DiagnosticSeverity::Warning,
        format!(
            "`love.{callback}` is defined after `love.{defined_before}`; define LÖVE callbacks in the order they run: `love.load`, `love.update`, `love.draw`"
        ),
    )
    .with_range(
        span.start.line,
        span.start.column,
        span.end.line,
        span.end.column,
    )
    .with_code(DiagnosticCode::LINT_LOVE_CALLBACK_ORDER)
}
