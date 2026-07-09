use std::collections::BTreeMap;

use full_moon::ast::luau::TypeInfo;
use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, Index, LastStmt, Prefix, Suffix,
    TableConstructor, Var,
};
use full_moon::node::Node;

use crate::asset::{self, AssetLoaderKind};
use crate::lua::{self, LuaItem, LuaItemKind};
use crate::module_index::{
    required_module_export_for_fields, required_module_export_type_for_fields,
    required_module_exports_for_receiver, ModuleExportField, ModuleExports,
};
use crate::narrowing::{directly_assigned_name, narrowed_type_at_position};
use crate::types::{
    FunctionType, MetatableFacts, PrimitiveType, TableField, Type, TypeEnvironment, TypedFunction,
};
use crate::{vector, ParsedSource, SourcePosition, SourceSpan, TuaType, TypedParameter};

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

    pub(crate) fn type_of_expression_at_position(
        &self,
        position: SourcePosition,
    ) -> Option<(Type, SourceSpan)> {
        let mut candidates = Vec::new();
        self.collect_expression_types_in_block(self.parsed.ast.nodes(), position, &mut candidates);

        candidates
            .into_iter()
            .min_by_key(|(_, span)| span_size(*span))
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
        if let Some(ty) = self.setmetatable_return_type(function_call, position) {
            return vec![ty];
        }

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

            if vector::is_constructor(root) {
                return vector::constructor_type().return_types_or_any().to_vec();
            }

            if let Some(returns) = self
                .type_of_name(root, position)
                .and_then(|ty| call_metamethod_return_types(self.environment, &ty))
            {
                return returns;
            }
        }

        if let Some(Type::Function(function)) = self.type_of_field_chain(parts, position) {
            if let Some(ty) = self.constructor_call_return_type(parts, position) {
                return vec![ty];
            }

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

            if let Some(function) = self.typed_function_for_local_export_alias(root, position) {
                return Some(function);
            }

            if let Some(kind) = asset::loader_kind(root) {
                return Some(typed_function_from_asset_loader(kind));
            }

            if vector::is_constructor(root) {
                return Some(typed_function_from_vector_constructor());
            }

            if let Some(function) = self
                .type_of_name(root, position)
                .and_then(|ty| call_metamethod_function(self.environment, &ty))
            {
                return Some(typed_function_from_function_type(
                    Some(qualified_name),
                    function,
                ));
            }
        }

        if let Some(function) = self.prototype_typed_function_for_call_parts(parts, position) {
            return Some(function);
        }

        if !fields.is_empty() {
            if let Some(export) = required_module_export_for_fields(self.parsed, root, fields) {
                if let Some(function) =
                    typed_function_from_module_export(Some(qualified_name.clone()), export)
                {
                    return Some(function);
                }
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

    fn typed_function_for_local_export_alias(
        &self,
        name: &str,
        position: SourcePosition,
    ) -> Option<TypedFunction> {
        let local = self.local_assignment_value(name, position)?;
        let expression = local.expression?;
        let Expression::Var(var) = expression else {
            return None;
        };
        let parts = plain_field_chain_from_var(var)?;
        let (root, fields) = parts.split_first()?;

        if fields.is_empty() {
            return None;
        }

        required_module_export_for_fields(self.parsed, root, fields)
            .and_then(|export| typed_function_from_module_export(Some(name.to_owned()), export))
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

    pub(crate) fn function_type_for_call_parts(
        &self,
        parts: &[String],
        position: SourcePosition,
    ) -> Option<crate::FunctionType> {
        match self.type_of_field_chain(parts, position)? {
            Type::Function(function) => Some(function),
            _ => None,
        }
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

        let ty = self
            .environment
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
            .or_else(|| self.loop_variable_type_at_position(name, position, depth + 1))
            .or_else(|| self.parameter_type_at_position(name, position))?;

        Some(self.apply_table_mutations_to_name(name, ty, position, depth + 1))
    }

    fn setmetatable_return_type(
        &self,
        function_call: &FunctionCall,
        position: SourcePosition,
    ) -> Option<Type> {
        let (target, meta) = setmetatable_arguments(function_call)?;
        let base = self.type_of_expression_return_at_depth(target, position, 0, 0)?;
        let meta = self.type_of_expression_return_at_depth(meta, position, 0, 0)?;

        Some(crate::types::MetatableFacts::from_type(&meta).apply_to_type(base))
    }

    fn apply_table_mutations_to_name(
        &self,
        name: &str,
        ty: Type,
        position: SourcePosition,
        depth: usize,
    ) -> Type {
        if depth >= MAX_SEMANTIC_INFERENCE_DEPTH {
            return ty;
        }
        let Type::Table(mut table) = ty else {
            return ty;
        };

        self.apply_table_mutations_in_block(
            self.parsed.ast.nodes(),
            name,
            position,
            depth + 1,
            &mut table,
        );

        Type::Table(table)
    }

    fn apply_table_mutations_in_block(
        &self,
        block: &ast::Block,
        name: &str,
        position: SourcePosition,
        depth: usize,
        table: &mut crate::TableShape,
    ) {
        for stmt in block.stmts() {
            if !starts_before_or_at(node_start(stmt), position) {
                continue;
            }

            match stmt {
                ast::Stmt::Assignment(assignment) => {
                    for (variable, expression) in
                        assignment.variables().iter().zip(assignment.expressions())
                    {
                        let Some(parts) = plain_field_chain_from_var(variable) else {
                            continue;
                        };
                        let [root, field] = parts.as_slice() else {
                            continue;
                        };

                        if root != name {
                            continue;
                        }

                        let is_self_index =
                            field == "__index" && expression_is_name(expression, name);
                        if is_self_index {
                            table.mark_self_index();
                        }

                        let ty = if is_self_index {
                            Type::Table(table.clone())
                        } else {
                            self.type_of_expression_return_at_depth(
                                expression,
                                node_start(variable),
                                0,
                                depth + 1,
                            )
                            .unwrap_or(Type::Any)
                        };
                        table.insert(field.to_owned(), TableField::required(ty));
                    }
                }
                ast::Stmt::FunctionDeclaration(function) => {
                    let Some((root, field, is_method)) = function_declaration_table_field(function)
                    else {
                        continue;
                    };

                    if root != name {
                        continue;
                    }

                    let ty = self
                        .environment
                        .function(function.name().to_string().trim())
                        .and_then(TypedFunction::as_type)
                        .unwrap_or_else(|| {
                            function_type_from_body_with_receiver(function.body(), is_method)
                        });
                    if is_method && function_body_returns_setmetatable_self(function.body()) {
                        table.mark_constructor_method(&field);
                    }
                    table.insert(field, TableField::required(ty));
                }
                ast::Stmt::FunctionCall(function_call) => {
                    let Some((target, meta)) = setmetatable_arguments(function_call) else {
                        continue;
                    };
                    if !expression_is_name(target, name) {
                        continue;
                    }
                    let Some(meta) = self.type_of_expression_return_at_depth(
                        meta,
                        node_start(function_call),
                        0,
                        depth + 1,
                    ) else {
                        continue;
                    };

                    MetatableFacts::from_type(&meta).apply_to_table(table);
                }
                _ => {}
            }
        }
    }

    fn constructor_call_return_type(
        &self,
        parts: &[String],
        position: SourcePosition,
    ) -> Option<Type> {
        let (method, receiver_parts) = parts.split_last()?;
        let receiver_type = self.type_of_field_chain(receiver_parts, position)?;
        let Type::Table(table) = resolve_aliases(self.environment, &receiver_type, 0) else {
            return None;
        };

        (table.has_self_index() && table.is_constructor_method(method)).then_some(receiver_type)
    }

    fn prototype_typed_function_for_call_parts(
        &self,
        parts: &[String],
        position: SourcePosition,
    ) -> Option<TypedFunction> {
        let (method, receiver_parts) = parts.split_last()?;
        let [receiver] = receiver_parts else {
            return None;
        };
        let prototype = self
            .setmetatable_prototype_for_receiver(receiver, position)
            .filter(|prototype| self.prototype_has_self_index(prototype, position))?;

        self.environment
            .function(&format!("{prototype}:{method}"))
            .or_else(|| self.environment.function(&format!("{prototype}.{method}")))
            .cloned()
    }

    fn setmetatable_prototype_for_receiver(
        &self,
        receiver: &str,
        position: SourcePosition,
    ) -> Option<String> {
        let mut candidates = Vec::new();

        for stmt in self.parsed.ast.nodes().stmts() {
            let Some(stmt_span) = node_span(stmt) else {
                continue;
            };
            if !starts_before_or_at(stmt_span.start, position) {
                continue;
            }

            match stmt {
                ast::Stmt::LocalAssignment(local) => {
                    for (name, expression) in local.names().iter().zip(local.expressions()) {
                        if token_text(name) != receiver {
                            continue;
                        }
                        let Some(prototype) = setmetatable_prototype_from_expression(expression)
                        else {
                            continue;
                        };

                        candidates.push((stmt_span.start, prototype));
                    }
                }
                ast::Stmt::Assignment(assignment) => {
                    for (variable, expression) in
                        assignment.variables().iter().zip(assignment.expressions())
                    {
                        let Var::Name(name) = variable else {
                            continue;
                        };
                        if token_text(name) != receiver {
                            continue;
                        }
                        let Some(prototype) = setmetatable_prototype_from_expression(expression)
                        else {
                            continue;
                        };

                        candidates.push((stmt_span.start, prototype));
                    }
                }
                ast::Stmt::FunctionCall(function_call) => {
                    let Some((target, meta)) = setmetatable_arguments(function_call) else {
                        continue;
                    };
                    if !expression_is_name(target, receiver) {
                        continue;
                    }
                    let Some(prototype) = expression_name(meta) else {
                        continue;
                    };

                    candidates.push((stmt_span.start, prototype));
                }
                _ => {}
            }
        }

        candidates
            .into_iter()
            .max_by_key(|(position, _)| position_key(*position))
            .map(|(_, prototype)| prototype)
    }

    fn prototype_has_self_index(&self, prototype: &str, position: SourcePosition) -> bool {
        for stmt in self.parsed.ast.nodes().stmts() {
            let Some(stmt_span) = node_span(stmt) else {
                continue;
            };
            if !starts_before_or_at(stmt_span.start, position) {
                continue;
            }
            let ast::Stmt::Assignment(assignment) = stmt else {
                continue;
            };

            for (variable, expression) in
                assignment.variables().iter().zip(assignment.expressions())
            {
                let Some(parts) = plain_field_chain_from_var(variable) else {
                    continue;
                };
                if parts.as_slice() == [prototype, "__index"]
                    && expression_is_name(expression, prototype)
                {
                    return true;
                }
            }
        }

        false
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
            Expression::UnaryOperator { unop, expression } => {
                let operand =
                    self.type_of_expression_return_at_depth(expression, position, 0, depth + 1)?;

                infer_unary_operator_type(self.environment, unop, &operand)
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
            Expression::Function(function) => Some(function_type_from_body(function.body())),
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
        let mut fields = Vec::new();
        let mut array_items = Vec::new();

        for field in table.fields() {
            match field {
                Field::ExpressionKey { key, value, .. } => {
                    let ty = self
                        .type_of_expression_return_at_depth(value, position, 0, depth + 1)
                        .unwrap_or(Type::Any);

                    if let Some(key) = string_literal_expression(key) {
                        fields.push((key, TableField::required(ty)));
                    } else if expression_is_number(key) {
                        array_items.push(ty);
                    }
                }
                Field::NameKey { key, value, .. } => {
                    let key_text = token_text(key);
                    let ty = if key_text == "__index" {
                        index_type_from_expression(value).or_else(|| {
                            self.type_of_expression_return_at_depth(value, position, 0, depth + 1)
                        })
                    } else {
                        self.type_of_expression_return_at_depth(value, position, 0, depth + 1)
                    }
                    .unwrap_or(Type::Any);

                    fields.push((key_text, TableField::required(ty)));
                }
                Field::NoKey(value) => {
                    array_items.push(
                        self.type_of_expression_return_at_depth(value, position, 0, depth + 1)
                            .unwrap_or(Type::Any),
                    );
                }
                _ => {}
            }
        }

        table_or_array_type(fields, array_items)
    }

    fn collect_expression_types_in_block(
        &self,
        block: &'a ast::Block,
        position: SourcePosition,
        candidates: &mut Vec<(Type, SourceSpan)>,
    ) {
        for stmt in block.stmts() {
            if let Some(span) = node_span(stmt) {
                if !contains_position(span, position) {
                    continue;
                }
            }

            match stmt {
                ast::Stmt::Assignment(assignment) => {
                    for expression in assignment.expressions() {
                        self.collect_expression_type(expression, position, candidates);
                    }
                }
                ast::Stmt::Do(block) => {
                    self.collect_expression_types_in_block(block.block(), position, candidates);
                }
                ast::Stmt::FunctionDeclaration(function) => {
                    self.collect_expression_types_in_block(
                        function.body().block(),
                        position,
                        candidates,
                    );
                }
                ast::Stmt::FunctionCall(function_call) => {
                    if let Some(span) = node_span(function_call) {
                        if contains_position(span, position) {
                            if let Some(ty) = self
                                .return_types_of_call(function_call, position)
                                .first()
                                .cloned()
                            {
                                candidates.push((ty, span));
                            }
                        }
                    }
                }
                ast::Stmt::GenericFor(for_stmt) => {
                    self.collect_expression_types_in_block(for_stmt.block(), position, candidates);
                }
                ast::Stmt::If(if_stmt) => {
                    self.collect_expression_type(if_stmt.condition(), position, candidates);
                    self.collect_expression_types_in_block(if_stmt.block(), position, candidates);

                    if let Some(else_ifs) = if_stmt.else_if() {
                        for else_if in else_ifs {
                            self.collect_expression_type(else_if.condition(), position, candidates);
                            self.collect_expression_types_in_block(
                                else_if.block(),
                                position,
                                candidates,
                            );
                        }
                    }

                    if let Some(else_block) = if_stmt.else_block() {
                        self.collect_expression_types_in_block(else_block, position, candidates);
                    }
                }
                ast::Stmt::LocalAssignment(local) => {
                    for expression in local.expressions() {
                        self.collect_expression_type(expression, position, candidates);
                    }
                }
                ast::Stmt::LocalFunction(function) => {
                    self.collect_expression_types_in_block(
                        function.body().block(),
                        position,
                        candidates,
                    );
                }
                ast::Stmt::NumericFor(for_stmt) => {
                    self.collect_expression_type(for_stmt.start(), position, candidates);
                    self.collect_expression_type(for_stmt.end(), position, candidates);

                    if let Some(step) = for_stmt.step() {
                        self.collect_expression_type(step, position, candidates);
                    }

                    self.collect_expression_types_in_block(for_stmt.block(), position, candidates);
                }
                ast::Stmt::Repeat(repeat) => {
                    self.collect_expression_types_in_block(repeat.block(), position, candidates);
                    self.collect_expression_type(repeat.until(), position, candidates);
                }
                ast::Stmt::While(while_stmt) => {
                    self.collect_expression_type(while_stmt.condition(), position, candidates);
                    self.collect_expression_types_in_block(
                        while_stmt.block(),
                        position,
                        candidates,
                    );
                }
                _ => {}
            }
        }

        if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
            for expression in return_stmt.returns() {
                self.collect_expression_type(expression, position, candidates);
            }
        }
    }

    fn collect_expression_type(
        &self,
        expression: &'a Expression,
        position: SourcePosition,
        candidates: &mut Vec<(Type, SourceSpan)>,
    ) {
        let Some(span) = node_span(expression) else {
            return;
        };
        if !contains_position(span, position) {
            return;
        }

        match expression {
            Expression::BinaryOperator { lhs, rhs, .. } => {
                self.collect_expression_type(lhs, position, candidates);
                self.collect_expression_type(rhs, position, candidates);
            }
            Expression::Function(function) => {
                self.collect_expression_types_in_block(
                    function.body().block(),
                    position,
                    candidates,
                );
            }
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. }
            | Expression::UnaryOperator { expression, .. } => {
                self.collect_expression_type(expression, position, candidates);
            }
            Expression::TableConstructor(table) => {
                for field in table.fields() {
                    match field {
                        Field::ExpressionKey { key, value, .. } => {
                            self.collect_expression_type(key, position, candidates);
                            self.collect_expression_type(value, position, candidates);
                        }
                        Field::NameKey { value, .. } | Field::NoKey(value) => {
                            self.collect_expression_type(value, position, candidates);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        if let Some(ty) = self.type_of_expression_return(expression, position, 0) {
            candidates.push((ty, span));
        }
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
            Var::Expression(expression) => {
                self.type_of_var_expression_at_depth(expression, position, depth + 1)
            }
            _ => None,
        }
    }

    fn type_of_var_expression_at_depth(
        &self,
        expression: &ast::VarExpression,
        position: SourcePosition,
        depth: usize,
    ) -> Option<Type> {
        if depth >= MAX_SEMANTIC_INFERENCE_DEPTH {
            return None;
        }

        let mut current = match expression.prefix() {
            Prefix::Name(name) => {
                self.type_of_name_at_depth(&token_text(name), position, depth + 1)?
            }
            Prefix::Expression(expression) => {
                self.type_of_expression_return_at_depth(expression, position, 0, depth + 1)?
            }
            _ => return None,
        };

        for suffix in expression.suffixes() {
            match suffix {
                Suffix::Index(Index::Dot { name, .. }) => {
                    current =
                        resolve_table_field_type(self.environment, &current, &token_text(name))?;
                }
                Suffix::Index(Index::Brackets { expression, .. }) => {
                    let key = self.index_key_type(expression, position, depth + 1)?;
                    current = resolve_index_type(self.environment, &current, &key)?;
                }
                _ => return None,
            }
        }

        Some(current)
    }

    fn index_key_type(
        &self,
        expression: &Expression,
        position: SourcePosition,
        depth: usize,
    ) -> Option<Type> {
        match expression {
            Expression::Number(_) => Some(Type::number()),
            Expression::String(value) => {
                Some(Type::string_literal(unquote_string(&value.to_string())))
            }
            _ => self.type_of_expression_return_at_depth(expression, position, 0, depth + 1),
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

    fn loop_variable_type_at_position(
        &self,
        name: &str,
        position: SourcePosition,
        depth: usize,
    ) -> Option<Type> {
        let mut candidates = Vec::new();

        self.collect_loop_variable_types_in_block(
            self.parsed.ast.nodes(),
            name,
            position,
            depth,
            &mut candidates,
        );

        candidates
            .into_iter()
            .max_by_key(|(span, _)| position_key(span.start))
            .map(|(_, ty)| ty)
    }

    fn collect_loop_variable_types_in_block(
        &self,
        block: &'a ast::Block,
        name: &str,
        position: SourcePosition,
        depth: usize,
        candidates: &mut Vec<(SourceSpan, Type)>,
    ) {
        for stmt in block.stmts() {
            let Some(stmt_span) = node_span(stmt) else {
                continue;
            };
            if !contains_position(stmt_span, position) {
                continue;
            }

            match stmt {
                ast::Stmt::GenericFor(for_stmt) => {
                    if let Some(block_span) = node_span(for_stmt.block()) {
                        if contains_position(stmt_span, position) {
                            let types = self.generic_for_variable_types(for_stmt, position, depth);
                            for (loop_name, ty) in for_stmt.names().into_iter().zip(types) {
                                if token_text(loop_name) == name {
                                    candidates.push((block_span, ty));
                                }
                            }
                        }
                    }
                    self.collect_loop_variable_types_in_block(
                        for_stmt.block(),
                        name,
                        position,
                        depth + 1,
                        candidates,
                    );
                }
                ast::Stmt::NumericFor(for_stmt) => {
                    if token_text(for_stmt.index_variable()) == name
                        && contains_position(stmt_span, position)
                    {
                        candidates.push((stmt_span, Type::number()));
                    }
                    self.collect_loop_variable_types_in_block(
                        for_stmt.block(),
                        name,
                        position,
                        depth + 1,
                        candidates,
                    );
                }
                ast::Stmt::Do(block) => self.collect_loop_variable_types_in_block(
                    block.block(),
                    name,
                    position,
                    depth + 1,
                    candidates,
                ),
                ast::Stmt::FunctionDeclaration(function) => self
                    .collect_loop_variable_types_in_block(
                        function.body().block(),
                        name,
                        position,
                        depth + 1,
                        candidates,
                    ),
                ast::Stmt::If(if_stmt) => {
                    self.collect_loop_variable_types_in_block(
                        if_stmt.block(),
                        name,
                        position,
                        depth + 1,
                        candidates,
                    );
                    if let Some(else_ifs) = if_stmt.else_if() {
                        for else_if in else_ifs {
                            self.collect_loop_variable_types_in_block(
                                else_if.block(),
                                name,
                                position,
                                depth + 1,
                                candidates,
                            );
                        }
                    }
                    if let Some(else_block) = if_stmt.else_block() {
                        self.collect_loop_variable_types_in_block(
                            else_block,
                            name,
                            position,
                            depth + 1,
                            candidates,
                        );
                    }
                }
                ast::Stmt::LocalFunction(function) => self.collect_loop_variable_types_in_block(
                    function.body().block(),
                    name,
                    position,
                    depth + 1,
                    candidates,
                ),
                ast::Stmt::Repeat(repeat) => self.collect_loop_variable_types_in_block(
                    repeat.block(),
                    name,
                    position,
                    depth + 1,
                    candidates,
                ),
                ast::Stmt::While(while_stmt) => self.collect_loop_variable_types_in_block(
                    while_stmt.block(),
                    name,
                    position,
                    depth + 1,
                    candidates,
                ),
                _ => {}
            }
        }
    }

    fn generic_for_variable_types(
        &self,
        for_stmt: &ast::GenericFor,
        position: SourcePosition,
        depth: usize,
    ) -> Vec<Type> {
        let Some(Expression::FunctionCall(function_call)) = for_stmt.expressions().iter().next()
        else {
            return vec![Type::Any; for_stmt.names().into_iter().count()];
        };
        let Some((iterator, value_expression)) = iterator_call(function_call) else {
            return vec![Type::Any; for_stmt.names().into_iter().count()];
        };
        let value_type = self
            .type_of_expression_return_at_depth(value_expression, position, 0, depth + 1)
            .unwrap_or(Type::Any);

        match iterator.as_str() {
            "ipairs" => iterator_metamethod_pair_types(self.environment, &value_type, "__ipairs")
                .map(|(key, value)| vec![key, value])
                .unwrap_or_else(|| {
                    vec![
                        Type::number(),
                        array_element_type(self.environment, &value_type).unwrap_or(Type::Any),
                    ]
                }),
            "pairs" => iterator_metamethod_pair_types(self.environment, &value_type, "__pairs")
                .or_else(|| pair_key_value_types(self.environment, &value_type))
                .map(|(key, value)| vec![key, value])
                .unwrap_or_else(|| vec![Type::Any, Type::Any]),
            _ => vec![Type::Any; for_stmt.names().into_iter().count()],
        }
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
        Type::Table(table) => table.semantic_field(field).map(|field| field.ty.clone()),
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

pub(crate) fn resolve_index_type(
    environment: &TypeEnvironment,
    collection: &Type,
    key: &Type,
) -> Option<Type> {
    match resolve_aliases(environment, collection, 0) {
        Type::Array(item) => is_number_like(key).then_some(*item),
        Type::Tuple(items) => is_number_like(key).then(|| union_types(items)),
        Type::Dictionary {
            key: dict_key,
            value,
        } => key_accepts_index(&dict_key, key).then_some(*value),
        Type::Table(table) => match key {
            Type::StringLiteral(value) => table.semantic_field(value).map(|field| field.ty.clone()),
            Type::Primitive(PrimitiveType::String) => {
                union_types_optional(table.semantic_fields().into_values().map(|field| field.ty))
            }
            _ => None,
        },
        Type::Union(types) => union_types_optional(
            types
                .iter()
                .filter_map(|ty| resolve_index_type(environment, ty, key)),
        ),
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_) => None,
    }
}

fn array_element_type(environment: &TypeEnvironment, ty: &Type) -> Option<Type> {
    match resolve_aliases(environment, ty, 0) {
        Type::Array(item) => Some(*item),
        Type::Tuple(items) => Some(union_types(items)),
        Type::Union(types) => union_types_optional(
            types
                .iter()
                .filter_map(|ty| array_element_type(environment, ty)),
        ),
        Type::Any
        | Type::Alias(_)
        | Type::Dictionary { .. }
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Table(_) => None,
    }
}

fn pair_key_value_types(environment: &TypeEnvironment, ty: &Type) -> Option<(Type, Type)> {
    match resolve_aliases(environment, ty, 0) {
        Type::Array(item) => Some((Type::number(), *item)),
        Type::Tuple(items) => Some((Type::number(), union_types(items))),
        Type::Dictionary { key, value } => Some((*key, *value)),
        Type::Table(table) => {
            let fields = table.semantic_fields();
            let keys = fields
                .keys()
                .cloned()
                .map(Type::string_literal)
                .collect::<Vec<_>>();
            let values = fields
                .into_values()
                .map(|field| field.ty)
                .collect::<Vec<_>>();

            Some((union_types(keys), union_types(values)))
        }
        Type::Union(types) => {
            let pairs = types
                .iter()
                .filter_map(|ty| pair_key_value_types(environment, ty))
                .collect::<Vec<_>>();
            let keys = pairs.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>();
            let values = pairs
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Vec<_>>();

            Some((union_types(keys), union_types(values)))
        }
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_) => None,
    }
}

fn iterator_metamethod_pair_types(
    environment: &TypeEnvironment,
    ty: &Type,
    name: &str,
) -> Option<(Type, Type)> {
    match resolve_aliases(environment, ty, 0) {
        Type::Table(table) => table
            .metatable_facts()
            .and_then(|facts| facts.metamethod(name))
            .and_then(pair_types_from_iterator_function),
        Type::Union(types) => {
            let pairs = types
                .iter()
                .filter_map(|ty| iterator_metamethod_pair_types(environment, ty, name))
                .collect::<Vec<_>>();
            let keys = pairs.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>();
            let values = pairs
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Vec<_>>();

            (!keys.is_empty()).then(|| (union_types(keys), union_types(values)))
        }
        Type::Any
        | Type::Alias(_)
        | Type::Array(_)
        | Type::Dictionary { .. }
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Tuple(_) => None,
    }
}

fn pair_types_from_iterator_function(function: &FunctionType) -> Option<(Type, Type)> {
    let returns = function.return_types_or_any();

    match returns {
        [Type::Tuple(items)] if items.len() >= 2 => {
            Some((items.first()?.clone(), items.get(1)?.clone()))
        }
        [key, value, ..] => Some((key.clone(), value.clone())),
        _ => None,
    }
}

fn table_or_array_type(fields: Vec<(String, TableField)>, array_items: Vec<Type>) -> Type {
    let table = (!fields.is_empty()).then(|| Type::table(fields));
    let array = (!array_items.is_empty()).then(|| Type::array(union_types(array_items)));

    match (table, array) {
        (Some(table), Some(array)) => Type::union([table, array]),
        (Some(table), None) => table,
        (None, Some(array)) => array,
        (None, None) => Type::empty_table(),
    }
}

fn union_types(types: Vec<Type>) -> Type {
    let mut unique = Vec::new();
    for ty in types {
        if !unique.contains(&ty) {
            unique.push(ty);
        }
    }

    match unique.len() {
        0 => Type::Any,
        1 => unique.into_iter().next().unwrap_or(Type::Any),
        _ => Type::union(unique),
    }
}

fn union_types_optional(types: impl IntoIterator<Item = Type>) -> Option<Type> {
    let types = types.into_iter().collect::<Vec<_>>();

    (!types.is_empty()).then(|| union_types(types))
}

fn is_number_like(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(PrimitiveType::Number))
}

fn key_accepts_index(expected: &Type, actual: &Type) -> bool {
    match (expected, actual) {
        (Type::Any, _) | (_, Type::Any) => true,
        (Type::Primitive(PrimitiveType::String), Type::Primitive(PrimitiveType::String))
        | (Type::Primitive(PrimitiveType::String), Type::StringLiteral(_))
        | (Type::Primitive(PrimitiveType::Number), Type::Primitive(PrimitiveType::Number)) => true,
        (Type::StringLiteral(expected), Type::StringLiteral(actual)) => expected == actual,
        (Type::Union(types), actual) => types.iter().any(|ty| key_accepts_index(ty, actual)),
        (expected, Type::Union(types)) => types.iter().any(|ty| key_accepts_index(expected, ty)),
        _ => false,
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
        TuaType::StringLiteral(value) => Type::string_literal(value),
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
        TuaType::Array(item) => Type::array(type_from_tua_type(item)),
        TuaType::Tuple(items) => Type::tuple(items.iter().map(type_from_tua_type)),
        TuaType::Dictionary { key, value } => {
            Type::dictionary(type_from_tua_type(key), type_from_tua_type(value))
        }
        TuaType::Function {
            parameters,
            returns,
        } => Type::function_returns(
            parameters.iter().map(|parameter| {
                let ty = type_from_tua_type(&parameter.ty);

                if parameter.optional {
                    Type::union([ty, Type::nil()])
                } else {
                    ty
                }
            }),
            returns.iter().map(type_from_tua_type),
        ),
    }
}

fn index_type_from_expression(expression: &Expression) -> Option<Type> {
    let fields = index_fields_from_expression(expression);

    (!fields.is_empty()).then(|| Type::table(fields))
}

fn index_fields_from_expression(expression: &Expression) -> Vec<(String, TableField)> {
    let Expression::Function(function) = expression else {
        return Vec::new();
    };

    index_fields_from_block(function.body().block())
}

fn index_fields_from_block(block: &ast::Block) -> Vec<(String, TableField)> {
    let mut fields = Vec::new();

    for stmt in block.stmts() {
        if let ast::Stmt::If(if_stmt) = stmt {
            fields.extend(index_fields_from_if(if_stmt));
        }
    }

    fields
}

fn index_fields_from_if(if_stmt: &ast::If) -> Vec<(String, TableField)> {
    let Some(key) = string_key_comparison(if_stmt.condition()) else {
        return Vec::new();
    };
    let Some(LastStmt::Return(return_stmt)) = if_stmt.block().last_stmt() else {
        return Vec::new();
    };
    let Some(expression) = return_stmt.returns().iter().next() else {
        return Vec::new();
    };
    let ty = match expression {
        Expression::Number(_) => Type::number(),
        Expression::String(value) => Type::string_literal(unquote_string(&value.to_string())),
        Expression::Symbol(symbol) => infer_symbol_type(&token_text(symbol)).unwrap_or(Type::Any),
        Expression::TableConstructor(table) => {
            Type::table(table.fields().iter().filter_map(|field| {
                let Field::NameKey { key, .. } = field else {
                    return None;
                };

                Some((token_text(key), TableField::required(Type::Any)))
            }))
        }
        _ => Type::Any,
    };

    vec![(key, TableField::required(ty))]
}

fn string_key_comparison(expression: &Expression) -> Option<String> {
    let Expression::BinaryOperator { lhs, binop, rhs } = expression else {
        return None;
    };

    if !matches!(binop, ast::BinOp::TwoEqual(_)) {
        return None;
    }

    string_literal_expression(rhs).or_else(|| string_literal_expression(lhs))
}

fn string_literal_expression(expression: &Expression) -> Option<String> {
    match expression {
        Expression::String(value) => Some(unquote_string(&value.to_string())),
        _ => None,
    }
}

fn expression_is_number(expression: &Expression) -> bool {
    matches!(expression, Expression::Number(_))
}

fn iterator_call(function_call: &FunctionCall) -> Option<(String, &Expression)> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };
    let Some(Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }))) =
        function_call.suffixes().next()
    else {
        return None;
    };

    Some((token_text(name), arguments.iter().next()?))
}

fn unquote_string(value: &str) -> String {
    value
        .trim()
        .strip_prefix(['"', '\''])
        .and_then(|value| value.strip_suffix(['"', '\'']))
        .unwrap_or_else(|| value.trim())
        .to_owned()
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

fn plain_field_chain_from_var(var: &Var) -> Option<Vec<String>> {
    match var {
        Var::Name(name) => Some(vec![token_text(name)]),
        Var::Expression(expression) => plain_field_chain_from_var_expression(expression),
        _ => None,
    }
}

fn expression_is_name(expression: &Expression, expected: &str) -> bool {
    matches!(expression, Expression::Var(Var::Name(name)) if token_text(name) == expected)
}

fn expression_name(expression: &Expression) -> Option<String> {
    match expression {
        Expression::Var(Var::Name(name)) => Some(token_text(name)),
        _ => None,
    }
}

fn setmetatable_prototype_from_expression(expression: &Expression) -> Option<String> {
    let Expression::FunctionCall(function_call) = expression else {
        return None;
    };
    let (_, meta) = setmetatable_arguments(function_call)?;

    expression_name(meta)
}

fn function_declaration_table_field(
    function: &ast::FunctionDeclaration,
) -> Option<(String, String, bool)> {
    let name = function.name().to_string();
    let colon = name.rfind(':');
    let dot = name.rfind('.');
    let separator = match (colon, dot) {
        (Some(colon), Some(dot)) => Some((colon.max(dot), colon > dot)),
        (Some(colon), None) => Some((colon, true)),
        (None, Some(dot)) => Some((dot, false)),
        (None, None) => None,
    }?;
    let owner = name[..separator.0].trim();
    let field = name[separator.0 + 1..].trim();

    (!owner.is_empty() && !field.is_empty())
        .then(|| (owner.to_owned(), field.to_owned(), separator.1))
}

fn setmetatable_arguments(function_call: &FunctionCall) -> Option<(&Expression, &Expression)> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    if token_text(name) != "setmetatable" {
        return None;
    }

    let Some(Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }))) =
        function_call.suffixes().next()
    else {
        return None;
    };

    Some((arguments.iter().next()?, arguments.iter().nth(1)?))
}

