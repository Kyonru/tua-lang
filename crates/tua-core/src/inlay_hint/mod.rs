use std::collections::BTreeMap;
use std::path::Path;

use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, Index, LastStmt, Prefix, Suffix,
    TableConstructor, Var,
};
use full_moon::node::Node;

use crate::asset::{self, AssetLoaderKind};
use crate::lua::{self, LuaItem};
use crate::semantic::{resolve_aliases, resolve_field_chain_type, resolve_table_field_type};
use crate::types::{
    build_type_environment, FunctionType, TableField, Type, TypeEnvironment, TypedFunction,
};
use crate::{parse_tua_source, Diagnostic, ParsedSource, SourcePosition};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlayHint {
    pub position: SourcePosition,
    pub label: String,
    pub kind: InlayHintKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InlayHintKind {
    Type,
    Parameter,
}

pub fn inlay_hints_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<Vec<InlayHint>, Vec<Diagnostic>> {
    let parsed = parse_tua_source(path, source)?;

    inlay_hints_parsed_source(&parsed)
}

pub fn inlay_hints_parsed_source(parsed: &ParsedSource) -> Result<Vec<InlayHint>, Vec<Diagnostic>> {
    let environment = build_type_environment(parsed)?;
    let mut collector = InlayHintCollector {
        environment: &environment,
        inferred_locals: BTreeMap::new(),
        hints: Vec::new(),
    };

    collector.collect_block(parsed.ast.nodes());
    collector
        .hints
        .sort_by_key(|hint| position_key(hint.position));

    Ok(collector.hints)
}

struct InlayHintCollector<'a> {
    environment: &'a TypeEnvironment,
    inferred_locals: BTreeMap<String, Type>,
    hints: Vec<InlayHint>,
}

