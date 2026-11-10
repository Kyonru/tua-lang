use std::collections::HashMap;

use full_moon::ast::{Call, Expression, Field, FunctionArgs, FunctionCall, Prefix, Stmt, Suffix};
use full_moon::node::Node;

use crate::ast_utils::{field_chain_before_first_call, token_text};
use crate::inspector::{BehaviorTreeInspection, BehaviorTreeNodeInspection};
use crate::{ParsedSource, SourceSpan};

const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 1_000;

pub(crate) fn inspections(parsed: &ParsedSource) -> Vec<BehaviorTreeInspection> {
    let mut trees = Vec::new();
    let mut names = HashMap::new();
    for statement in parsed.ast.nodes().stmts() {
        match statement {
            Stmt::LocalAssignment(local) => {
                let (Some(name), Some(expression)) = (
                    local.names().iter().next(),
                    local.expressions().iter().next(),
                ) else {
                    continue;
                };
                if local.names().iter().count() != 1 || local.expressions().iter().count() != 1 {
                    continue;
                }
                let name_text = token_text(name);
                let Some(call) = call_expression(expression) else {
                    names.remove(&name_text);
                    continue;
                };
                if !is_tree_constructor(call) {
                    names.remove(&name_text);
                    continue;
                }
                let (Some(span), Some(name_span)) = (span(local), span(name)) else {
                    continue;
                };
                let mut remaining = MAX_NODES;
                let mut truncated = false;
                let root = first_argument(call).and_then(|expression| {
                    inspect_node(expression, 0, &mut remaining, &mut truncated)
                });
                names.insert(name_text.clone(), trees.len());
                trees.push(BehaviorTreeInspection {
                    name: name_text,
                    root,
                    truncated,
                    span,
                    name_span,
                });
            }
            Stmt::FunctionCall(call) => {
                let Prefix::Name(name) = call.prefix() else {
                    continue;
                };
                let Some(Suffix::Call(Call::MethodCall(method))) = call.suffixes().next() else {
                    continue;
                };
                if call.suffixes().count() != 1 || token_text(method.name()) != "setRoot" {
                    continue;
                }
                let Some(index) = names.get(&token_text(name)).copied() else {
                    continue;
                };
                let FunctionArgs::Parentheses { arguments, .. } = method.args() else {
                    continue;
                };
                if let Some(root) = arguments.iter().next() {
                    let mut remaining = MAX_NODES;
                    let mut truncated = false;
                    trees[index].root = inspect_node(root, 0, &mut remaining, &mut truncated);
                    trees[index].truncated = truncated;
                }
            }
            _ => {}
        }
    }
    trees
}

fn inspect_node(
    expression: &Expression,
    depth: usize,
    remaining: &mut usize,
    truncated: &mut bool,
) -> Option<BehaviorTreeNodeInspection> {
    if *remaining == 0 {
        *truncated = true;
        return None;
    }
    *remaining -= 1;
    let node_span = span(expression)?;
    if depth >= MAX_DEPTH {
        return Some(BehaviorTreeNodeInspection {
            kind: "dynamic".to_owned(),
            children: Vec::new(),
            span: node_span,
        });
    }
    let Some(call) = call_expression(expression) else {
        return Some(dynamic_node(node_span));
    };
    let Some(parts) = field_chain_before_first_call(call) else {
        return Some(dynamic_node(node_span));
    };
    if parts.len() != 2 || parts[0] != "BehaviorTree" || call.suffixes().count() != 2 {
        return Some(dynamic_node(node_span));
    }
    let kind = parts[1].as_str();
    let children = match kind {
        "sequence" | "selector" | "reactiveSequence" | "reactiveSelector" | "parallelAll"
        | "parallelAny" => first_argument(call)
            .and_then(|argument| match argument {
                Expression::TableConstructor(table) => Some(
                    table
                        .fields()
                        .iter()
                        .filter_map(|field| match field {
                            Field::NoKey(expression) => {
                                inspect_node(expression, depth + 1, remaining, truncated)
                            }
                            _ => None,
                        })
                        .collect(),
                ),
                _ => inspect_node(argument, depth + 1, remaining, truncated).map(|node| vec![node]),
            })
            .unwrap_or_default(),
        "invert" => first_argument(call)
            .and_then(|argument| inspect_node(argument, depth + 1, remaining, truncated))
            .into_iter()
            .collect(),
        "repeatN" => argument_at(call, 1)
            .and_then(|argument| inspect_node(argument, depth + 1, remaining, truncated))
            .into_iter()
            .collect(),
        "action" | "condition" | "wait" => Vec::new(),
        _ => return Some(dynamic_node(node_span)),
    };
    Some(BehaviorTreeNodeInspection {
        kind: kind.to_owned(),
        children,
        span: node_span,
    })
}

