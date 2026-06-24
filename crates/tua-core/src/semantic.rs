use std::collections::BTreeMap;

use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, Index, Prefix, Suffix,
    TableConstructor, Var,
};
use full_moon::node::Node;

use crate::asset::{self, AssetLoaderKind};
use crate::lua::{self, LuaItem, LuaItemKind};
use crate::module_index::{
    required_module_export_type_for_fields, required_module_exports_for_receiver, ModuleExports,
};
use crate::narrowing::{directly_assigned_name, narrowed_type_at_position};
use crate::types::{FunctionType, PrimitiveType, TableField, Type, TypeEnvironment, TypedFunction};
use crate::{ParsedSource, SourcePosition, SourceSpan, TuaType, TypedParameter};

const MAX_TYPE_RESOLUTION_DEPTH: usize = 64;
const MAX_SEMANTIC_INFERENCE_DEPTH: usize = 32;

pub(crate) struct SemanticModel<'a> {
    parsed: &'a ParsedSource,
    environment: &'a TypeEnvironment,
}

impl<'a> SemanticModel<'a> {
    pub(crate) fn new(parsed: &'a ParsedSource, environment: &'a TypeEnvironment) -> Self {
        Self {
            parsed,
            environment,
        }
    }

    pub(crate) fn type_of_name(&self, name: &str, position: SourcePosition) -> Option<Type> {
        self.type_of_name_at_depth(name, position, 0)
    }

    pub(crate) fn type_of_local_assignment(
        &self,
        name: &str,
        position: SourcePosition,
    ) -> Option<Type> {
        let local = self.local_assignment_value(name, position)?;

        local.expression.and_then(|expression| {
            self.type_of_expression_return_at_depth(
                expression,
                local.span.start,
                local.return_index,
                0,
            )
        })
    }

    pub(crate) fn type_of_expression_return(
        &self,
        expression: &Expression,
        position: SourcePosition,
        return_index: usize,
    ) -> Option<Type> {
        self.type_of_expression_return_at_depth(expression, position, return_index, 0)
    }

    pub(crate) fn type_of_field_chain(
        &self,
        parts: &[String],
        position: SourcePosition,
    ) -> Option<Type> {
        let (root, fields) = parts.split_first()?;

        if let Some(ty) = resolve_field_chain_type(self.environment, parts) {
            return Some(ty);
        }

        if fields.is_empty() {
            return self.type_of_name(root, position);
        }

        if let Some(ty) = required_module_export_type_for_fields(self.parsed, root, fields) {
            return Some(ty);
        }

        let mut current = self.type_of_name(root, position)?;

        for field in fields {
            current = resolve_table_field_type(self.environment, &current, field)?;
        }

        Some(current)
    }

    pub(crate) fn return_types_of_call(
        &self,
        function_call: &FunctionCall,
        position: SourcePosition,
    ) -> Vec<Type> {
        let Some(parts) = field_chain_before_first_call(function_call) else {
            return Vec::new();
        };

        self.return_types_for_call_parts(&parts, position)
    }

    pub(crate) fn return_types_for_call_parts(
        &self,
        parts: &[String],
        position: SourcePosition,
    ) -> Vec<Type> {
        let Some((root, fields)) = parts.split_first() else {
            return Vec::new();
        };

        if fields.is_empty() {
            if let Some(function) = self.environment.function(root) {
                return function.return_types_or_any();
            }

            if let Some(kind) = asset::loader_kind(root) {
                return vec![kind.return_type()];
            }
        }

        if let Some(Type::Function(function)) = self.type_of_field_chain(parts, position) {
            return function.return_types_or_any().to_vec();
        }

        if !fields.is_empty() {
            if let Some(method) = parts.last() {
                if let Some(receiver_parts) = parts.get(..parts.len().saturating_sub(1)) {
                    if let Some(receiver_type) = self.type_of_field_chain(receiver_parts, position)
                    {
                        if let Some(item) = self.lua_type_field_item(&receiver_type, method) {
                            return return_types_from_function_detail(item.detail);
                        }
                    }
                }
            }
        }

        lua::hover_item(parts)
            .map(|item| return_types_from_function_detail(item.detail))
            .unwrap_or_default()
    }

