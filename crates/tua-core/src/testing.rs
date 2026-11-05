//! Static Busted discovery. Never executes spec files or guesses dynamic names.
use full_moon::ast::{Call, Expression, FunctionArgs, FunctionCall, Prefix, Suffix};
use full_moon::node::Node;
use full_moon::tokenizer::TokenType;
use full_moon::visitors::Visitor;

use crate::{ParsedSource, SourcePosition};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct TestCase {
    pub name: String,
    pub full_name: String,
    pub line: usize,
    pub column: usize,
    pub pending: bool,
}

struct Registration {
    name: Option<String>,
    kind: String,
    start: SourcePosition,
    end: SourcePosition,
}

/// Literal, unshadowed `describe`/`context` and `it`/`pending` registrations.
/// Dynamic registrations remain runnable at file level.
pub fn discover_test_cases(parsed: &ParsedSource) -> Vec<TestCase> {
    struct Calls<'a> {
        parsed: &'a ParsedSource,
        scope: crate::ScopeMap,
        registrations: Vec<Registration>,
    }
    impl Visitor for Calls<'_> {
        fn visit_function_call(&mut self, call: &FunctionCall) {
            let Prefix::Name(token) = call.prefix() else {
                return;
            };
            let kind = crate::ast_utils::token_text(token);
            if !matches!(kind.as_str(), "describe" | "context" | "it" | "pending")
                || call.suffixes().count() != 1
            {
                return;
            }
            let Some((start, end)) = call.range() else {
                return;
            };
            let start = SourcePosition::from_full_moon(start);
            if self.scope.has_non_builtin_binding_at(&kind, start) {
                return;
            }
            let Some(Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses {
                arguments,
                ..
            }))) = call.suffixes().next()
            else {
                return;
            };
            let name = arguments.iter().next().and_then(|expression| {
                let Expression::String(token) = expression else {
                    return None;
                };
                let TokenType::StringLiteral { literal, .. } = token.token().token_type() else {
                    return None;
                };
                let name = literal.to_string();
                // Until escape/newline decoding is shared with runtime semantics,
                // retain file-level execution rather than invent an exact filter.
                (!name.is_empty() && !name.contains(['\\', '\r', '\n'])).then_some(name)
            });
            let Some(origin) = self.parsed.raw.parser_position_origin(start) else {
                return;
            };
            let Some(end) = self
                .parsed
                .raw
                .parser_position_origin(SourcePosition::from_full_moon(end))
            else {
                return;
            };
            self.registrations.push(Registration {
                name,
                kind,
                start: origin.source_position,
                end: end.source_position,
            });
        }
    }
    let mut calls = Calls {
        parsed,
        scope: crate::scope_map_parsed_source(parsed),
        registrations: Vec::new(),
    };
    calls.visit_ast(&parsed.ast);
    let key = |p: SourcePosition| (p.line, p.column);
    calls.registrations.sort_by_key(|entry| key(entry.start));
    let mut parents: Vec<&Registration> = Vec::new();
    let mut cases = Vec::new();
    for entry in &calls.registrations {
        while parents
            .last()
            .is_some_and(|parent| key(parent.end) <= key(entry.start))
        {
            parents.pop();
        }
        if matches!(entry.kind.as_str(), "describe" | "context") {
            parents.push(entry);
            continue;
        }
        let Some(name) = &entry.name else { continue };
        let Some(mut names) = parents
            .iter()
            .map(|parent| parent.name.as_deref())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        names.push(name);
        cases.push(TestCase {
            name: name.clone(),
            full_name: names.join(" "),
            line: entry.start.line,
            column: entry.start.column,
            pending: entry.kind == "pending",
        });
    }
    cases
}

#[cfg(test)]
mod tests {
    #[test]
    fn discovers_static_nested_cases_without_executing_or_guessing_dynamic_names() {
        let source = r#"describe("math #unit", function()
  context('positive', function()
    it('works [1]', function() error('must not execute') end)
    pending('later')
  end)
end)
describe(dynamic_name, function() it('unknown parent', function() end) end)
local it = function(...) end
it('shadow', function() end)
"#;
        let parsed = crate::parse_tua_source("spec.tua", source).unwrap();
        let cases = super::discover_test_cases(&parsed);
        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0].full_name, "math #unit positive works [1]");
        assert_eq!(cases[0].line, 3);
        assert!(cases[1].pending);
    }
}
