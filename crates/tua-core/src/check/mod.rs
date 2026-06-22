use std::collections::BTreeMap;
use std::path::Path;

use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, Index, LastStmt, Prefix, Return,
    Suffix, TableConstructor, Var,
};
use full_moon::node::Node;

use crate::config::DiagnosticsConfig;
use crate::lint::lint_parsed_source;
use crate::parser::{parse_tua_source, ParsedSource, SourcePosition};
use crate::types::{
    build_type_environment, FunctionType, TableField, Type, TypeContext, TypeEnvironment,
    TypedFunction,
};
use crate::{asset, lua};
use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity};

const MAX_TYPE_RESOLUTION_DEPTH: usize = 64;

pub fn check_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<(), Vec<Diagnostic>> {
    let parsed = parse_tua_source(path, source)?;

    check_parsed_source(&parsed)
}

pub fn check_tua_source_with_diagnostics_config(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    diagnostics_config: &DiagnosticsConfig,
) -> Result<(), Vec<Diagnostic>> {
    let parsed = parse_tua_source(path, source)?;

    check_parsed_source_with_diagnostics_config(&parsed, diagnostics_config)
}

pub fn check_parsed_source(parsed: &ParsedSource) -> Result<(), Vec<Diagnostic>> {
    check_parsed_source_with_diagnostics_config(parsed, &DiagnosticsConfig::default())
}

pub fn check_parsed_source_with_diagnostics_config(
    parsed: &ParsedSource,
    diagnostics_config: &DiagnosticsConfig,
) -> Result<(), Vec<Diagnostic>> {
    let environment = build_type_environment(parsed)?;
    let mut checker = Checker::new(parsed, &environment);

    checker.check_block(parsed.ast.nodes(), None);
    checker
        .diagnostics
        .extend(lint_parsed_source(parsed, diagnostics_config));

    if diagnostics_config.doc_annotations {
        checker
            .diagnostics
            .extend(parsed.lua_doc_diagnostics.iter().cloned());
    }

    if checker.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(checker.diagnostics)
    }
}