impl<'a> InlayHintCollector<'a> {
    fn collect_block(&mut self, block: &ast::Block) {
        for stmt in block.stmts() {
            self.collect_stmt(stmt);
        }

        if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
            for expression in return_stmt.returns() {
                self.collect_expression(expression);
            }
        }
    }

    fn collect_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Assignment(assignment) => {
                for expression in assignment.expressions() {
                    self.collect_expression(expression);
                }
            }
            ast::Stmt::Do(block) => self.collect_block(block.block()),
            ast::Stmt::FunctionCall(function_call) => self.collect_function_call(function_call),
            ast::Stmt::FunctionDeclaration(function) => self.collect_block(function.body().block()),
            ast::Stmt::GenericFor(for_stmt) => self.collect_block(for_stmt.block()),
            ast::Stmt::If(if_stmt) => {
                self.collect_block(if_stmt.block());

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        self.collect_block(else_if.block());
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    self.collect_block(else_block);
                }
            }
            ast::Stmt::LocalAssignment(local) => self.collect_local_assignment(local),
            ast::Stmt::LocalFunction(function) => self.collect_block(function.body().block()),
            ast::Stmt::NumericFor(for_stmt) => self.collect_block(for_stmt.block()),
            ast::Stmt::Repeat(repeat) => self.collect_block(repeat.block()),
            ast::Stmt::While(while_stmt) => self.collect_block(while_stmt.block()),
            _ => {}
        }
    }

    fn collect_local_assignment(&mut self, local: &ast::LocalAssignment) {
        let specifiers = local.type_specifiers().collect::<Vec<_>>();
        let expressions = local.expressions().iter().collect::<Vec<_>>();

        for expression in &expressions {
            self.collect_expression(expression);
        }

        for (index, name) in local.names().iter().enumerate() {
            if specifiers
                .get(index)
                .and_then(|specifier| *specifier)
                .is_some()
            {
                continue;
            }

            let Some((expression, return_index)) =
                assignment_expression_for_local(&expressions, index)
            else {
                continue;
            };
            let Some(ty) = self.infer_expression_return(expression, return_index) else {
                continue;
            };

            self.inferred_locals.insert(token_text(name), ty.clone());

            if should_hint_type(&ty) {
                self.hints.push(InlayHint {
                    position: node_end(name),
                    label: format!(": {ty}"),
                    kind: InlayHintKind::Type,
                });
            }
        }
    }

    fn collect_expression(&mut self, expression: &Expression) {
        match expression {
            Expression::BinaryOperator { lhs, rhs, .. } => {
                self.collect_expression(lhs);
                self.collect_expression(rhs);
            }
            Expression::Function(function) => self.collect_block(function.body().block()),
            Expression::FunctionCall(function_call) => self.collect_function_call(function_call),
            Expression::Parentheses { expression, .. } => self.collect_expression(expression),
            Expression::TableConstructor(table) => self.collect_table_constructor(table),
            Expression::TypeAssertion { expression, .. } => self.collect_expression(expression),
            Expression::UnaryOperator { expression, .. } => self.collect_expression(expression),
            _ => {}
        }
    }

    fn collect_table_constructor(&mut self, table: &TableConstructor) {
        for field in table.fields() {
            match field {
                Field::ExpressionKey { key, value, .. } => {
                    self.collect_expression(key);
                    self.collect_expression(value);
                }
                Field::NameKey { value, .. } | Field::NoKey(value) => {
                    self.collect_expression(value);
                }
                _ => {}
            }
        }
    }

    fn collect_function_call(&mut self, function_call: &FunctionCall) {
        let Some(function) = self.typed_function_for_call(function_call) else {
            self.collect_call_arguments(function_call);
            return;
        };
        let arguments = parenthesized_arguments(function_call).unwrap_or_default();

        for (argument, parameter) in arguments.iter().zip(&function.parameters) {
            self.collect_expression(argument);

            if !parameter.name.is_empty() {
                self.hints.push(InlayHint {
                    position: node_start(*argument),
                    label: format!("{}:", parameter.name),
                    kind: InlayHintKind::Parameter,
                });
            }
        }

        for argument in arguments.iter().skip(function.parameters.len()) {
            self.collect_expression(argument);
        }
    }

    fn collect_call_arguments(&mut self, function_call: &FunctionCall) {
        if let Some(arguments) = parenthesized_arguments(function_call) {
            for argument in arguments {
                self.collect_expression(argument);
            }
        }
    }

    fn infer_expression(&self, expression: &Expression) -> Option<Type> {
        self.infer_expression_return(expression, 0)
    }

    fn infer_expression_return(
        &self,
        expression: &Expression,
        return_index: usize,
    ) -> Option<Type> {
        match expression {
            Expression::FunctionCall(function_call) => self
                .return_types_for_call(function_call)
                .get(return_index)
                .cloned(),
            _ if return_index > 0 => None,
            Expression::BinaryOperator { lhs, binop, rhs } => {
                let left = self.infer_expression(lhs)?;
                let right = self.infer_expression(rhs)?;

                infer_binary_operator_type(self.environment, binop, &left, &right)
            }
            Expression::Number(_) => Some(Type::number()),
            Expression::String(_) => Some(Type::string()),
            Expression::Symbol(symbol) => infer_symbol_type(&token_text(symbol)),
            Expression::TableConstructor(table) => Some(self.infer_table_constructor(table)),
            Expression::Parentheses { expression, .. } => {
                self.infer_expression_return(expression, return_index)
            }
            Expression::TypeAssertion { expression, .. } => {
                self.infer_expression_return(expression, return_index)
            }
            Expression::Var(var) => self.infer_var_value(var),
            _ => None,
        }
    }

    fn infer_table_constructor(&self, table: &TableConstructor) -> Type {
        Type::table(table.fields().iter().filter_map(|field| match field {
            Field::NameKey { key, value, .. } => {
                let ty = self.infer_expression(value).unwrap_or(Type::Any);

                Some((token_text(key), TableField::required(ty)))
            }
            _ => None,
        }))
    }

    fn infer_var_value(&self, var: &Var) -> Option<Type> {
        match var {
            Var::Name(name) => self.infer_named_value(&token_text(name)),
            Var::Expression(expression) => resolve_field_chain_type(
                self.environment,
                &plain_field_chain_from_var_expression(expression)?,
            ),
            _ => None,
        }
    }

    fn infer_named_value(&self, name: &str) -> Option<Type> {
        self.environment
            .local(name)
            .cloned()
            .or_else(|| self.inferred_locals.get(name).cloned())
            .or_else(|| {
                self.environment
                    .function(name)
                    .and_then(TypedFunction::as_type)
            })
    }

    fn typed_function_for_call(&self, function_call: &FunctionCall) -> Option<TypedFunction> {
        let parts = field_chain_before_first_call(function_call)?;
        let (root, fields) = parts.split_first()?;

        if fields.is_empty() {
            if let Some(function) = self.environment.function(root).cloned().filter(|function| {
                function.has_fully_annotated_parameters() && function.return_type.is_some()
            }) {
                return Some(function);
            }

            return asset::loader_kind(root).map(typed_function_from_asset_loader);
        }

        if let Some(function) = self.typed_love_object_method_for_parts(&parts) {
            return Some(function);
        }

        let callee_type = resolve_field_chain_type(self.environment, &parts)?;
        let Type::Function(function) = callee_type else {
            return None;
        };

        Some(typed_function_from_function_type(function))
    }

    fn typed_love_object_method_for_parts(&self, parts: &[String]) -> Option<TypedFunction> {
        let method = parts.last()?;
        let receiver_parts = parts.get(..parts.len().checked_sub(1)?)?;
        let receiver_type = self.infer_field_chain_type(receiver_parts)?;
        let item = lua_type_field_item(self.environment, &receiver_type, method)?;

        typed_function_from_lua_item(item)
    }

    fn return_types_for_call(&self, function_call: &FunctionCall) -> Vec<Type> {
        let Some(parts) = field_chain_before_first_call(function_call) else {
            return Vec::new();
        };

        if parts.len() == 1 {
            if let Some(function) = self.typed_function_for_call(function_call) {
                return function.return_types_or_any();
            }
        }

        if let Some(method) = parts.last() {
            if let Some(receiver_parts) = parts.get(..parts.len().saturating_sub(1)) {
                if let Some(receiver_type) = self.infer_field_chain_type(receiver_parts) {
                    if let Some(item) =
                        lua_type_field_item(self.environment, &receiver_type, method)
                    {
                        return return_types_from_function_detail(item.detail);
                    }
                }
            }
        }

        self.typed_function_for_call(function_call)
            .map(|function| function.return_types_or_any())
            .unwrap_or_default()
    }

    fn infer_field_chain_type(&self, parts: &[String]) -> Option<Type> {
        let (root, fields) = parts.split_first()?;
        let mut current = self.infer_named_value(root)?;

        for field in fields {
            current = resolve_table_field_type(self.environment, &current, field)?;
        }

        Some(current)
    }
}

