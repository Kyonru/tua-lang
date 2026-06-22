use std::path::PathBuf;

use full_moon::ast::{
    self, Assignment, Call, Expression, Field, FunctionBody, Index, LastStmt, Prefix, Suffix, Var,
};
use full_moon::node::Node;
use full_moon::tokenizer::TokenReference;

use crate::{Definition, DefinitionKind, ParsedSource, SourceSpan};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallHierarchySymbol {
    pub name: String,
    pub kind: CallHierarchySymbolKind,
    pub path: PathBuf,
    pub range: SourceSpan,
    pub selection_range: SourceSpan,
    pub definition: Definition,
    pub target: CallHierarchySymbolTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallHierarchySymbolKind {
    Function,
    Method,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallHierarchySymbolTarget {
    Local(String),
    Qualified(Vec<String>),
    Method { receiver: String, method: String },
    Export(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallHierarchyCallSite {
    pub span: SourceSpan,
    pub target: CallHierarchyCallTarget,
    pub caller: Option<Definition>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallHierarchyCallTarget {
    Name(String),
    Field(Vec<String>),
    Method { receiver: String, method: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallHierarchyExport {
    pub field: String,
    pub target_name: Option<String>,
    pub span: SourceSpan,
}

pub fn call_hierarchy_prepare_parsed_source(
    parsed: &ParsedSource,
    position: crate::SourcePosition,
) -> Option<CallHierarchySymbol> {
    call_hierarchy_symbols_parsed_source(parsed)
        .into_iter()
        .find(|symbol| span_contains(symbol.selection_range, position))
}

pub fn call_hierarchy_symbols_parsed_source(parsed: &ParsedSource) -> Vec<CallHierarchySymbol> {
    let mut symbols = Vec::new();

    collect_block_symbols(parsed, parsed.ast.nodes(), &mut symbols);
    symbols.sort_by_key(symbol_key);
    symbols
}

pub fn call_hierarchy_calls_parsed_source(parsed: &ParsedSource) -> Vec<CallHierarchyCallSite> {
    let symbols = call_hierarchy_symbols_parsed_source(parsed);
    let mut calls = Vec::new();

    collect_block_calls(parsed.ast.nodes(), &mut calls);

    for call in &mut calls {
        call.caller = symbols
            .iter()
            .filter(|symbol| span_contains(symbol.range, call.span.start))
            .min_by_key(|symbol| span_size(symbol.range))
            .map(|symbol| symbol.definition.clone());
    }

    calls.sort_by_key(|call| {
        (
            call.span.start.line,
            call.span.start.column,
            call.span.end.line,
            call.span.end.column,
        )
    });
    calls
}

pub fn call_hierarchy_exports_parsed_source(parsed: &ParsedSource) -> Vec<CallHierarchyExport> {
    let Some(expression) = returned_expression(parsed) else {
        return Vec::new();
    };

    exports_from_expression(expression)
}

fn collect_block_symbols(
    parsed: &ParsedSource,
    block: &ast::Block,
    symbols: &mut Vec<CallHierarchySymbol>,
) {
    for stmt in block.stmts() {
        collect_stmt_symbols(parsed, stmt, symbols);
    }

    if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
        for expression in return_stmt.returns() {
            collect_return_expression_symbols(parsed, expression, symbols);
        }
    }
}

fn collect_stmt_symbols(
    parsed: &ParsedSource,
    stmt: &ast::Stmt,
    symbols: &mut Vec<CallHierarchySymbol>,
) {
    match stmt {
        ast::Stmt::Assignment(assignment) => {
            collect_assignment_symbols(parsed, assignment, symbols)
        }
        ast::Stmt::Do(block) => collect_block_symbols(parsed, block.block(), symbols),
        ast::Stmt::FunctionDeclaration(function) => {
            collect_function_declaration_symbol(parsed, stmt, function, symbols);
            collect_block_symbols(parsed, function.body().block(), symbols);
        }
        ast::Stmt::GenericFor(for_stmt) => collect_block_symbols(parsed, for_stmt.block(), symbols),
        ast::Stmt::If(if_stmt) => {
            collect_block_symbols(parsed, if_stmt.block(), symbols);

            if let Some(else_ifs) = if_stmt.else_if() {
                for else_if in else_ifs {
                    collect_block_symbols(parsed, else_if.block(), symbols);
                }
            }

            if let Some(else_block) = if_stmt.else_block() {
                collect_block_symbols(parsed, else_block, symbols);
            }
        }
        ast::Stmt::LocalAssignment(local) => {
            for (name, expression) in local.names().iter().zip(local.expressions()) {
                let Some(body) = function_body_from_expression(expression) else {
                    collect_expression_symbols(parsed, expression, symbols);
                    continue;
                };
                let Some(selection_range) = node_span(name) else {
                    continue;
                };
                let Some(range) = node_span(local) else {
                    continue;
                };
                let name_text = token_text(name);

                symbols.push(symbol(
                    parsed,
                    name_text.clone(),
                    CallHierarchySymbolKind::Function,
                    range,
                    selection_range,
                    CallHierarchySymbolTarget::Local(name_text),
                ));
                collect_block_symbols(parsed, body.block(), symbols);
            }
        }
        ast::Stmt::LocalFunction(function) => {
            let Some(selection_range) = node_span(function.name()) else {
                return;
            };
            let Some(range) = node_span(stmt) else {
                return;
            };
            let name = token_text(function.name());

            symbols.push(symbol(
                parsed,
                name.clone(),
                CallHierarchySymbolKind::Function,
                range,
                selection_range,
                CallHierarchySymbolTarget::Local(name),
            ));
            collect_block_symbols(parsed, function.body().block(), symbols);
        }
        ast::Stmt::NumericFor(for_stmt) => collect_block_symbols(parsed, for_stmt.block(), symbols),
        ast::Stmt::Repeat(repeat) => collect_block_symbols(parsed, repeat.block(), symbols),
        ast::Stmt::While(while_stmt) => collect_block_symbols(parsed, while_stmt.block(), symbols),
        _ => {}
    }
}

fn collect_assignment_symbols(
    parsed: &ParsedSource,
    assignment: &Assignment,
    symbols: &mut Vec<CallHierarchySymbol>,
) {
    for (var, expression) in assignment.variables().iter().zip(assignment.expressions()) {
        let Some(body) = function_body_from_expression(expression) else {
            collect_expression_symbols(parsed, expression, symbols);
            continue;
        };
        let Some(parts) = chain_parts_from_var(var) else {
            continue;
        };
        let Some(last) = parts.last() else {
            continue;
        };
        let Some(range) = node_span(assignment) else {
            continue;
        };
        let name = parts
            .iter()
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join(".");

        symbols.push(symbol(
            parsed,
            name,
            CallHierarchySymbolKind::Function,
            range,
            last.span,
            CallHierarchySymbolTarget::Qualified(parts.into_iter().map(|part| part.text).collect()),
        ));
        collect_block_symbols(parsed, body.block(), symbols);
    }
}

fn collect_function_declaration_symbol(
    parsed: &ParsedSource,
    stmt: &ast::Stmt,
    function: &ast::FunctionDeclaration,
    symbols: &mut Vec<CallHierarchySymbol>,
) {
    let Some(range) = node_span(stmt) else {
        return;
    };

    if let Some(method) = function.name().method_name() {
        let Some(receiver) = function.name().names().iter().last() else {
            return;
        };
        let Some(selection_range) = node_span(method) else {
            return;
        };
        let receiver = token_text(receiver);
        let method = token_text(method);

        symbols.push(symbol(
            parsed,
            format!("{receiver}:{method}"),
            CallHierarchySymbolKind::Method,
            range,
            selection_range,
            CallHierarchySymbolTarget::Method { receiver, method },
        ));
        return;
    }

    let parts = function
        .name()
        .names()
        .iter()
        .filter_map(|name| {
            Some(ChainPart {
                text: token_text(name),
                span: node_span(name)?,
            })
        })
        .collect::<Vec<_>>();
    let Some(last) = parts.last() else {
        return;
    };
    let target = if parts.len() == 1 {
        CallHierarchySymbolTarget::Local(last.text.clone())
    } else {
        CallHierarchySymbolTarget::Qualified(parts.iter().map(|part| part.text.clone()).collect())
    };

    symbols.push(symbol(
        parsed,
        parts
            .iter()
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join("."),
        CallHierarchySymbolKind::Function,
        range,
        last.span,
        target,
    ));
}

fn collect_return_expression_symbols(
    parsed: &ParsedSource,
    expression: &Expression,
    symbols: &mut Vec<CallHierarchySymbol>,
) {
    match expression {
        Expression::TableConstructor(table) => {
            for field in table.fields() {
                match field {
                    Field::NameKey { key, value, .. } => {
                        let Some(body) = function_body_from_expression(value) else {
                            collect_expression_symbols(parsed, value, symbols);
                            continue;
                        };
                        let Some(selection_range) = node_span(key) else {
                            continue;
                        };
                        let Some(range) = node_span(field) else {
                            continue;
                        };
                        let name = token_text(key);

                        symbols.push(symbol(
                            parsed,
                            name.clone(),
                            CallHierarchySymbolKind::Function,
                            range,
                            selection_range,
                            CallHierarchySymbolTarget::Export(name),
                        ));
                        collect_block_symbols(parsed, body.block(), symbols);
                    }
                    Field::ExpressionKey { key, value, .. } => {
                        collect_expression_symbols(parsed, key, symbols);
                        collect_expression_symbols(parsed, value, symbols);
                    }
                    Field::NoKey(value) => collect_expression_symbols(parsed, value, symbols),
                    _ => {}
                }
            }
        }
        _ => collect_expression_symbols(parsed, expression, symbols),
    }
}

fn collect_expression_symbols(
    parsed: &ParsedSource,
    expression: &Expression,
    symbols: &mut Vec<CallHierarchySymbol>,
) {
    match expression {
        Expression::BinaryOperator { lhs, rhs, .. } => {
            collect_expression_symbols(parsed, lhs, symbols);
            collect_expression_symbols(parsed, rhs, symbols);
        }
        Expression::Function(function) => {
            collect_block_symbols(parsed, function.body().block(), symbols)
        }
        Expression::FunctionCall(call) => collect_call_arg_symbols(parsed, call, symbols),
        Expression::Parentheses { expression, .. } => {
            collect_expression_symbols(parsed, expression, symbols)
        }
        Expression::TableConstructor(table) => {
            for field in table.fields() {
                match field {
                    Field::ExpressionKey { key, value, .. } => {
                        collect_expression_symbols(parsed, key, symbols);
                        collect_expression_symbols(parsed, value, symbols);
                    }
                    Field::NameKey { value, .. } | Field::NoKey(value) => {
                        collect_expression_symbols(parsed, value, symbols);
                    }
                    _ => {}
                }
            }
        }
        Expression::TypeAssertion { expression, .. } => {
            collect_expression_symbols(parsed, expression, symbols);
        }
        Expression::UnaryOperator { expression, .. } => {
            collect_expression_symbols(parsed, expression, symbols)
        }
        _ => {}
    }
}

fn collect_block_calls(block: &ast::Block, calls: &mut Vec<CallHierarchyCallSite>) {
    for stmt in block.stmts() {
        collect_stmt_calls(stmt, calls);
    }

    if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
        for expression in return_stmt.returns() {
            collect_expression_calls(expression, calls);
        }
    }
}

fn collect_stmt_calls(stmt: &ast::Stmt, calls: &mut Vec<CallHierarchyCallSite>) {
    match stmt {
        ast::Stmt::Assignment(assignment) => {
            for expression in assignment.expressions() {
                collect_expression_calls(expression, calls);
            }
        }
        ast::Stmt::Do(block) => collect_block_calls(block.block(), calls),
        ast::Stmt::FunctionCall(call) => collect_function_call(call, calls),
        ast::Stmt::FunctionDeclaration(function) => {
            collect_block_calls(function.body().block(), calls)
        }
        ast::Stmt::GenericFor(for_stmt) => collect_block_calls(for_stmt.block(), calls),
        ast::Stmt::If(if_stmt) => {
            collect_block_calls(if_stmt.block(), calls);

            if let Some(else_ifs) = if_stmt.else_if() {
                for else_if in else_ifs {
                    collect_block_calls(else_if.block(), calls);
                }
            }

            if let Some(else_block) = if_stmt.else_block() {
                collect_block_calls(else_block, calls);
            }
        }
        ast::Stmt::LocalAssignment(local) => {
            for expression in local.expressions() {
                collect_expression_calls(expression, calls);
            }
        }
        ast::Stmt::LocalFunction(function) => collect_block_calls(function.body().block(), calls),
        ast::Stmt::NumericFor(for_stmt) => collect_block_calls(for_stmt.block(), calls),
        ast::Stmt::Repeat(repeat) => collect_block_calls(repeat.block(), calls),
        ast::Stmt::While(while_stmt) => collect_block_calls(while_stmt.block(), calls),
        _ => {}
    }
}

fn collect_expression_calls(expression: &Expression, calls: &mut Vec<CallHierarchyCallSite>) {
    match expression {
        Expression::BinaryOperator { lhs, rhs, .. } => {
            collect_expression_calls(lhs, calls);
            collect_expression_calls(rhs, calls);
        }
        Expression::Function(function) => collect_block_calls(function.body().block(), calls),
        Expression::FunctionCall(call) => collect_function_call(call, calls),
        Expression::Parentheses { expression, .. } => collect_expression_calls(expression, calls),
        Expression::TableConstructor(table) => {
            for field in table.fields() {
                match field {
                    Field::ExpressionKey { key, value, .. } => {
                        collect_expression_calls(key, calls);
                        collect_expression_calls(value, calls);
                    }
                    Field::NameKey { value, .. } | Field::NoKey(value) => {
                        collect_expression_calls(value, calls);
                    }
                    _ => {}
                }
            }
        }
        Expression::TypeAssertion { expression, .. } => collect_expression_calls(expression, calls),
        Expression::UnaryOperator { expression, .. } => collect_expression_calls(expression, calls),
        _ => {}
    }
}

fn collect_function_call(call: &ast::FunctionCall, calls: &mut Vec<CallHierarchyCallSite>) {
    let mut parts = prefix_chain_parts(call.prefix()).unwrap_or_default();

    for suffix in call.suffixes() {
        match suffix {
            Suffix::Index(Index::Dot { name, .. }) => {
                if let Some(span) = node_span(name) {
                    parts.push(ChainPart {
                        text: token_text(name),
                        span,
                    });
                }
            }
            Suffix::Call(Call::AnonymousCall(args)) => {
                if let Some(site) = call_site_for_parts(&parts) {
                    calls.push(site);
                }

                collect_function_args_calls(args, calls);
                break;
            }
            Suffix::Call(Call::MethodCall(method)) => {
                let Some(receiver) = parts.last() else {
                    collect_function_args_calls(method.args(), calls);
                    break;
                };
                let Some(span) = node_span(method.name()) else {
                    collect_function_args_calls(method.args(), calls);
                    break;
                };

                calls.push(CallHierarchyCallSite {
                    span,
                    target: CallHierarchyCallTarget::Method {
                        receiver: receiver.text.clone(),
                        method: token_text(method.name()),
                    },
                    caller: None,
                });
                collect_function_args_calls(method.args(), calls);
                break;
            }
            _ => {}
        }
    }
}

fn collect_call_arg_symbols(
    parsed: &ParsedSource,
    call: &ast::FunctionCall,
    symbols: &mut Vec<CallHierarchySymbol>,
) {
    for suffix in call.suffixes() {
        match suffix {
            Suffix::Call(Call::AnonymousCall(args)) => {
                collect_function_args_symbols(parsed, args, symbols);
                break;
            }
            Suffix::Call(Call::MethodCall(method)) => {
                collect_function_args_symbols(parsed, method.args(), symbols);
                break;
            }
            _ => {}
        }
    }
}

fn collect_function_args_calls(args: &ast::FunctionArgs, calls: &mut Vec<CallHierarchyCallSite>) {
    match args {
        ast::FunctionArgs::Parentheses { arguments, .. } => {
            for argument in arguments {
                collect_expression_calls(argument, calls);
            }
        }
        ast::FunctionArgs::TableConstructor(table) => {
            collect_expression_calls(&Expression::TableConstructor(table.clone()), calls);
        }
        ast::FunctionArgs::String(_) => {}
        _ => {}
    }
}

fn collect_function_args_symbols(
    parsed: &ParsedSource,
    args: &ast::FunctionArgs,
    symbols: &mut Vec<CallHierarchySymbol>,
) {
    match args {
        ast::FunctionArgs::Parentheses { arguments, .. } => {
            for argument in arguments {
                collect_expression_symbols(parsed, argument, symbols);
            }
        }
        ast::FunctionArgs::TableConstructor(table) => {
            collect_expression_symbols(
                parsed,
                &Expression::TableConstructor(table.clone()),
                symbols,
            );
        }
        ast::FunctionArgs::String(_) => {}
        _ => {}
    }
}

fn call_site_for_parts(parts: &[ChainPart]) -> Option<CallHierarchyCallSite> {
    let last = parts.last()?;
    let target = if parts.len() == 1 {
        CallHierarchyCallTarget::Name(last.text.clone())
    } else {
        CallHierarchyCallTarget::Field(parts.iter().map(|part| part.text.clone()).collect())
    };

    Some(CallHierarchyCallSite {
        span: last.span,
        target,
        caller: None,
    })
}

fn exports_from_expression(expression: &Expression) -> Vec<CallHierarchyExport> {
    let Expression::TableConstructor(table) = expression else {
        return Vec::new();
    };

    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NameKey { key, value, .. } = field else {
                return None;
            };
            let span = node_span(key)?;
            let target_name = match value {
                Expression::Var(Var::Name(name)) => Some(token_text(name)),
                _ => None,
            };

            Some(CallHierarchyExport {
                field: token_text(key),
                target_name,
                span,
            })
        })
        .collect()
}

fn returned_expression(parsed: &ParsedSource) -> Option<&Expression> {
    let LastStmt::Return(return_stmt) = parsed.ast.nodes().last_stmt()? else {
        return None;
    };

    return_stmt.returns().iter().next()
}

fn function_body_from_expression(expression: &Expression) -> Option<&FunctionBody> {
    match expression {
        Expression::Function(function) => Some(function.body()),
        Expression::Parentheses { expression, .. } => function_body_from_expression(expression),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChainPart {
    text: String,
    span: SourceSpan,
}

fn prefix_chain_parts(prefix: &Prefix) -> Option<Vec<ChainPart>> {
    match prefix {
        Prefix::Name(name) => Some(vec![ChainPart {
            text: token_text(name),
            span: node_span(name)?,
        }]),
        Prefix::Expression(expression) => {
            let Expression::Var(var) = expression.as_ref() else {
                return None;
            };

            chain_parts_from_var(var)
        }
        _ => None,
    }
}

fn chain_parts_from_var(var: &Var) -> Option<Vec<ChainPart>> {
    match var {
        Var::Name(name) => Some(vec![ChainPart {
            text: token_text(name),
            span: node_span(name)?,
        }]),
        Var::Expression(expression) => {
            let Prefix::Name(name) = expression.prefix() else {
                return None;
            };
            let mut parts = vec![ChainPart {
                text: token_text(name),
                span: node_span(name)?,
            }];

            for suffix in expression.suffixes() {
                let Suffix::Index(Index::Dot { name, .. }) = suffix else {
                    return None;
                };

                parts.push(ChainPart {
                    text: token_text(name),
                    span: node_span(name)?,
                });
            }

            Some(parts)
        }
        _ => None,
    }
}

fn symbol(
    parsed: &ParsedSource,
    name: String,
    kind: CallHierarchySymbolKind,
    range: SourceSpan,
    selection_range: SourceSpan,
    target: CallHierarchySymbolTarget,
) -> CallHierarchySymbol {
    CallHierarchySymbol {
        name,
        kind,
        path: parsed.path.clone(),
        range,
        selection_range,
        definition: Definition {
            path: parsed.path.clone(),
            span: selection_range,
            kind: DefinitionKind::Function,
        },
        target,
    }
}

fn symbol_key(symbol: &CallHierarchySymbol) -> (usize, usize, usize, usize, String) {
    (
        symbol.selection_range.start.line,
        symbol.selection_range.start.column,
        symbol.selection_range.end.line,
        symbol.selection_range.end.column,
        symbol.name.clone(),
    )
}

fn span_size(span: SourceSpan) -> (usize, usize) {
    (
        span.end.line.saturating_sub(span.start.line),
        span.end.column.saturating_sub(span.start.column),
    )
}

fn span_contains(span: SourceSpan, position: crate::SourcePosition) -> bool {
    let start = (span.start.line, span.start.column);
    let end = (span.end.line, span.end.column);
    let position = (position.line, position.column);

    start <= position && position < end
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn token_text(token: &TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

#[cfg(test)]
mod tests;
