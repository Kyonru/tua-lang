//! Static Busted discovery. Never executes spec files or guesses dynamic names.
use full_moon::ast::{Call, Expression, FunctionArgs, FunctionCall, Prefix, Suffix};
use full_moon::node::Node;
use full_moon::visitors::Visitor;

use crate::{ParsedSource, SourcePosition};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct TestCase {
    pub name: String,
    pub full_name: String,
    pub line: usize,
    pub column: usize,
    pub pending: bool,
    pub suite: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct TestSuite {
    pub name: String,
    pub full_name: String,
    pub parent: Option<String>,
    pub line: usize,
    pub column: usize,
}

#[derive(Default)]
pub struct TestDiscovery {
    pub cases: Vec<TestCase>,
    pub suites: Vec<TestSuite>,
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
    discover_tests(parsed).cases
}

/// Discover suite and case locations together from the same registration tree.
pub fn discover_tests(parsed: &ParsedSource) -> TestDiscovery {
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
                let name =
                    String::from_utf8(crate::ast_utils::string_literal_runtime_bytes(token)?)
                        .ok()?;
                (!name.is_empty()).then_some(name)
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
    let mut discovery = TestDiscovery::default();
    for entry in &calls.registrations {
        while parents
            .last()
            .is_some_and(|parent| key(parent.end) <= key(entry.start))
        {
            parents.pop();
        }
        let names = parents
            .iter()
            .map(|parent| parent.name.as_deref())
            .collect::<Option<Vec<_>>>();
        let is_suite = matches!(entry.kind.as_str(), "describe" | "context");
        if let (Some(name), Some(mut names)) = (&entry.name, names) {
            let parent = (!names.is_empty()).then(|| names.join(" "));
            names.push(name);
            let full_name = names.join(" ");
            if is_suite {
                discovery.suites.push(TestSuite {
                    name: name.clone(),
                    full_name,
                    parent,
                    line: entry.start.line,
                    column: entry.start.column,
                });
            } else {
                discovery.cases.push(TestCase {
                    name: name.clone(),
                    full_name,
                    line: entry.start.line,
                    column: entry.start.column,
                    pending: entry.kind == "pending",
                    suite: parent,
                });
            }
        }
        // Unknown suite names must still hide descendants whose runtime full
        // names cannot be established without executing the spec.
        if is_suite {
            parents.push(entry);
        }
    }
    discovery
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

    #[test]
    fn discovers_suite_tree_locations_and_keeps_dynamic_children_runnable_by_suite() {
        let source = r#"describe("outer [x]", function()
  context("nested", function()
    it("literal", function() end)
    it(dynamic_name, function() end)
  end)
end)
describe("empty", function() end)
describe(dynamic_name, function()
  describe("unknown parent", function() end)
end)
local describe = function(...) end
describe("shadowed", function() end)
"#;
        for lua in [false, true] {
            let parsed = if lua {
                crate::parse_lua_source("spec.lua", source)
            } else {
                crate::parse_tua_source("spec.tua", source)
            }
            .unwrap();
            let discovery = super::discover_tests(&parsed);
            assert_eq!(discovery.suites.len(), 3);
            assert_eq!(discovery.suites[0].full_name, "outer [x]");
            assert_eq!(discovery.suites[0].parent, None);
            assert_eq!(discovery.suites[0].line, 1);
            assert_eq!(discovery.suites[1].parent.as_deref(), Some("outer [x]"));
            assert_eq!(discovery.suites[1].line, 2);
            assert_eq!(discovery.suites[1].column, 3);
            assert_eq!(discovery.suites[2].full_name, "empty");
            assert_eq!(discovery.cases.len(), 1);
            assert_eq!(
                discovery.cases[0].suite.as_deref(),
                Some("outer [x] nested")
            );
            assert_eq!(discovery.cases[0].line, 3);
        }
    }

    #[test]
    fn decodes_escaped_static_busted_names_like_the_lua_runtime() {
        let source = r#"describe("math\n", function()
  it("works\065", function() end)
end)
"#;
        let parsed = crate::parse_tua_source("spec.tua", source).unwrap();
        let cases = super::discover_test_cases(&parsed);
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].full_name, "math\n worksA");
    }
}