fn typed_function_from_asset_loader(kind: AssetLoaderKind) -> TypedFunction {
    let return_type = kind.return_type();

    TypedFunction {
        name: Some(kind.name().to_owned()),
        parameters: vec![crate::TypedParameter {
            name: "path".to_owned(),
            ty: Type::string(),
            documentation: None,
        }],
        parameter_count: 1,
        has_varargs: false,
        return_type: Some(return_type.clone()),
        return_types: vec![return_type],
        documentation: Some(kind.documentation().to_owned()),
    }
}

fn typed_function_from_function_type(function: FunctionType) -> TypedFunction {
    let return_types = function.return_types_or_any().to_vec();

    TypedFunction {
        name: None,
        parameters: function
            .parameters
            .into_iter()
            .map(|ty| crate::TypedParameter {
                name: String::new(),
                ty,
                documentation: None,
            })
            .collect(),
        parameter_count: 0,
        has_varargs: false,
        return_type: return_types.first().cloned(),
        return_types,
        documentation: None,
    }
}

fn typed_function_from_lua_item(item: LuaItem) -> Option<TypedFunction> {
    let parameters = parameters_from_function_detail(item.detail)?;
    let mut return_types = return_types_from_function_detail(item.detail);

    if return_types.is_empty() {
        return_types.push(Type::Any);
    }

    Some(TypedFunction {
        name: Some(item.label.to_owned()),
        parameters,
        parameter_count: 0,
        has_varargs: false,
        return_type: return_types.first().cloned(),
        return_types,
        documentation: Some(item.documentation.to_owned()),
    })
}

fn parameters_from_function_detail(detail: &str) -> Option<Vec<crate::TypedParameter>> {
    let start = detail.find('(')? + 1;
    let end = detail[start..].find(')')? + start;
    let parameters = detail[start..end].trim();

    if parameters.is_empty() {
        return Some(Vec::new());
    }

    Some(
        parameters
            .split(',')
            .map(str::trim)
            .filter(|parameter| !parameter.is_empty())
            .map(|parameter| {
                let (name, ty) = parameter
                    .split_once(':')
                    .map(|(name, ty)| (name.trim().trim_end_matches('?'), ty.trim()))
                    .unwrap_or(("", parameter));

                crate::TypedParameter {
                    name: name.to_owned(),
                    ty: type_from_label(ty),
                    documentation: None,
                }
            })
            .collect(),
    )
}

fn return_types_from_function_detail(detail: &str) -> Vec<Type> {
    let Some(start) = detail.rfind("): ").map(|index| index + "): ".len()) else {
        return Vec::new();
    };

    detail[start..]
        .split(',')
        .map(str::trim)
        .filter(|ty| !ty.is_empty())
        .map(type_from_label)
        .collect()
}

fn type_from_label(label: &str) -> Type {
    match label {
        "any" => Type::Any,
        "boolean" => Type::boolean(),
        "nil" => Type::nil(),
        "number" => Type::number(),
        "string" => Type::string(),
        "table" => Type::empty_table(),
        other => Type::alias(other),
    }
}

