//! Context contracts for the existing behavior runtime. This specializes shared
//! call signatures; no runtime fields or source rewrites are introduced.
use crate::ast_utils::field_chain_before_first_call;
use crate::types::{FunctionParameter, FunctionType, TableField, TableShape, Type};
use full_moon::ast::{Call, Expression, FunctionArgs, FunctionCall, Suffix};

const CONTEXT: &str = "$tua_behavior_context";

pub(crate) fn is_internal_field(table: &TableShape, name: &str) -> bool {
    table.field(CONTEXT).is_some() && name == CONTEXT
}

fn function(
    parameters: impl IntoIterator<Item = (&'static str, Type)>,
    required: usize,
    result: Type,
) -> FunctionType {
    FunctionType::new_signature(
        parameters
            .into_iter()
            .map(|(name, ty)| FunctionParameter::named(name, ty)),
        required,
        None,
        [result],
    )
}

fn status() -> Type {
    Type::union(["success", "failure", "running"].map(|value| Type::StringLiteral(value.into())))
}

fn node(context: Type) -> Type {
    // A private invariant witness preserves the callback contract through
    // imports and collections. It is optional so dynamic LuaDoc nodes (for
    // example an extracted constructor) retain their existing permissive API.
    let witness = Type::Function(function([("context", context.clone())], 1, context));
    let table = TableShape::from_fields([
        ("kind", TableField::required(Type::string())),
        (CONTEXT, TableField::optional(witness)),
    ]);
    Type::Table(table)
}

fn context(ty: &Type) -> Option<Type> {
    match ty {
        Type::Table(table) if table.field(CONTEXT).is_some() => {
            witness_context(&table.field(CONTEXT)?.ty)
        }
        Type::Array(item) => context(item),
        Type::Tuple(items) | Type::Union(items) => {
            items.iter().filter_map(context).find(|ty| *ty != Type::Any)
        }
        _ => None,
    }
}

fn witness_context(ty: &Type) -> Option<Type> {
    match ty {
        Type::Function(witness) => Some(witness.return_type.as_ref().clone()),
        Type::Union(types) => types
            .iter()
            .filter_map(witness_context)
            .find(|ty| *ty != Type::Any),
        _ => None,
    }
}

fn callback_context(ty: &Type) -> Option<Type> {
    let Type::Function(callback) = ty else {
        return None;
    };
    callback
        .parameters
        .first()
        .map(|parameter| parameter.ty.clone())
}

fn runner(context: Type) -> Type {
    Type::table([
        ("context", TableField::required(context)),
        ("maxSteps", TableField::required(Type::number())),
        ("maxDepth", TableField::required(Type::number())),
        (
            "update",
            TableField::required(Type::Function(function(
                [("self", Type::Any), ("dt", Type::number())],
                2,
                status(),
            ))),
        ),
        (
            "reset",
            TableField::required(Type::Function(function(
                [("self", Type::Any)],
                1,
                Type::nil(),
            ))),
        ),
        (
            "cancel",
            TableField::required(Type::Function(function(
                [("self", Type::Any)],
                1,
                Type::nil(),
            ))),
        ),
    ])
}

fn definition(context: Type) -> Type {
    Type::table([
        (
            "instance",
            TableField::required(Type::Function(function(
                [("self", Type::Any), ("context", context.clone())],
                2,
                runner(context.clone()),
            ))),
        ),
        (
            "setRoot",
            TableField::required(Type::Function(function(
                [("self", Type::Any), ("root", node(context))],
                2,
                Type::Any,
            ))),
        ),
    ])
}

/// Cheap prefilter before a caller resolves a potentially expensive receiver.
pub(crate) fn is_constructor(method: &str) -> bool {
    matches!(
        method,
        "action"
            | "condition"
            | "sequence"
            | "selector"
            | "reactiveSequence"
            | "reactiveSelector"
            | "parallelAll"
            | "parallelAny"
            | "invert"
            | "new"
            | "repeatN"
            | "wait"
    )
}

/// All consumers (inference, checking and editor signatures) use this contract.
/// Identity is structural, so local module aliases work and unrelated methods
/// with the same names are not treated as behavior constructors.
pub(crate) fn specialized_call(
    receiver: &Type,
    call: &FunctionCall,
    mut resolve: impl FnMut(&Expression) -> Option<Type>,
) -> Option<FunctionType> {
    let parts = field_chain_before_first_call(call)?;
    let method = parts.last()?.as_str();
    if !is_constructor(method)
        || !crate::builtin_module::matches_module_type(
            receiver,
            &crate::builtin_module::BEHAVIOR_TREE,
        )
    {
        return None;
    }
    let mut argument = |index: usize| {
        call.suffixes().find_map(|suffix| match suffix {
            Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })) => {
                arguments.iter().nth(index).and_then(&mut resolve)
            }
            Suffix::Call(Call::AnonymousCall(FunctionArgs::TableConstructor(table)))
                if index == 0 =>
            {
                resolve(&Expression::TableConstructor(table.clone()))
            }
            _ => None,
        })
    };
    Some(match method {
        "action" | "condition" => {
            let count = if method == "action" { 3 } else { 1 };
            let actor = (0..count)
                .filter_map(|index| argument(index).as_ref().and_then(callback_context))
                .find(|ty| *ty != Type::Any)
                .unwrap_or(Type::Any);
            let tick = Type::Function(function(
                [("context", actor.clone()), ("dt", Type::number())],
                2,
                if method == "condition" {
                    Type::boolean()
                } else {
                    status()
                },
            ));
            if method == "condition" {
                function([("test", tick)], 1, node(actor))
            } else {
                let enter = Type::union([
                    Type::nil(),
                    Type::Function(function([("context", actor.clone())], 1, Type::nil())),
                ]);
                let exit = Type::union([
                    Type::nil(),
                    Type::Function(function(
                        [("context", actor.clone()), ("reason", Type::string())],
                        2,
                        Type::nil(),
                    )),
                ]);
                function(
                    [("tick", tick), ("enter", enter), ("exit", exit)],
                    1,
                    node(actor),
                )
            }
        }
        "sequence" | "selector" | "reactiveSequence" | "reactiveSelector" | "parallelAll"
        | "parallelAny" => {
            let actor = argument(0).as_ref().and_then(context).unwrap_or(Type::Any);
            function(
                [("children", Type::array(node(actor.clone())))],
                1,
                node(actor),
            )
        }
        "invert" | "new" => {
            let actor = argument(0).as_ref().and_then(context).unwrap_or(Type::Any);
            function(
                [("root", node(actor.clone()))],
                usize::from(method != "new"),
                if method == "new" {
                    definition(actor)
                } else {
                    node(actor)
                },
            )
        }
        "repeatN" => {
            let actor = argument(1).as_ref().and_then(context).unwrap_or(Type::Any);
            function(
                [("count", Type::number()), ("child", node(actor.clone()))],
                2,
                node(actor),
            )
        }
        "wait" => function([("seconds", Type::number())], 1, node(Type::Any)),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::check_tua_source;

    const TREE: &str = r#"
type Actor = { health: number }
local bt = BehaviorTree
local tree = bt.new(bt.reactiveSelector({
  bt.sequence({
    bt.condition(function(actor: Actor) return actor.health > 0 end),
    bt.repeatN(2, bt.invert(bt.action(function(actor: Actor)
      actor.health -= 1
      return "success"
    end))),
  }),
  bt.wait(1),
  bt.action(function() return "running" end),
}))
"#;
    #[test]
    fn behavior_context_contract_checks_runner_and_callbacks() {
        check_tua_source("src/brain.tua", format!("{TREE}\nlocal runner = tree:instance({{health=5}})\nlocal _hp: number = runner.context.health\nrunner:update(.1)\nrunner:cancel()\n")).expect("valid actor");
        for suffix in [
            "tree:instance({health='bad'})",
            "local runner = tree:instance({health=1})\nrunner.context.health = 'bad'",
            "tree:setRoot(bt.action(function(actor: {name: string}) return 'success' end))",
        ] {
            let errors = check_tua_source("src/brain.tua", format!("{TREE}\n{suffix}\n"))
                .expect_err("invalid context");
            assert!(
                errors
                    .iter()
                    .any(|d| d.code == Some(crate::DiagnosticCode::TYPE_MISMATCH)),
                "{errors:?}"
            );
        }
    }
    #[test]
    fn behavior_context_contract_rejects_mixed_actors_and_hook_types() {
        for source in [
            "BehaviorTree.new(BehaviorTree.sequence({ BehaviorTree.action(function(a: {health: number}) return 'success' end), BehaviorTree.condition(function(a: {name: string}) return true end) }))",
            "BehaviorTree.action(function(a: {health: number}) return 'success' end, function(a: {health: string}) end)",
            "BehaviorTree.action(function(a: {health: number}) return 'wrong' end)",
        ] {
            check_tua_source("src/brain.tua", source).expect_err("incompatible callbacks");
        }
    }
    #[test]
    fn behavior_context_contract_uses_hooks_and_table_call_sugar() {
        let source = "local leaf = BehaviorTree.action(function() return 'running' end, function(actor: {health: number}) actor.health -= 1 end)\nlocal nodes = {BehaviorTree.wait(.1), leaf}\nlocal tree = BehaviorTree.new(BehaviorTree.selector(nodes))\ntree:instance({health='bad'})";
        let errors =
            check_tua_source("src/hooks.tua", source).expect_err("hook context must reach runner");
        assert!(
            errors
                .iter()
                .any(|d| d.code == Some(crate::DiagnosticCode::TYPE_MISMATCH)),
            "{errors:?}"
        );
        let source = "local tree = BehaviorTree.new(BehaviorTree.sequence {BehaviorTree.condition(function(actor: {health: number}) return actor.health > 0 end), BehaviorTree.wait(1)})\ntree:instance({health='bad'})";
        let errors =
            check_tua_source("src/sugar.tua", source).expect_err("table call must retain context");
        assert!(
            errors
                .iter()
                .any(|d| d.code == Some(crate::DiagnosticCode::TYPE_MISMATCH)),
            "{errors:?}"
        );
    }

    #[test]
    fn behavior_context_contract_preserves_dynamic_constructor_values() {
        check_tua_source("src/dynamic.tua", "local wait = BehaviorTree.wait\nlocal tree = BehaviorTree.new(wait(1))\nlocal actor: any = {}\ntree:instance(actor)\n").unwrap();
    }

    #[test]
    fn behavior_context_contract_does_not_specialize_shadowed_module() {
        let diagnostics = check_tua_source("src/brain.tua", "local BehaviorTree = { new = function(value: string): string return value end }\nlocal _value: string = BehaviorTree.new('ok')\n").unwrap_err();
        assert!(
            diagnostics
                .iter()
                .all(|d| d.severity != crate::DiagnosticSeverity::Error),
            "{diagnostics:?}"
        );
    }
}