fn dynamic_node(span: SourceSpan) -> BehaviorTreeNodeInspection {
    BehaviorTreeNodeInspection {
        kind: "dynamic".to_owned(),
        children: Vec::new(),
        span,
    }
}

fn is_tree_constructor(call: &FunctionCall) -> bool {
    field_chain_before_first_call(call).as_deref()
        == Some(&["BehaviorTree".to_owned(), "new".to_owned()])
        && call.suffixes().count() == 2
}

fn call_expression(expression: &Expression) -> Option<&FunctionCall> {
    match expression {
        Expression::FunctionCall(call) => Some(call),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => call_expression(expression),
        _ => None,
    }
}

fn first_argument(call: &FunctionCall) -> Option<&Expression> {
    argument_at(call, 0)
}

fn argument_at(call: &FunctionCall, index: usize) -> Option<&Expression> {
    let suffix = call
        .suffixes()
        .find(|suffix| matches!(suffix, Suffix::Call(_)))?;
    let Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })) = suffix
    else {
        return None;
    };
    arguments.iter().nth(index)
}

fn span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_tua_source;

    #[test]
    fn inspects_nested_nodes_and_dynamic_children_without_running_source() {
        let parsed = parse_tua_source(
            "src/ai.tua",
            r#"local fallback = BehaviorTree.wait(1)
local patrol = BehaviorTree.new(BehaviorTree.selector({
  BehaviorTree.sequence({
    BehaviorTree.condition(function(actor) return actor.ready end),
    BehaviorTree.action(function() return "success" end),
  }),
  fallback,
}))
"#,
        )
        .unwrap();
        let trees = inspections(&parsed);
        assert_eq!(trees.len(), 1);
        assert_eq!(trees[0].name, "patrol");
        let root = trees[0].root.as_ref().unwrap();
        assert_eq!(root.kind, "selector");
        assert_eq!(root.children[0].kind, "sequence");
        assert_eq!(root.children[0].children[0].kind, "condition");
        assert_eq!(root.children[0].children[1].kind, "action");
        assert_eq!(root.children[1].kind, "dynamic");
        assert!(root.children[0].span.start.line > trees[0].name_span.start.line);
    }

    #[test]
    fn top_level_set_root_supplies_a_tree_created_without_a_root() {
        let parsed = parse_tua_source(
            "src/ai.tua",
            "local patrol = BehaviorTree.new()\npatrol:setRoot(BehaviorTree.repeatN(3, BehaviorTree.wait(0.2)))\n",
        )
        .unwrap();
        let trees = inspections(&parsed);
        assert_eq!(trees[0].root.as_ref().unwrap().kind, "repeatN");
        assert_eq!(trees[0].root.as_ref().unwrap().children[0].kind, "wait");
    }

    #[test]
    fn shadowed_top_level_name_does_not_retarget_an_earlier_tree() {
        let parsed = parse_tua_source(
            "src/ai.tua",
            "local tree = BehaviorTree.new()\nlocal tree = {}\ntree:setRoot(BehaviorTree.wait(1))\n",
        )
        .unwrap();
        let trees = inspections(&parsed);
        assert_eq!(trees.len(), 1);
        assert!(trees[0].root.is_none());
    }
}
