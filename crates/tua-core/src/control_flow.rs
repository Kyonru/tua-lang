use std::collections::BTreeSet;

use full_moon::ast::{
    self, Call, Expression, FunctionArgs, FunctionCall, LastStmt, Prefix, Stmt, Suffix,
};
use full_moon::node::Node;

use crate::parser::SourcePosition;

pub(crate) fn block_fallthrough_position(
    block: &ast::Block,
    empty_position: SourcePosition,
    statement_terminates: &impl Fn(&Stmt) -> bool,
) -> Option<SourcePosition> {
    block_can_fall_through(block, statement_terminates).then(|| {
        block
            .stmts()
            .last()
            .map(node_end_position)
            .unwrap_or(empty_position)
    })
}

fn block_can_fall_through(
    block: &ast::Block,
    statement_terminates: &impl Fn(&Stmt) -> bool,
) -> bool {
    for statement in block.stmts() {
        if !statement_can_fall_through(statement, statement_terminates) {
            return false;
        }
    }

    !matches!(
        block.last_stmt(),
        Some(LastStmt::Return(_) | LastStmt::Break(_))
    )
}

fn statement_can_fall_through(
    statement: &Stmt,
    statement_terminates: &impl Fn(&Stmt) -> bool,
) -> bool {
    match statement {
        Stmt::Do(statement) => block_can_fall_through(statement.block(), statement_terminates),
        Stmt::If(statement) => {
            if block_can_fall_through(statement.block(), statement_terminates)
                || statement.else_if().is_some_and(|branches| {
                    branches
                        .iter()
                        .any(|branch| block_can_fall_through(branch.block(), statement_terminates))
                })
            {
                return true;
            }

            match statement.else_block() {
                Some(block) => block_can_fall_through(block, statement_terminates),
                None => true,
            }
        }
        Stmt::While(statement) => {
            !is_true_literal(statement.condition())
                || block_has_reachable_break(statement.block(), statement_terminates)
        }
        Stmt::Repeat(statement) => {
            let body_falls_through =
                block_can_fall_through(statement.block(), statement_terminates);
            let body_can_break = block_has_reachable_break(statement.block(), statement_terminates);

            if !body_falls_through {
                body_can_break
            } else {
                !is_false_literal(statement.until()) || body_can_break
            }
        }
        _ => !statement_terminates(statement),
    }
}

fn block_has_reachable_break(
    block: &ast::Block,
    statement_terminates: &impl Fn(&Stmt) -> bool,
) -> bool {
    for statement in block.stmts() {
        if statement_has_reachable_break(statement, statement_terminates) {
            return true;
        }
        if !statement_can_fall_through(statement, statement_terminates) {
            return false;
        }
    }

    matches!(block.last_stmt(), Some(LastStmt::Break(_)))
}

fn statement_has_reachable_break(
    statement: &Stmt,
    statement_terminates: &impl Fn(&Stmt) -> bool,
) -> bool {
    match statement {
        Stmt::Do(statement) => block_has_reachable_break(statement.block(), statement_terminates),
        Stmt::If(if_statement) => {
            block_has_reachable_break(if_statement.block(), statement_terminates)
                || if_statement.else_if().is_some_and(|branches| {
                    branches.iter().any(|branch| {
                        block_has_reachable_break(branch.block(), statement_terminates)
                    })
                })
                || if_statement
                    .else_block()
                    .is_some_and(|block| block_has_reachable_break(block, statement_terminates))
        }
        _ => false,
    }
}

pub(crate) fn is_false_literal(expression: &Expression) -> bool {
    match expression {
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => is_false_literal(expression),
        Expression::Symbol(token) => token.to_string().trim() == "false",
        _ => false,
    }
}

pub(crate) fn call_is_terminating_builtin(
    call: &FunctionCall,
    terminating_builtin_references: &BTreeSet<(usize, usize)>,
) -> bool {
    let Prefix::Name(name) = call.prefix() else {
        return false;
    };
    let position = SourcePosition::from_full_moon(name.token().start_position());
    if !terminating_builtin_references.contains(&(position.line, position.column)) {
        return false;
    }

    let Some(Suffix::Call(Call::AnonymousCall(arguments))) = call.suffixes().next() else {
        return false;
    };

    match name.to_string().trim() {
        "error" => true,
        "assert" => match arguments {
            FunctionArgs::Parentheses { arguments, .. } => {
                arguments.iter().next().is_some_and(is_false_literal)
            }
            _ => false,
        },
        _ => false,
    }
}

pub(crate) fn statement_is_terminating_builtin(
    statement: &Stmt,
    terminating_builtin_references: &BTreeSet<(usize, usize)>,
) -> bool {
    matches!(statement, Stmt::FunctionCall(call) if call_is_terminating_builtin(call, terminating_builtin_references))
}

fn is_true_literal(expression: &Expression) -> bool {
    match expression {
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => is_true_literal(expression),
        Expression::Symbol(token) => token.to_string().trim() == "true",
        _ => false,
    }
}

fn node_end_position(node: &impl Node) -> SourcePosition {
    node.range()
        .map(|(_, end)| SourcePosition::from_full_moon(end))
        .unwrap_or(SourcePosition { line: 1, column: 1 })
}