    pub(crate) fn typed_function_for_call(
        &self,
        function_call: &FunctionCall,
        position: SourcePosition,
    ) -> Option<TypedFunction> {
        let parts = field_chain_before_first_call(function_call)?;

        self.typed_function_for_call_parts(&parts, position)
    }

    pub(crate) fn typed_function_for_call_parts(
        &self,
        parts: &[String],
        position: SourcePosition,
    ) -> Option<TypedFunction> {
        let (root, fields) = parts.split_first()?;
        let qualified_name = parts.join(".");

        if let Some(function) = self.environment.function(&qualified_name) {
            return Some(function.clone());
        }

        if fields.is_empty() {
            if let Some(function) = self.environment.function(root) {
                return Some(function.clone());
            }

            if let Some(kind) = asset::loader_kind(root) {
                return Some(typed_function_from_asset_loader(kind));
            }
        }

        if let Some(Type::Function(function)) = self.type_of_field_chain(parts, position) {
            return Some(typed_function_from_function_type(
                Some(qualified_name),
                function,
            ));
        }

        if let Some(item) = self.lua_function_item_for_call_parts(parts, position) {
            return typed_function_from_lua_item(item);
        }

        None
    }

    pub(crate) fn lua_function_item_for_call_parts(
        &self,
        parts: &[String],
        position: SourcePosition,
    ) -> Option<LuaItem> {
        if let Some(item) = lua::hover_item(parts).filter(|item| item.kind == LuaItemKind::Function)
        {
            return Some(item);
        }

        let (method, receiver_parts) = parts.split_last()?;
        let receiver_type = self.type_of_field_chain(receiver_parts, position)?;

        self.lua_type_field_item(&receiver_type, method)
            .filter(|item| item.kind == LuaItemKind::Function)
    }

    pub(crate) fn lua_type_field_item(&self, ty: &Type, field: &str) -> Option<LuaItem> {
        lua_type_field_item(self.environment, ty, field)
    }

    fn type_of_name_at_depth(
        &self,
        name: &str,
        position: SourcePosition,
        depth: usize,
    ) -> Option<Type> {
        if depth >= MAX_SEMANTIC_INFERENCE_DEPTH {
            return None;
        }

        if let Some(base) = self.type_of_name_without_narrowing(name, position, depth) {
            let aliases = self.alias_map();
            if let Some(narrowed) =
                narrowed_type_at_position(self.parsed, &aliases, name, position, |lookup_name| {
                    self.type_of_name_without_narrowing(lookup_name, position, depth + 1)
                })
            {
                return Some(narrowed);
            }

            return Some(base);
        }

        None
    }

    fn type_of_name_without_narrowing(
        &self,
        name: &str,
        position: SourcePosition,
        depth: usize,
    ) -> Option<Type> {
        if depth >= MAX_SEMANTIC_INFERENCE_DEPTH {
            return None;
        }

        self.environment
            .local(name)
            .cloned()
            .or_else(|| {
                self.environment
                    .function(name)
                    .and_then(TypedFunction::as_type)
            })
            .or_else(|| {
                required_module_exports_for_receiver(self.parsed, name)
                    .map(module_exports_table_type)
            })
            .or_else(|| {
                let local = self.local_assignment_value(name, position)?;

                local.expression.and_then(|expression| {
                    self.type_of_expression_return_at_depth(
                        expression,
                        local.span.start,
                        local.return_index,
                        depth + 1,
                    )
                })
            })
            .or_else(|| self.parameter_type_at_position(name, position))
    }