fn function_type_from_body(body: &ast::FunctionBody) -> Type {
    let parameters = function_parameter_types(body);
    let return_type = body
        .return_type()
        .and_then(|specifier| type_from_type_info(specifier.type_info()))
        .unwrap_or(Type::Any);

    Type::function(parameters, return_type)
}

fn function_parameter_types(body: &ast::FunctionBody) -> Vec<Type> {
    let parameters = body
        .parameters()
        .iter()
        .filter_map(|parameter| match parameter {
            ast::Parameter::Name(name) => Some((name.start_position()?, name.end_position()?)),
            ast::Parameter::Ellipsis(_) => None,
            _ => None,
        })
        .collect::<Vec<_>>();
    let specifiers = body
        .type_specifiers()
        .filter_map(|specifier| {
            let specifier = specifier?;
            Some((
                specifier.start_position()?,
                type_from_type_info(specifier.type_info())?,
            ))
        })
        .collect::<Vec<_>>();

    parameters
        .iter()
        .enumerate()
        .map(|(index, (_, parameter_end))| {
            let next_parameter_start = parameters.get(index + 1).map(|(start, _)| *start);

            specifiers
                .iter()
                .find_map(|(specifier_start, ty)| {
                    if specifier_start >= parameter_end
                        && next_parameter_start
                            .map(|next_start| specifier_start < &next_start)
                            .unwrap_or(true)
                    {
                        Some(ty.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or(Type::Any)
        })
        .collect()
}

fn type_from_type_info(type_info: &TypeInfo) -> Option<Type> {
    match type_info {
        TypeInfo::Basic(token) => Some(Type::named(token_text(token))),
        TypeInfo::String(token) => Some(Type::string_literal(unquote_string(&token_text(token)))),
        TypeInfo::Optional { base, .. } => {
            Some(Type::union([type_from_type_info(base)?, Type::nil()]))
        }
        TypeInfo::Union(union) => union
            .types()
            .iter()
            .map(type_from_type_info)
            .collect::<Option<Vec<_>>>()
            .map(Type::union),
        _ => Some(Type::named(type_info.to_string())),
    }
}

fn function_type_from_body_with_receiver(body: &ast::FunctionBody, include_receiver: bool) -> Type {
    let Type::Function(mut function) = function_type_from_body(body) else {
        return Type::Any;
    };

    if include_receiver {
        function.parameters.insert(0, Type::Any);
    }

    Type::Function(function)
}

fn function_body_returns_setmetatable_self(body: &ast::FunctionBody) -> bool {
    let Some(LastStmt::Return(return_stmt)) = body.block().last_stmt() else {
        return false;
    };
    let Some(Expression::FunctionCall(function_call)) = return_stmt.returns().iter().next() else {
        return false;
    };
    let Some((_, meta)) = setmetatable_arguments(function_call) else {
        return false;
    };

    expression_is_name(meta, "self")
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
        Type::Any
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::Table(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => None,
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
        vararg_type: None,
        return_type: Some(return_type.clone()),
        return_types: vec![return_type],
        documentation: Some(kind.documentation().to_owned()),
        overloads: Vec::new(),
    }
}

fn typed_function_from_vector_constructor() -> TypedFunction {
    let return_type = Type::alias(vector::TYPE_NAME);

    TypedFunction {
        name: Some(vector::CONSTRUCTOR_NAME.to_owned()),
        parameters: vec![
            TypedParameter {
                name: "x".to_owned(),
                ty: Type::number(),
                documentation: None,
            },
            TypedParameter {
                name: "y".to_owned(),
                ty: Type::number(),
                documentation: None,
            },
        ],
        parameter_count: 2,
        has_varargs: false,
        vararg_type: None,
        return_type: Some(return_type.clone()),
        return_types: vec![return_type],
        documentation: Some(vector::documentation().to_owned()),
        overloads: Vec::new(),
    }
}

fn typed_function_from_function_type(
    name: Option<String>,
    function: FunctionType,
) -> TypedFunction {
    let return_types = function.return_types_or_any().to_vec();
    let parameter_count = function.parameters.len();

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
        parameter_count,
        has_varargs: false,
        vararg_type: None,
        return_type: return_types.first().cloned(),
        return_types,
        documentation: None,
        overloads: Vec::new(),
    }
}

fn typed_function_from_module_export(
    name: Option<String>,
    export: ModuleExportField,
) -> Option<TypedFunction> {
    let Type::Function(function) = export.ty else {
        return None;
    };

    let return_types = function.return_types_or_any().to_vec();
    let parameter_count = function.parameters.len();
    let parameters = function
        .parameters
        .into_iter()
        .enumerate()
        .map(|(index, ty)| TypedParameter {
            name: export
                .parameter_names
                .get(index)
                .cloned()
                .unwrap_or_default(),
            ty,
            documentation: export.parameter_documentation.get(index).cloned().flatten(),
        })
        .collect();

    Some(TypedFunction {
        name,
        parameters,
        parameter_count,
        has_varargs: false,
        vararg_type: None,
        return_type: return_types.first().cloned(),
        return_types,
        documentation: export.documentation,
        overloads: Vec::new(),
    })
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
        vararg_type: None,
        return_type: return_types.first().cloned(),
        return_types,
        documentation: Some(item.documentation.to_owned()),
        overloads: Vec::new(),
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

    if let Some(name) = arithmetic_metamethod_name(binop) {
        if let Some(function) = binary_metamethod(environment, left, right, name) {
            return Some(metamethod_return_or(&function, left.clone()));
        }
    }

    if let Some(name) = comparison_metamethod_name(binop) {
        if let Some(function) = binary_metamethod(environment, left, right, name) {
            return Some(metamethod_return_or(&function, Type::boolean()));
        }
    }

    if matches!(left, Type::Any) || matches!(right, Type::Any) {
        return Some(Type::Any);
    }

    None
}

fn infer_unary_operator_type(
    environment: &TypeEnvironment,
    unop: &ast::UnOp,
    operand: &Type,
) -> Option<Type> {
    match unop {
        ast::UnOp::Hash(_) => {
            if let Some(function) =
                resolve_metatable_facts(environment, operand).and_then(|facts| facts.len)
            {
                return Some(metamethod_return_or(&function, Type::number()));
            }

            if matches!(operand, Type::Any) {
                Some(Type::Any)
            } else {
                Some(Type::number())
            }
        }
        ast::UnOp::Minus(_) => {
            if is_number_type(environment, operand) {
                Some(Type::number())
            } else if matches!(operand, Type::Any) {
                Some(Type::Any)
            } else {
                None
            }
        }
        ast::UnOp::Not(_) => Some(Type::boolean()),
        _ => {
            if matches!(operand, Type::Any) {
                Some(Type::Any)
            } else {
                Some(Type::number())
            }
        }
    }
}

fn binary_metamethod(
    environment: &TypeEnvironment,
    left: &Type,
    right: &Type,
    name: &str,
) -> Option<FunctionType> {
    resolve_metatable_facts(environment, left)
        .and_then(|facts| facts.metamethod(name).cloned())
        .or_else(|| {
            resolve_metatable_facts(environment, right)
                .and_then(|facts| facts.metamethod(name).cloned())
        })
}

fn resolve_metatable_facts(environment: &TypeEnvironment, ty: &Type) -> Option<MetatableFacts> {
    match resolve_aliases(environment, ty, 0) {
        Type::Table(table) => table.metatable_facts().cloned(),
        Type::Union(types) => {
            for ty in types {
                if let Some(facts) = resolve_metatable_facts(environment, &ty) {
                    return Some(facts);
                }
            }

            None
        }
        _ => None,
    }
}

fn call_metamethod_return_types(environment: &TypeEnvironment, ty: &Type) -> Option<Vec<Type>> {
    Some(
        call_metamethod_function(environment, ty)?
            .return_types_or_any()
            .to_vec(),
    )
}

fn call_metamethod_function(environment: &TypeEnvironment, ty: &Type) -> Option<FunctionType> {
    match resolve_aliases(environment, ty, 0) {
        Type::Table(table) => table.metatable_facts().and_then(|facts| {
            let mut function = facts.call.clone()?;

            if !function.parameters.is_empty() {
                function.parameters.remove(0);
            }

            Some(function)
        }),
        Type::Union(types) => types
            .iter()
            .find_map(|ty| call_metamethod_function(environment, ty)),
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => None,
    }
}

fn metamethod_return_or(function: &FunctionType, fallback: Type) -> Type {
    let ty = function
        .return_types_or_any()
        .first()
        .cloned()
        .unwrap_or(Type::Any);

    if matches!(ty, Type::Any) {
        fallback
    } else {
        ty
    }
}

fn arithmetic_metamethod_name(binop: &ast::BinOp) -> Option<&'static str> {
    match binop {
        ast::BinOp::Plus(_) => Some("__add"),
        ast::BinOp::Minus(_) => Some("__sub"),
        ast::BinOp::Star(_) => Some("__mul"),
        ast::BinOp::Slash(_) => Some("__div"),
        _ => None,
    }
}

fn comparison_metamethod_name(binop: &ast::BinOp) -> Option<&'static str> {
    match binop {
        ast::BinOp::TwoEqual(_) | ast::BinOp::TildeEqual(_) => Some("__eq"),
        ast::BinOp::LessThan(_) | ast::BinOp::GreaterThan(_) => Some("__lt"),
        ast::BinOp::LessThanEqual(_) | ast::BinOp::GreaterThanEqual(_) => Some("__le"),
        _ => None,
    }
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

fn span_size(span: SourceSpan) -> (usize, usize) {
    (
        span.end.line.saturating_sub(span.start.line),
        span.end.column.saturating_sub(span.start.column),
    )
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