struct Checker<'a> {
    parsed: &'a ParsedSource,
    environment: &'a TypeEnvironment,
    context: TypeContext,
    scoped_locals: Vec<BTreeMap<String, Type>>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Checker<'a> {
    fn new(parsed: &'a ParsedSource, environment: &'a TypeEnvironment) -> Self {
        Self {
            parsed,
            environment,
            context: environment.to_type_context(),
            scoped_locals: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn check_block(&mut self, block: &ast::Block, current_return: Option<&[Type]>) {
        self.scoped_locals.push(BTreeMap::new());

        for stmt in block.stmts() {
            self.check_stmt(stmt, current_return);
        }

        if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
            self.check_return(return_stmt, current_return);
        }

        self.scoped_locals.pop();
    }

    fn check_stmt(&mut self, stmt: &ast::Stmt, current_return: Option<&[Type]>) {
        match stmt {
            ast::Stmt::Assignment(assignment) => self.check_assignment(assignment),
            ast::Stmt::FunctionCall(function_call) => {
                self.check_function_call(function_call);
            }
            ast::Stmt::LocalAssignment(local) => self.check_local_assignment(local),
            ast::Stmt::Do(block) => self.check_block(block.block(), current_return),
            ast::Stmt::If(if_stmt) => {
                self.check_condition_expression(if_stmt.condition());
                self.check_block(if_stmt.block(), current_return);
                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        self.check_condition_expression(else_if.condition());
                        self.check_block(else_if.block(), current_return);
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    self.check_block(else_block, current_return);
                }
            }
            ast::Stmt::GenericFor(for_stmt) => self.check_block(for_stmt.block(), current_return),
            ast::Stmt::LocalFunction(function) => {
                let name = token_text(function.name());
                let return_types = self.function_return_types(&name);
                let scoped_locals = self.function_parameter_locals(&name);

                self.scoped_locals.push(scoped_locals);
                self.check_block(function.body().block(), return_types.as_deref());
                self.scoped_locals.pop();
            }
            ast::Stmt::FunctionDeclaration(function) => {
                let name = function.name().to_string();
                let return_types = self.function_return_types(&name);
                let scoped_locals = self.function_parameter_locals(&name);

                self.scoped_locals.push(scoped_locals);
                self.check_block(function.body().block(), return_types.as_deref());
                self.scoped_locals.pop();
            }
            ast::Stmt::NumericFor(for_stmt) => self.check_block(for_stmt.block(), current_return),
            ast::Stmt::Repeat(repeat) => {
                self.check_condition_expression(repeat.until());
                self.check_block(repeat.block(), current_return);
            }
            ast::Stmt::While(while_stmt) => {
                self.check_condition_expression(while_stmt.condition());
                self.check_block(while_stmt.block(), current_return);
            }
            _ => {}
        }
    }

    fn check_condition_expression(&mut self, expression: &Expression) {
        match expression {
            Expression::BinaryOperator { lhs, binop, rhs } => {
                if is_equality_operator(binop) {
                    self.check_equality_comparison(lhs, binop, rhs);
                }

                self.check_condition_expression(lhs);
                self.check_condition_expression(rhs);
            }
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. } => {
                self.check_condition_expression(expression);
            }
            Expression::FunctionCall(function_call) => {
                self.check_function_call(function_call);
            }
            Expression::TableConstructor(table) => {
                for field in table.fields() {
                    if let Field::NameKey { value, .. } = field {
                        self.check_condition_expression(value);
                    }
                }
            }
            _ => {}
        }
    }

    fn check_equality_comparison(
        &mut self,
        lhs: &Expression,
        binop: &ast::BinOp,
        rhs: &Expression,
    ) {
        let Some(left) = self.infer_expression(lhs) else {
            return;
        };
        let Some(right) = self.infer_expression(rhs) else {
            return;
        };

        if self.types_can_overlap(&left, &right) {
            return;
        }

        self.diagnostics.push(impossible_comparison_diagnostic(
            &self.parsed.path,
            binop,
            &left,
            &right,
        ));
    }

    fn check_local_assignment(&mut self, local: &ast::LocalAssignment) {
        for ((name, specifier), expression) in local
            .names()
            .iter()
            .zip(local.type_specifiers())
            .zip(local.expressions().iter())
        {
            if specifier.is_none() {
                continue;
            }

            let name = token_text(name);
            let Some(target) = self.environment.local(&name).cloned() else {
                continue;
            };

            self.check_expression_assignable(expression, &target);
        }

        self.record_local_assignment_types(local);
    }

    fn record_local_assignment_types(&mut self, local: &ast::LocalAssignment) {
        let expressions = local.expressions().iter().collect::<Vec<_>>();
        let specifiers = local.type_specifiers().collect::<Vec<_>>();

        for (index, name) in local.names().iter().enumerate() {
            let name = token_text(name);
            let ty = specifiers
                .get(index)
                .and_then(|specifier| *specifier)
                .and_then(|_| self.environment.local(&name).cloned())
                .or_else(|| {
                    assignment_expression_for_local(&expressions, index).and_then(
                        |(expression, return_index)| {
                            self.infer_expression_return(expression, return_index)
                        },
                    )
                });

            if let Some(ty) = ty {
                self.insert_scoped_local(name, ty);
            }
        }
    }

    fn insert_scoped_local(&mut self, name: String, ty: Type) {
        if let Some(scope) = self.scoped_locals.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn check_assignment(&mut self, assignment: &ast::Assignment) {
        for (variable, expression) in assignment.variables().iter().zip(assignment.expressions()) {
            let Some(target) = self.infer_var_assignment_target(variable) else {
                continue;
            };

            self.check_expression_assignable(expression, &target);
        }
    }

    fn check_expression_assignable(&mut self, expression: &Expression, target: &Type) {
        let Some(source) = self.infer_expression(expression) else {
            return;
        };

        self.check_type_assignable(expression, &source, target);
    }

    fn check_type_assignable(&mut self, node: &impl Node, source: &Type, target: &Type) {
        if !self.context.is_assignable(source, target) {
            self.diagnostics
                .push(mismatch_diagnostic(&self.parsed.path, node, target, source));
        }
    }

    fn check_return(&mut self, return_stmt: &Return, target: Option<&[Type]>) {
        let Some(target) = target else {
            return;
        };

        let expressions = return_stmt.returns().iter().collect::<Vec<_>>();

        if expressions.is_empty() {
            self.check_type_assignable(
                return_stmt,
                &Type::nil(),
                target.first().unwrap_or(&Type::nil()),
            );
            return;
        }

        for (index, target) in target.iter().enumerate() {
            let source = assignment_expression_for_local(&expressions, index)
                .and_then(|(expression, return_index)| {
                    self.infer_expression_return(expression, return_index)
                })
                .unwrap_or_else(Type::nil);

            self.check_type_assignable(return_stmt, &source, target);
        }
    }

    fn infer_expression(&mut self, expression: &Expression) -> Option<Type> {
        self.infer_expression_return(expression, 0)
    }

    fn infer_expression_return(
        &mut self,
        expression: &Expression,
        return_index: usize,
    ) -> Option<Type> {
        match expression {
            Expression::FunctionCall(function_call) => self
                .check_function_call_returns(function_call)
                .get(return_index)
                .cloned(),
            _ if return_index > 0 => None,
            Expression::BinaryOperator { lhs, binop, rhs } => {
                let left = self.infer_expression(lhs)?;
                let right = self.infer_expression(rhs)?;

                self.infer_binary_operator_type(binop, &left, &right)
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

    fn infer_table_constructor(&mut self, table: &TableConstructor) -> Type {
        Type::table(table.fields().iter().filter_map(|field| match field {
            Field::NameKey { key, value, .. } => {
                let ty = self.infer_expression(value).unwrap_or(Type::Any);

                Some((token_text(key), TableField::required(ty)))
            }
            _ => None,
        }))
    }

    fn infer_var_value(&mut self, var: &Var) -> Option<Type> {
        match var {
            Var::Name(name) => self.infer_named_value(&token_text(name)),
            Var::Expression(expression) => {
                self.resolve_field_chain_type(&plain_field_chain_from_var_expression(expression)?)
            }
            _ => None,
        }
    }

    fn infer_named_value(&self, name: &str) -> Option<Type> {
        self.scoped_locals
            .iter()
            .rev()
            .find_map(|locals| locals.get(name).cloned())
            .or_else(|| self.environment.local(name).cloned())
            .or_else(|| {
                self.environment
                    .function(name)
                    .and_then(TypedFunction::as_type)
            })
    }

    fn check_function_call(&mut self, function_call: &FunctionCall) -> Option<Type> {
        self.check_function_call_returns(function_call)
            .into_iter()
            .next()
    }

    fn check_function_call_returns(&mut self, function_call: &FunctionCall) -> Vec<Type> {
        let Some(callee) = self.infer_call_target(function_call) else {
            return Vec::new();
        };

        match callee {
            Callable::Any => vec![Type::Any],
            Callable::Function(function) => {
                self.check_call_arguments(function_call, &function);

                function.return_types_or_any().to_vec()
            }
            Callable::ExternalFunction(function) => {
                self.check_external_call_arguments(function_call, &function);

                function.return_types
            }
        }
    }

    fn infer_call_target(&self, function_call: &FunctionCall) -> Option<Callable> {
        let parts = field_chain_before_first_call(function_call)?;
        let (root, fields) = parts.split_first()?;

        if fields.is_empty() {
            if let Some(local) = self.environment.local(root) {
                return callable_from_type(local, self.environment);
            }

            if let Some(function) = self.environment.function(root) {
                return Some(Callable::from_typed_function(function));
            }

            if let Some(kind) = asset::loader_kind(root) {
                return Some(Callable::Function(kind.function_type()));
            }
        }

        if !fields.is_empty() {
            if let Some(function) = self.love_object_method_call_target(&parts) {
                return Some(Callable::ExternalFunction(function));
            }
        }

        let callee_type = self.resolve_field_chain_type(&parts)?;

        callable_from_type(&callee_type, self.environment)
    }

    fn love_object_method_call_target(&self, parts: &[String]) -> Option<ExternalFunctionType> {
        let method = parts.last()?;
        let receiver_parts = parts.get(..parts.len().checked_sub(1)?)?;
        let receiver_type = self.resolve_field_chain_type(receiver_parts)?;
        let item = lua_type_field_item(self.environment, &receiver_type, method)?;

        external_function_type_from_lua_item(item)
    }

    fn check_call_arguments(&mut self, function_call: &FunctionCall, function: &FunctionType) {
        let arguments = call_arguments(function_call).unwrap_or_default();

        if arguments.len() != function.parameters.len() {
            self.diagnostics.push(argument_count_diagnostic(
                &self.parsed.path,
                function_call,
                function.parameters.len(),
                arguments.len(),
            ));
            return;
        }

        for (argument, parameter) in arguments.into_iter().zip(&function.parameters) {
            match argument {
                CallArgument::Expression(argument) => {
                    self.check_expression_assignable(argument, parameter);
                }
                CallArgument::Inferred(argument_type) => {
                    if !self.context.is_assignable(&argument_type, parameter) {
                        self.diagnostics.push(mismatch_diagnostic(
                            &self.parsed.path,
                            function_call,
                            parameter,
                            &argument_type,
                        ));
                    }
                }
            }
        }
    }

    fn check_external_call_arguments(
        &mut self,
        function_call: &FunctionCall,
        function: &ExternalFunctionType,
    ) {
        let arguments = call_arguments(function_call).unwrap_or_default();
        let required_parameter_count = function.required_parameter_count();

        if arguments.len() < required_parameter_count {
            self.diagnostics.push(argument_count_diagnostic(
                &self.parsed.path,
                function_call,
                required_parameter_count,
                arguments.len(),
            ));
            return;
        }

        if arguments.len() > function.parameters.len() {
            self.diagnostics.push(argument_count_diagnostic(
                &self.parsed.path,
                function_call,
                function.parameters.len(),
                arguments.len(),
            ));
            return;
        }

        for (argument, parameter) in arguments.into_iter().zip(&function.parameters) {
            match argument {
                CallArgument::Expression(argument) => {
                    let Some(argument_type) = self.infer_expression(argument) else {
                        continue;
                    };

                    self.check_external_argument_type(argument, &argument_type, &parameter.ty);
                }
                CallArgument::Inferred(argument_type) => {
                    self.check_external_argument_type(function_call, &argument_type, &parameter.ty);
                }
            }
        }
    }

    fn check_external_argument_type(&mut self, node: &impl Node, source: &Type, target: &Type) {
        if self.external_argument_assignable(source, target) {
            return;
        }

        self.diagnostics
            .push(mismatch_diagnostic(&self.parsed.path, node, target, source));
    }

    fn external_argument_assignable(&self, source: &Type, target: &Type) -> bool {
        self.context.is_assignable(source, target)
            || matches!(source, Type::Primitive(crate::PrimitiveType::String))
                && matches!(target, Type::Alias(name) if self.context.alias(name).is_none())
    }

    fn function_return_types(&self, name: &str) -> Option<Vec<Type>> {
        self.environment
            .function(name.trim())
            .map(|function| function.return_types_or_any())
    }

    fn infer_var_assignment_target(&self, var: &Var) -> Option<Type> {
        match var {
            Var::Name(name) => self.environment.local(&token_text(name)).cloned(),
            Var::Expression(expression) => {
                self.resolve_field_chain_type(&plain_field_chain_from_var_expression(expression)?)
            }
            _ => None,
        }
    }

    fn resolve_field_chain_type(&self, parts: &[String]) -> Option<Type> {
        let (root, fields) = parts.split_first()?;
        let mut current = self.infer_named_value(root)?;

        for field in fields {
            current = self.resolve_table_field_type(&current, field)?;
        }

        Some(current)
    }

    fn resolve_table_field_type(&self, ty: &Type, field: &str) -> Option<Type> {
        match resolve_aliases(ty, self.environment, 0) {
            Type::Table(table) => table.field(field).map(|field| field.ty.clone()),
            Type::Union(types) => {
                let mut resolved = Vec::new();

                for ty in types {
                    if let Some(field_type) = self.resolve_table_field_type(&ty, field) {
                        resolved.push(field_type);
                    }
                }

                match resolved.len() {
                    0 => None,
                    1 => resolved.into_iter().next(),
                    _ => Some(Type::union(resolved)),
                }
            }
            _ => None,
        }
    }

    fn function_parameter_locals(&self, name: &str) -> BTreeMap<String, Type> {
        self.environment
            .function(name.trim())
            .map(|function| {
                function
                    .parameters
                    .iter()
                    .map(|parameter| (parameter.name.clone(), parameter.ty.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn types_can_overlap(&self, left: &Type, right: &Type) -> bool {
        types_can_overlap(
            &resolve_aliases(left, self.environment, 0),
            &resolve_aliases(right, self.environment, 0),
        )
    }

    fn infer_binary_operator_type(
        &self,
        binop: &ast::BinOp,
        left: &Type,
        right: &Type,
    ) -> Option<Type> {
        if is_numeric_operator(binop) && self.is_number_type(left) && self.is_number_type(right) {
            return Some(Type::number());
        }

        if matches!(binop, ast::BinOp::TwoDots(_))
            && self.is_concat_operand_type(left)
            && self.is_concat_operand_type(right)
        {
            return Some(Type::string());
        }

        if matches!(left, Type::Any) || matches!(right, Type::Any) {
            return Some(Type::Any);
        }

        None
    }

    fn is_number_type(&self, ty: &Type) -> bool {
        match resolve_aliases(ty, self.environment, 0) {
            Type::Primitive(crate::PrimitiveType::Number) => true,
            Type::Union(types) => types.iter().all(|ty| self.is_number_type(ty)),
            _ => false,
        }
    }

    fn is_concat_operand_type(&self, ty: &Type) -> bool {
        match resolve_aliases(ty, self.environment, 0) {
            Type::Primitive(crate::PrimitiveType::Number | crate::PrimitiveType::String) => true,
            Type::Union(types) => types.iter().all(|ty| self.is_concat_operand_type(ty)),
            _ => false,
        }
    }
}

enum Callable {
    Any,
    Function(FunctionType),
    ExternalFunction(ExternalFunctionType),
}

enum CallArgument<'a> {
    Expression(&'a Expression),
    Inferred(Type),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExternalFunctionType {
    parameters: Vec<ExternalParameter>,
    return_types: Vec<Type>,
}

impl ExternalFunctionType {
    fn required_parameter_count(&self) -> usize {
        self.parameters
            .iter()
            .filter(|parameter| !parameter.optional)
            .count()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExternalParameter {
    ty: Type,
    optional: bool,
}

impl Callable {
    fn from_typed_function(function: &TypedFunction) -> Self {
        if function.has_fully_annotated_parameters() {
            Self::Function(FunctionType::new_returns(
                function
                    .parameters
                    .iter()
                    .map(|parameter| parameter.ty.clone()),
                function.return_types_or_any(),
            ))
        } else {
            Self::Any
        }
    }
}

fn infer_symbol_type(symbol: &str) -> Option<Type> {
    match symbol {
        "true" | "false" => Some(Type::boolean()),
        "nil" => Some(Type::nil()),
        _ => None,
    }
}

fn is_equality_operator(binop: &ast::BinOp) -> bool {
    matches!(binop, ast::BinOp::TwoEqual(_) | ast::BinOp::TildeEqual(_))
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

fn types_can_overlap(left: &Type, right: &Type) -> bool {
    match (left, right) {
        (Type::Any, _) | (_, Type::Any) => true,
        (Type::Primitive(left), Type::Primitive(right)) => left == right,
        (Type::Union(types), other) | (other, Type::Union(types)) => {
            types.iter().any(|ty| types_can_overlap(ty, other))
        }
        (Type::Function(_), Type::Function(_)) => true,
        (Type::Table(_), Type::Table(_)) => true,
        (Type::Alias(_), _) | (_, Type::Alias(_)) => true,
        (
            Type::Primitive(_) | Type::Function(_) | Type::Table(_),
            Type::Primitive(_) | Type::Function(_) | Type::Table(_),
        ) => false,
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

fn lua_type_field_item(
    environment: &TypeEnvironment,
    ty: &Type,
    field: &str,
) -> Option<lua::LuaItem> {
    match resolve_aliases(ty, environment, 0) {
        Type::Alias(name) => lua::type_field_items(&name, field)
            .into_iter()
            .find(|item| item.label == field),
        Type::Union(types) => types
            .iter()
            .find_map(|ty| lua_type_field_item(environment, ty, field)),
        Type::Any | Type::Function(_) | Type::Primitive(_) | Type::Table(_) => None,
    }
}

fn external_function_type_from_lua_item(item: lua::LuaItem) -> Option<ExternalFunctionType> {
    let parameters = parameters_from_function_detail(item.detail)?;
    let mut return_types = return_types_from_function_detail(item.detail);

    if return_types.is_empty() {
        return_types.push(Type::Any);
    }

    Some(ExternalFunctionType {
        parameters,
        return_types,
    })
}

fn parameters_from_function_detail(detail: &str) -> Option<Vec<ExternalParameter>> {
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
                    .map(|(name, ty)| (name.trim(), ty.trim()))
                    .unwrap_or(("", parameter));

                ExternalParameter {
                    ty: type_from_lua_label(ty).unwrap_or(Type::Any),
                    optional: name.ends_with('?'),
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
        .filter_map(type_from_lua_label)
        .collect()
}

fn type_from_lua_label(label: &str) -> Option<Type> {
    let label = label.trim();

    if label.contains('|') {
        let members = label
            .split('|')
            .filter_map(type_from_lua_label)
            .collect::<Vec<_>>();

        return match members.len() {
            0 => None,
            1 => members.into_iter().next(),
            _ => Some(Type::union(members)),
        };
    }

    match label {
        "any" => Some(Type::Any),
        "boolean" => Some(Type::boolean()),
        "nil" => Some(Type::nil()),
        "number" => Some(Type::number()),
        "string" => Some(Type::string()),
        "table" => Some(Type::empty_table()),
        other if !other.is_empty() => Some(Type::alias(other)),
        _ => None,
    }
}

fn call_arguments(function_call: &FunctionCall) -> Option<Vec<CallArgument<'_>>> {
    let call = function_call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;

    match call {
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            Some(arguments.iter().map(CallArgument::Expression).collect())
        }
        Call::AnonymousCall(FunctionArgs::String(_)) => {
            Some(vec![CallArgument::Inferred(Type::string())])
        }
        Call::MethodCall(method) => match method.args() {
            FunctionArgs::Parentheses { arguments, .. } => {
                Some(arguments.iter().map(CallArgument::Expression).collect())
            }
            FunctionArgs::String(_) => Some(vec![CallArgument::Inferred(Type::string())]),
            FunctionArgs::TableConstructor(_) => {
                Some(vec![CallArgument::Inferred(Type::empty_table())])
            }
            _ => None,
        },
        _ => None,
    }
}

fn callable_from_type(ty: &Type, environment: &TypeEnvironment) -> Option<Callable> {
    match resolve_aliases(ty, environment, 0) {
        Type::Any => Some(Callable::Any),
        Type::Function(function) => Some(Callable::Function(function)),
        _ => None,
    }
}

fn resolve_aliases(ty: &Type, environment: &TypeEnvironment, depth: usize) -> Type {
    if depth >= MAX_TYPE_RESOLUTION_DEPTH {
        return ty.clone();
    }

    match ty {
        Type::Alias(name) => environment
            .alias(name)
            .map(|resolved| resolve_aliases(resolved, environment, depth + 1))
            .unwrap_or_else(|| ty.clone()),
        _ => ty.clone(),
    }
}

fn argument_count_diagnostic(
    path: &Path,
    node: &impl Node,
    expected: usize,
    found: usize,
) -> Diagnostic {
    let position = node_position(node);

    Diagnostic::error(
        path,
        position.line,
        position.column,
        format!("argument count mismatch: expected {expected}, found {found}"),
    )
    .with_code(DiagnosticCode::TYPE_ARGUMENT_COUNT)
}

fn mismatch_diagnostic(path: &Path, node: &impl Node, expected: &Type, found: &Type) -> Diagnostic {
    let position = node_position(node);

    Diagnostic::error(
        path,
        position.line,
        position.column,
        format!("type mismatch: expected {expected}, found {found}"),
    )
    .with_code(DiagnosticCode::TYPE_MISMATCH)
}

fn impossible_comparison_diagnostic(
    path: &Path,
    binop: &ast::BinOp,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let position = binop
        .token()
        .start_position()
        .map(SourcePosition::from_full_moon)
        .unwrap_or(SourcePosition { line: 1, column: 1 });
    let result = match binop {
        ast::BinOp::TwoEqual(_) => "false",
        ast::BinOp::TildeEqual(_) => "true",
        _ => unreachable!("only equality operators produce comparison diagnostics"),
    };

    Diagnostic::new(
        path,
        position.line,
        position.column,
        DiagnosticSeverity::Warning,
        format!("comparison is always {result}: {left} and {right} do not overlap"),
    )
    .with_code(DiagnosticCode::TYPE_IMPOSSIBLE_COMPARISON)
}

fn node_position(node: &impl Node) -> SourcePosition {
    node.range()
        .map(|(start, _)| SourcePosition::from_full_moon(start))
        .unwrap_or(SourcePosition { line: 1, column: 1 })
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

#[cfg(test)]
mod tests;