    fn alias_map(&self) -> BTreeMap<String, Type> {
        self.environment
            .aliases()
            .map(|(name, ty)| (name.to_owned(), ty.clone()))
            .collect()
    }

    fn type_of_expression_return_at_depth(
        &self,
        expression: &Expression,
        position: SourcePosition,
        return_index: usize,
        depth: usize,
    ) -> Option<Type> {
        if depth >= MAX_SEMANTIC_INFERENCE_DEPTH {
            return None;
        }

        match expression {
            Expression::FunctionCall(function_call) => self
                .return_types_of_call(function_call, position)
                .get(return_index)
                .cloned(),
            _ if return_index > 0 => None,
            Expression::BinaryOperator { lhs, binop, rhs } => {
                let left = self.type_of_expression_return_at_depth(lhs, position, 0, depth + 1)?;
                let right = self.type_of_expression_return_at_depth(rhs, position, 0, depth + 1)?;

                infer_binary_operator_type(self.environment, binop, &left, &right)
            }
            Expression::Number(_) => Some(Type::number()),
            Expression::String(_) => Some(Type::string()),
            Expression::Symbol(symbol) => infer_symbol_type(&token_text(symbol)),
            Expression::TableConstructor(table) => {
                Some(self.type_of_table_constructor(table, position, depth + 1))
            }
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. } => self
                .type_of_expression_return_at_depth(expression, position, return_index, depth + 1),
            Expression::Var(var) => self.type_of_var_at_depth(var, position, depth + 1),
            _ => None,
        }
    }

    fn type_of_table_constructor(
        &self,
        table: &TableConstructor,
        position: SourcePosition,
        depth: usize,
    ) -> Type {
        Type::table(table.fields().iter().filter_map(|field| match field {
            Field::NameKey { key, value, .. } => {
                let ty = self
                    .type_of_expression_return_at_depth(value, position, 0, depth + 1)
                    .unwrap_or(Type::Any);

                Some((token_text(key), TableField::required(ty)))
            }
            _ => None,
        }))
    }

    fn type_of_var_at_depth(
        &self,
        var: &Var,
        position: SourcePosition,
        depth: usize,
    ) -> Option<Type> {
        if depth >= MAX_SEMANTIC_INFERENCE_DEPTH {
            return None;
        }

        match var {
            Var::Name(name) => self.type_of_name_at_depth(&token_text(name), position, depth + 1),
            Var::Expression(expression) => self.type_of_field_chain(
                &plain_field_chain_from_var_expression(expression)?,
                position,
            ),
            _ => None,
        }
    }

    fn local_assignment_value(
        &self,
        name: &str,
        position: SourcePosition,
    ) -> Option<LocalAssignmentValue<'a>> {
        local_assignment_value_in_block(self.parsed.ast.nodes(), name, position)
    }

    fn parameter_type_at_position(&self, name: &str, position: SourcePosition) -> Option<Type> {
        self.parsed
            .function_annotations
            .iter()
            .filter_map(|function| {
                let span = function.span?;

                contains_position(span, position).then_some(function)
            })
            .filter_map(|function| {
                function
                    .parameters
                    .iter()
                    .find(|parameter| parameter.name == name)
            })
            .next_back()
            .map(|parameter| type_from_tua_type(&parameter.ty))
    }
}

#[derive(Clone, Copy, Debug)]
struct LocalAssignmentValue<'a> {
    span: SourceSpan,
    expression: Option<&'a Expression>,
    return_index: usize,
}

pub(crate) fn resolve_field_chain_type(
    environment: &TypeEnvironment,
    parts: &[String],
) -> Option<Type> {
    let (root, fields) = parts.split_first()?;
    let mut current = environment.local(root)?.clone();

    for field in fields {
        current = resolve_table_field_type(environment, &current, field)?;
    }

    Some(current)
}