fn lua_type_field_item(environment: &TypeEnvironment, ty: &Type, field: &str) -> Option<LuaItem> {
    match resolve_aliases(environment, ty, 0) {
        Type::Alias(name) => lua::type_field_items(&name, field)
            .into_iter()
            .find(|item| item.label == field),
        Type::Union(types) => types
            .iter()
            .find_map(|ty| lua_type_field_item(environment, ty, field)),
        Type::Any | Type::Function(_) | Type::Primitive(_) | Type::Table(_) => None,
    }
}

fn assignment_expression_for_local<'a>(
    expressions: &[&'a Expression],
    index: usize,
) -> Option<(&'a Expression, usize)> {
    if expressions.is_empty() {
        return None;
    }

    let fixed_expression_count = expressions.len().saturating_sub(1);

    if index < fixed_expression_count {
        return expressions
            .get(index)
            .copied()
            .map(|expression| (expression, 0));
    }

    expressions
        .last()
        .copied()
        .map(|expression| (expression, index - fixed_expression_count))
}

fn should_hint_type(ty: &Type) -> bool {
    !matches!(ty, Type::Any)
}

fn infer_symbol_type(symbol: &str) -> Option<Type> {
    match symbol {
        "true" | "false" => Some(Type::boolean()),
        "nil" => Some(Type::nil()),
        _ => None,
    }
}

fn infer_binary_operator_type(
    environment: &TypeEnvironment,
    binop: &ast::BinOp,
    left: &Type,
    right: &Type,
) -> Option<Type> {
    if is_numeric_operator(binop)
        && is_number_type(environment, left)
        && is_number_type(environment, right)
    {
        return Some(Type::number());
    }

    if matches!(binop, ast::BinOp::TwoDots(_))
        && is_concat_operand_type(environment, left)
        && is_concat_operand_type(environment, right)
    {
        return Some(Type::string());
    }

    if matches!(left, Type::Any) || matches!(right, Type::Any) {
        return Some(Type::Any);
    }

    None
}

fn is_numeric_operator(binop: &ast::BinOp) -> bool {
    matches!(
        binop,
        ast::BinOp::Caret(_)
            | ast::BinOp::Minus(_)
            | ast::BinOp::Percent(_)
            | ast::BinOp::Plus(_)
            | ast::BinOp::Slash(_)
            | ast::BinOp::Star(_)
    )
}

fn is_number_type(environment: &TypeEnvironment, ty: &Type) -> bool {
    match resolve_aliases(environment, ty, 0) {
        Type::Primitive(crate::PrimitiveType::Number) => true,
        Type::Union(types) => types.iter().all(|ty| is_number_type(environment, ty)),
        _ => false,
    }
}

fn is_concat_operand_type(environment: &TypeEnvironment, ty: &Type) -> bool {
    match resolve_aliases(environment, ty, 0) {
        Type::Primitive(crate::PrimitiveType::Number | crate::PrimitiveType::String) => true,
        Type::Union(types) => types
            .iter()
            .all(|ty| is_concat_operand_type(environment, ty)),
        _ => false,
    }
}

fn plain_field_chain_from_var_expression(expression: &ast::VarExpression) -> Option<Vec<String>> {
    let Prefix::Name(name) = expression.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in expression.suffixes() {
        let Suffix::Index(index) = suffix else {
            return None;
        };

        let Index::Dot { name, .. } = index else {
            return None;
        };

        parts.push(token_text(name));
    }

    Some(parts)
}

fn field_chain_before_first_call(function_call: &FunctionCall) -> Option<Vec<String>> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in function_call.suffixes() {
        match suffix {
            Suffix::Index(Index::Dot { name, .. }) => parts.push(token_text(name)),
            Suffix::Call(Call::MethodCall(method)) => {
                parts.push(token_text(method.name()));
                return Some(parts);
            }
            Suffix::Call(_) => return Some(parts),
            _ => return None,
        }
    }

    None
}

fn parenthesized_arguments(function_call: &FunctionCall) -> Option<Vec<&Expression>> {
    let call = function_call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;

    match call {
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            Some(arguments.iter().collect())
        }
        _ => None,
    }
}

fn node_start(node: &impl Node) -> SourcePosition {
    node.range()
        .map(|(start, _)| SourcePosition::from_full_moon(start))
        .unwrap_or(SourcePosition { line: 1, column: 1 })
}

fn node_end(node: &impl Node) -> SourcePosition {
    node.range()
        .map(|(_, end)| SourcePosition::from_full_moon(end))
        .unwrap_or(SourcePosition { line: 1, column: 1 })
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests;