pub(crate) fn resolve_table_field_type(
    environment: &TypeEnvironment,
    ty: &Type,
    field: &str,
) -> Option<Type> {
    match resolve_aliases(environment, ty, 0) {
        Type::Table(table) => table.field(field).map(|field| field.ty.clone()),
        Type::Union(types) => {
            let mut resolved = Vec::new();

            for ty in types {
                if let Some(field_type) = resolve_table_field_type(environment, &ty, field) {
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

pub(crate) fn resolve_aliases(environment: &TypeEnvironment, ty: &Type, depth: usize) -> Type {
    if depth >= MAX_TYPE_RESOLUTION_DEPTH {
        return ty.clone();
    }

    match ty {
        Type::Alias(name) => environment
            .alias(name)
            .map(|resolved| resolve_aliases(environment, resolved, depth + 1))
            .unwrap_or_else(|| ty.clone()),
        _ => ty.clone(),
    }
}

fn module_exports_table_type(exports: ModuleExports) -> Type {
    Type::table(
        exports
            .fields
            .into_iter()
            .map(|(name, field)| (name, TableField::required(field.ty))),
    )
}

fn local_assignment_value_in_block<'a>(
    block: &'a ast::Block,
    name: &str,
    position: SourcePosition,
) -> Option<LocalAssignmentValue<'a>> {
    let mut candidates = Vec::new();

    collect_local_assignment_values(block, name, position, &mut candidates);

    candidates
        .into_iter()
        .max_by_key(|candidate| position_key(candidate.span.start))
}

fn collect_local_assignment_values<'a>(
    block: &'a ast::Block,
    name: &str,
    position: SourcePosition,
    candidates: &mut Vec<LocalAssignmentValue<'a>>,
) {
    for stmt in block.stmts() {
        if !starts_before_or_at(node_start(stmt), position) {
            continue;
        }

        match stmt {
            ast::Stmt::Assignment(assignment) => {
                let expressions = assignment.expressions().iter().collect::<Vec<_>>();

                for (index, variable) in assignment.variables().iter().enumerate() {
                    let Some(span) = node_span(variable) else {
                        continue;
                    };

                    if directly_assigned_name(variable).as_deref() != Some(name)
                        || !starts_before_or_at(span.start, position)
                    {
                        continue;
                    }

                    let (expression, return_index) =
                        assignment_expression_for_local(&expressions, index)
                            .map(|(expression, return_index)| (Some(expression), return_index))
                            .unwrap_or((None, 0));

                    candidates.push(LocalAssignmentValue {
                        span,
                        expression,
                        return_index,
                    });
                }
            }
            ast::Stmt::LocalAssignment(local) => {
                let expressions = local.expressions().iter().collect::<Vec<_>>();

                for (index, local_name) in local.names().iter().enumerate() {
                    let Some(span) = node_span(local_name) else {
                        continue;
                    };

                    if token_text(local_name) != name || !starts_before_or_at(span.start, position)
                    {
                        continue;
                    }

                    let (expression, return_index) =
                        assignment_expression_for_local(&expressions, index)
                            .map(|(expression, return_index)| (Some(expression), return_index))
                            .unwrap_or((None, 0));

                    candidates.push(LocalAssignmentValue {
                        span,
                        expression,
                        return_index,
                    });
                }
            }
            ast::Stmt::Do(block) => {
                collect_local_assignment_values(block.block(), name, position, candidates);
            }
            ast::Stmt::FunctionDeclaration(function) => {
                collect_local_assignment_values(
                    function.body().block(),
                    name,
                    position,
                    candidates,
                );
            }
            ast::Stmt::GenericFor(for_stmt) => {
                collect_local_assignment_values(for_stmt.block(), name, position, candidates);
            }
            ast::Stmt::If(if_stmt) => {
                collect_local_assignment_values(if_stmt.block(), name, position, candidates);

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        collect_local_assignment_values(
                            else_if.block(),
                            name,
                            position,
                            candidates,
                        );
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    collect_local_assignment_values(else_block, name, position, candidates);
                }
            }
            ast::Stmt::LocalFunction(function) => {
                collect_local_assignment_values(
                    function.body().block(),
                    name,
                    position,
                    candidates,
                );
            }
            ast::Stmt::NumericFor(for_stmt) => {
                collect_local_assignment_values(for_stmt.block(), name, position, candidates);
            }
            ast::Stmt::Repeat(repeat) => {
                collect_local_assignment_values(repeat.block(), name, position, candidates);
            }
            ast::Stmt::While(while_stmt) => {
                collect_local_assignment_values(while_stmt.block(), name, position, candidates);
            }
            _ => {}
        }
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

fn type_from_tua_type(ty: &TuaType) -> Type {
    match ty {
        TuaType::Named(name) => Type::named(name.clone()),
        TuaType::Union(types) => Type::union(types.iter().map(type_from_tua_type)),
        TuaType::Table(fields) => Type::table(fields.iter().map(|field| {
            (
                field.name.clone(),
                TableField {
                    ty: type_from_tua_type(&field.ty),
                    optional: field.optional,
                },
            )
        })),
    }
}

fn contains_position(span: SourceSpan, position: SourcePosition) -> bool {
    let start = position_key(span.start);
    let end = position_key(span.end);
    let position = position_key(position);

    start <= position && position < end
}

pub(crate) fn plain_field_chain_from_var_expression(
    expression: &ast::VarExpression,
) -> Option<Vec<String>> {
    let Prefix::Name(name) = expression.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in expression.suffixes() {
        let Suffix::Index(Index::Dot { name, .. }) = suffix else {
            return None;
        };

        parts.push(token_text(name));
    }

    Some(parts)
}

pub(crate) fn field_chain_before_first_call(function_call: &FunctionCall) -> Option<Vec<String>> {
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
            Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { .. }))
            | Suffix::Call(Call::AnonymousCall(FunctionArgs::String(_)))
            | Suffix::Call(Call::AnonymousCall(FunctionArgs::TableConstructor(_))) => {
                return Some(parts);
            }
            _ => return None,
        }
    }

    None
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

fn typed_function_from_asset_loader(kind: AssetLoaderKind) -> TypedFunction {
    let return_type = kind.return_type();

    TypedFunction {
        name: Some(kind.name().to_owned()),
        parameters: vec![TypedParameter {
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

fn typed_function_from_function_type(
    name: Option<String>,
    function: FunctionType,
) -> TypedFunction {
    let return_types = function.return_types_or_any().to_vec();

    TypedFunction {
        name,
        parameters: function
            .parameters
            .into_iter()
            .map(|ty| TypedParameter {
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

fn parameters_from_function_detail(detail: &str) -> Option<Vec<TypedParameter>> {
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

                TypedParameter {
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
        Type::Primitive(PrimitiveType::Number) => true,
        Type::Union(types) => types.iter().all(|ty| is_number_type(environment, ty)),
        _ => false,
    }
}

fn is_concat_operand_type(environment: &TypeEnvironment, ty: &Type) -> bool {
    match resolve_aliases(environment, ty, 0) {
        Type::Primitive(PrimitiveType::Number | PrimitiveType::String) => true,
        Type::Union(types) => types
            .iter()
            .all(|ty| is_concat_operand_type(environment, ty)),
        _ => false,
    }
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    let (start, end) = node.range()?;

    Some(SourceSpan {
        start: SourcePosition::from_full_moon(start),
        end: SourcePosition::from_full_moon(end),
    })
}

fn node_start(node: &impl Node) -> SourcePosition {
    node.range()
        .map(|(start, _)| SourcePosition::from_full_moon(start))
        .unwrap_or(SourcePosition { line: 1, column: 1 })
}

fn starts_before_or_at(left: SourcePosition, right: SourcePosition) -> bool {
    (left.line, left.column) <= (right.line, right.column)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}
