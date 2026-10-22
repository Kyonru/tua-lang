use std::collections::BTreeMap;
use std::path::Path;

use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, Index, LastStmt, Parameter, Prefix,
    Suffix, Var,
};
use full_moon::node::Node;
use full_moon::tokenizer::TokenReference;

use crate::hir::{lower_parsed_source, HirSymbolKind};
use crate::{lua, ParsedSource, SourcePosition, SourceSpan};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeMap {
    declarations: Vec<ScopeDeclaration>,
    references: Vec<ScopeReference>,
    globals: BTreeMap<String, ScopeGlobal>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ScopeSymbolId(pub usize);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeDeclaration {
    pub id: ScopeSymbolId,
    pub name: String,
    pub kind: ScopeDeclarationKind,
    pub name_span: SourceSpan,
    pub declaration_span: SourceSpan,
    pub scope_span: SourceSpan,
    pub parent: Option<ScopeSymbolId>,
    pub shadows: Option<ScopeSymbolId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScopeDeclarationKind {
    Local,
    Parameter,
    Function,
    ModuleAlias { module: String },
    TableField,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeReference {
    pub name: String,
    pub span: SourceSpan,
    pub resolution: ScopeResolution,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScopeResolution {
    Symbol(ScopeSymbolId),
    Global(ScopeGlobal),
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeGlobal {
    pub name: String,
    pub kind: ScopeGlobalKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeGlobalKind {
    Configured,
    LuaBuiltin,
    LoveBuiltin,
    Assigned,
}

impl ScopeMap {
    pub fn declarations(&self) -> &[ScopeDeclaration] {
        &self.declarations
    }

    pub fn references(&self) -> &[ScopeReference] {
        &self.references
    }

    pub fn globals(&self) -> &BTreeMap<String, ScopeGlobal> {
        &self.globals
    }

    pub fn declaration(&self, id: ScopeSymbolId) -> Option<&ScopeDeclaration> {
        self.declarations.get(id.0)
    }

    pub fn declaration_at_span(&self, span: SourceSpan) -> Option<&ScopeDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.name_span == span)
    }

    pub fn symbol_at_position(&self, position: SourcePosition) -> Option<ScopeSymbolId> {
        self.declarations
            .iter()
            .find(|declaration| contains_position(declaration.name_span, position))
            .map(|declaration| declaration.id)
            .or_else(|| {
                self.references.iter().find_map(|reference| {
                    (contains_position(reference.span, position))
                        .then_some(reference.resolution.symbol_id())
                        .flatten()
                })
            })
    }

    pub fn references_to_symbol(
        &self,
        symbol: ScopeSymbolId,
        include_declaration: bool,
    ) -> Vec<ScopeReference> {
        let mut references = Vec::new();

        if include_declaration {
            if let Some(declaration) = self.declaration(symbol) {
                references.push(ScopeReference {
                    name: declaration.name.clone(),
                    span: declaration.name_span,
                    resolution: ScopeResolution::Symbol(symbol),
                });
            }
        }

        references.extend(
            self.references
                .iter()
                .filter(|reference| reference.resolution.symbol_id() == Some(symbol))
                .cloned(),
        );
        references.sort_by_key(|reference| position_key(reference.span.start));
        references
    }

    pub fn unresolved_references(&self) -> impl Iterator<Item = &ScopeReference> {
        self.references
            .iter()
            .filter(|reference| reference.resolution == ScopeResolution::Unresolved)
    }
}

impl ScopeResolution {
    pub fn symbol_id(&self) -> Option<ScopeSymbolId> {
        match self {
            ScopeResolution::Symbol(id) => Some(*id),
            ScopeResolution::Global(_) | ScopeResolution::Unresolved => None,
        }
    }
}

pub fn scope_map_parsed_source(parsed: &ParsedSource) -> ScopeMap {
    scope_map_parsed_source_with_globals(parsed, std::iter::empty::<String>())
}

pub fn scope_map_parsed_source_with_globals(
    parsed: &ParsedSource,
    configured_globals: impl IntoIterator<Item = impl Into<String>>,
) -> ScopeMap {
    let mut builder = ScopeBuilder::new(parsed, configured_globals);

    builder.collect_table_fields();
    builder.analyze_block(parsed.ast.nodes(), None);
    builder.finish()
}

#[derive(Clone, Debug)]
struct LexicalScope {
    span: SourceSpan,
    bindings: BTreeMap<String, ScopeSymbolId>,
}

struct ScopeBuilder<'a> {
    parsed: &'a ParsedSource,
    declarations: Vec<ScopeDeclaration>,
    references: Vec<ScopeReference>,
    scopes: Vec<LexicalScope>,
    globals: BTreeMap<String, ScopeGlobal>,
}

impl<'a> ScopeBuilder<'a> {
    fn new(
        parsed: &'a ParsedSource,
        configured_globals: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut globals = configured_globals
            .into_iter()
            .map(|name| {
                let name = name.into();
                (
                    name.clone(),
                    ScopeGlobal {
                        name,
                        kind: ScopeGlobalKind::Configured,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        let builtin_modules = crate::builtin_module::MODULES
            .iter()
            .map(|module| module.name);
        for name in ["love", "require", "_G", crate::ecs::MODULE_NAME]
            .into_iter()
            .chain(builtin_modules)
        {
            let kind = if name == "love" {
                ScopeGlobalKind::LoveBuiltin
            } else {
                ScopeGlobalKind::LuaBuiltin
            };

            globals.entry(name.to_owned()).or_insert(ScopeGlobal {
                name: name.to_owned(),
                kind,
            });
        }

        Self {
            parsed,
            declarations: Vec::new(),
            references: Vec::new(),
            scopes: Vec::new(),
            globals,
        }
    }

    fn finish(self) -> ScopeMap {
        ScopeMap {
            declarations: self.declarations,
            references: self.references,
            globals: self.globals,
        }
    }

    fn collect_table_fields(&mut self) {
        let hir = lower_parsed_source(self.parsed);

        for symbol in hir.symbols() {
            if symbol.kind != HirSymbolKind::TableField {
                continue;
            }

            let id = self.push_declaration(ScopeDeclarationInput {
                name: symbol.name.clone(),
                kind: ScopeDeclarationKind::TableField,
                name_span: symbol.name_span,
                declaration_span: symbol.origin_span,
                scope_span: symbol.origin_span,
                parent: None,
            });

            debug_assert_eq!(id.0, self.declarations.len() - 1);
        }
    }

    fn analyze_block(&mut self, block: &'a ast::Block, parent: Option<ScopeSymbolId>) {
        let span = node_span(block).unwrap_or_else(|| self.parsed.span.unwrap_or(default_span()));

        self.enter_scope(span);

        for stmt in block.stmts() {
            self.analyze_stmt(stmt, parent);
        }

        if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
            for expression in return_stmt.returns() {
                self.analyze_expression(expression);
            }
        }

        self.exit_scope();
    }

    fn analyze_child_block(&mut self, block: &'a ast::Block, parent: Option<ScopeSymbolId>) {
        self.analyze_block(block, parent);
    }

    fn analyze_stmt(&mut self, stmt: &'a ast::Stmt, parent: Option<ScopeSymbolId>) {
        match stmt {
            ast::Stmt::Assignment(assignment) => self.analyze_assignment(assignment),
            ast::Stmt::CompoundAssignment(assignment) => {
                self.analyze_expression(assignment.rhs());
                self.analyze_assignment_target(assignment.lhs());
            }
            ast::Stmt::FunctionCall(function_call) => self.analyze_function_call(function_call),
            ast::Stmt::LocalAssignment(local) => self.analyze_local_assignment(local, parent),
            ast::Stmt::LocalFunction(function) => self.analyze_local_function(function, parent),
            ast::Stmt::FunctionDeclaration(function) => {
                self.analyze_function_declaration(function, parent)
            }
            ast::Stmt::Do(block) => self.analyze_child_block(block.block(), parent),
            ast::Stmt::If(if_stmt) => {
                self.analyze_expression(if_stmt.condition());
                self.analyze_child_block(if_stmt.block(), parent);

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        self.analyze_expression(else_if.condition());
                        self.analyze_child_block(else_if.block(), parent);
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    self.analyze_child_block(else_block, parent);
                }
            }
            ast::Stmt::GenericFor(for_stmt) => {
                for expression in for_stmt.expressions() {
                    self.analyze_expression(expression);
                }

                let span = node_span(for_stmt.block()).unwrap_or_else(default_span);
                self.enter_scope(span);

                for name in for_stmt.names() {
                    self.declare_token(
                        token_text(name),
                        name,
                        ScopeDeclarationKind::Local,
                        node_span(for_stmt).unwrap_or(span),
                        parent,
                    );
                }

                self.analyze_block_contents_without_new_scope(for_stmt.block(), parent);
                self.exit_scope();
            }
            ast::Stmt::NumericFor(for_stmt) => {
                self.analyze_expression(for_stmt.start());
                self.analyze_expression(for_stmt.end());

                if let Some(step) = for_stmt.step() {
                    self.analyze_expression(step);
                }

                let span = node_span(for_stmt.block()).unwrap_or_else(default_span);
                self.enter_scope(span);
                self.declare_token(
                    token_text(for_stmt.index_variable()),
                    for_stmt.index_variable(),
                    ScopeDeclarationKind::Local,
                    node_span(for_stmt).unwrap_or(span),
                    parent,
                );
                self.analyze_block_contents_without_new_scope(for_stmt.block(), parent);
                self.exit_scope();
            }
            ast::Stmt::Repeat(repeat) => {
                let span = node_span(repeat.block()).unwrap_or_else(default_span);
                self.enter_scope(span);
                self.analyze_block_contents_without_new_scope(repeat.block(), parent);
                self.analyze_expression(repeat.until());
                self.exit_scope();
            }
            ast::Stmt::While(while_stmt) => {
                self.analyze_expression(while_stmt.condition());
                self.analyze_child_block(while_stmt.block(), parent);
            }
            _ => {}
        }
    }

    fn analyze_block_contents_without_new_scope(
        &mut self,
        block: &'a ast::Block,
        parent: Option<ScopeSymbolId>,
    ) {
        for stmt in block.stmts() {
            self.analyze_stmt(stmt, parent);
        }

        if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
            for expression in return_stmt.returns() {
                self.analyze_expression(expression);
            }
        }
    }

    fn analyze_assignment(&mut self, assignment: &'a ast::Assignment) {
        for expression in assignment.expressions() {
            self.analyze_expression(expression);
        }

        for variable in assignment.variables() {
            self.analyze_assignment_target(variable);
        }
    }

    fn analyze_assignment_target(&mut self, variable: &'a Var) {
        match variable {
            Var::Name(name) => {
                let name_text = token_text(name);

                if self.resolve_local(&name_text).is_none() {
                    self.push_global(name_text, ScopeGlobalKind::Assigned);
                } else {
                    self.read_token(name);
                }
            }
            Var::Expression(expression) => self.analyze_var_expression_base(expression),
            _ => {}
        }
    }

    fn analyze_local_assignment(
        &mut self,
        local: &'a ast::LocalAssignment,
        parent: Option<ScopeSymbolId>,
    ) {
        let expressions = local.expressions().iter().collect::<Vec<_>>();

        for expression in &expressions {
            self.analyze_expression(expression);
        }

        let names = local.names();

        for (index, name) in names.iter().enumerate() {
            let kind = expressions
                .get(index)
                .and_then(|expression| {
                    module_specifier_from_expression(expression, names.len(), expressions.len())
                })
                .map(|module| ScopeDeclarationKind::ModuleAlias { module })
                .unwrap_or(ScopeDeclarationKind::Local);

            self.declare_token(
                token_text(name),
                name,
                kind,
                node_span(local).unwrap_or_else(default_span),
                parent,
            );
        }
    }

    fn analyze_local_function(
        &mut self,
        function: &'a ast::LocalFunction,
        parent: Option<ScopeSymbolId>,
    ) {
        let function_id = self.declare_token(
            token_text(function.name()),
            function.name(),
            ScopeDeclarationKind::Function,
            node_span(function).unwrap_or_else(default_span),
            parent,
        );

        self.analyze_function_body(function.body(), function_id.or(parent), None);
    }

    fn analyze_function_declaration(
        &mut self,
        function: &'a ast::FunctionDeclaration,
        parent: Option<ScopeSymbolId>,
    ) {
        let names = function.name().names().iter().collect::<Vec<_>>();

        if let Some(root) = names.first() {
            let root_name = token_text(root);

            if names.len() == 1 && function.name().method_name().is_none() {
                self.push_global(root_name, ScopeGlobalKind::Assigned);
            } else {
                self.read_token(root);
            }
        }

        let implicit_self = function.name().method_name().and_then(node_span);

        self.analyze_function_body(function.body(), parent, implicit_self);
    }

    fn analyze_function_body(
        &mut self,
        body: &'a ast::FunctionBody,
        parent: Option<ScopeSymbolId>,
        implicit_self: Option<SourceSpan>,
    ) {
        let span = node_span(body.block()).unwrap_or_else(default_span);

        self.enter_scope(span);

        if let Some(self_span) = implicit_self {
            self.declare_synthetic(
                "self".to_owned(),
                ScopeDeclarationKind::Parameter,
                self_span,
                parent,
            );
        }

        for parameter in body.parameters() {
            if let Parameter::Name(name) = parameter {
                self.declare_token(
                    token_text(name),
                    name,
                    ScopeDeclarationKind::Parameter,
                    node_span(body).unwrap_or(span),
                    parent,
                );
            }
        }

        self.analyze_block_contents_without_new_scope(body.block(), parent);
        self.exit_scope();
    }

    fn analyze_expression(&mut self, expression: &'a Expression) {
        match expression {
            Expression::BinaryOperator { lhs, rhs, .. } => {
                self.analyze_expression(lhs);
                self.analyze_expression(rhs);
            }
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. } => self.analyze_expression(expression),
            Expression::UnaryOperator { expression, .. } => self.analyze_expression(expression),
            Expression::Function(function) => {
                self.analyze_function_body(function.body(), None, None)
            }
            Expression::FunctionCall(function_call) => self.analyze_function_call(function_call),
            Expression::TableConstructor(table) => self.analyze_table_constructor(table),
            Expression::Var(var) => self.analyze_var(var),
            _ => {}
        }
    }

    fn analyze_table_constructor(&mut self, table: &'a ast::TableConstructor) {
        for field in table.fields() {
            match field {
                Field::ExpressionKey { key, value, .. } => {
                    self.analyze_expression(key);
                    self.analyze_expression(value);
                }
                Field::NameKey { value, .. } => self.analyze_expression(value),
                Field::NoKey(expression) => self.analyze_expression(expression),
                _ => {}
            }
        }
    }

    fn analyze_var(&mut self, var: &'a Var) {
        match var {
            Var::Name(name) => self.read_token(name),
            Var::Expression(expression) => self.analyze_var_expression(expression),
            _ => {}
        }
    }

    fn analyze_var_expression(&mut self, expression: &'a ast::VarExpression) {
        self.analyze_prefix(expression.prefix());
        self.analyze_suffixes(expression.suffixes());
    }

    fn analyze_var_expression_base(&mut self, expression: &'a ast::VarExpression) {
        self.analyze_prefix(expression.prefix());

        for suffix in expression.suffixes() {
            if let Suffix::Index(Index::Brackets { expression, .. }) = suffix {
                self.analyze_expression(expression);
            }
        }
    }

    fn analyze_function_call(&mut self, function_call: &'a FunctionCall) {
        self.analyze_prefix(function_call.prefix());
        self.analyze_suffixes(function_call.suffixes());
    }

    fn analyze_prefix(&mut self, prefix: &'a Prefix) {
        match prefix {
            Prefix::Name(name) => self.read_token(name),
            Prefix::Expression(expression) => self.analyze_expression(expression),
            _ => {}
        }
    }

    fn analyze_suffixes(&mut self, suffixes: impl Iterator<Item = &'a Suffix>) {
        for suffix in suffixes {
            match suffix {
                Suffix::Call(call) => self.analyze_call(call),
                Suffix::Index(Index::Brackets { expression, .. }) => {
                    self.analyze_expression(expression);
                }
                Suffix::Index(Index::Dot { .. }) => {}
                _ => {}
            }
        }
    }

    fn analyze_call(&mut self, call: &'a Call) {
        match call {
            Call::AnonymousCall(args) => self.analyze_function_args(args),
            Call::MethodCall(method) => self.analyze_function_args(method.args()),
            _ => {}
        }
    }

    fn analyze_function_args(&mut self, args: &'a FunctionArgs) {
        match args {
            FunctionArgs::Parentheses { arguments, .. } => {
                for argument in arguments {
                    self.analyze_expression(argument);
                }
            }
            FunctionArgs::TableConstructor(table) => self.analyze_table_constructor(table),
            FunctionArgs::String(_) => {}
            _ => {}
        }
    }

    fn enter_scope(&mut self, span: SourceSpan) {
        self.scopes.push(LexicalScope {
            span,
            bindings: BTreeMap::new(),
        });
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare_token(
        &mut self,
        name: String,
        token: &TokenReference,
        kind: ScopeDeclarationKind,
        declaration_span: SourceSpan,
        parent: Option<ScopeSymbolId>,
    ) -> Option<ScopeSymbolId> {
        let name_span = node_span(token)?;
        let scope_span = self
            .scopes
            .last()
            .map(|scope| scope.span)
            .unwrap_or_else(default_span);
        let id = self.push_declaration(ScopeDeclarationInput {
            name: name.clone(),
            kind,
            name_span,
            declaration_span,
            scope_span,
            parent,
        });

        if let Some(scope) = self.scopes.last_mut() {
            scope.bindings.insert(name, id);
        }

        Some(id)
    }

    fn declare_synthetic(
        &mut self,
        name: String,
        kind: ScopeDeclarationKind,
        span: SourceSpan,
        parent: Option<ScopeSymbolId>,
    ) -> Option<ScopeSymbolId> {
        let id = self.push_declaration(ScopeDeclarationInput {
            name: name.clone(),
            kind,
            name_span: span,
            declaration_span: span,
            scope_span: self
                .scopes
                .last()
                .map(|scope| scope.span)
                .unwrap_or_else(default_span),
            parent,
        });

        if let Some(scope) = self.scopes.last_mut() {
            scope.bindings.insert(name, id);
        }

        Some(id)
    }

    fn push_declaration(&mut self, input: ScopeDeclarationInput) -> ScopeSymbolId {
        let id = ScopeSymbolId(self.declarations.len());
        let shadows = self.resolve_local(&input.name);

        self.declarations.push(ScopeDeclaration {
            id,
            name: input.name,
            kind: input.kind,
            name_span: input.name_span,
            declaration_span: input.declaration_span,
            scope_span: input.scope_span,
            parent: input.parent,
            shadows,
        });

        id
    }

    fn read_token(&mut self, token: &TokenReference) {
        let name = token_text(token);
        let Some(span) = node_span(token) else {
            return;
        };
        let resolution = self
            .resolve_local(&name)
            .map(ScopeResolution::Symbol)
            .or_else(|| self.global_resolution(&name))
            .unwrap_or(ScopeResolution::Unresolved);

        self.references.push(ScopeReference {
            name,
            span,
            resolution,
        });
    }

    fn resolve_local(&self, name: &str) -> Option<ScopeSymbolId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.bindings.get(name).copied())
    }

    fn global_resolution(&mut self, name: &str) -> Option<ScopeResolution> {
        if let Some(global) = self.globals.get(name).cloned() {
            return Some(ScopeResolution::Global(global));
        }

        let kind = if name == "love" {
            Some(ScopeGlobalKind::LoveBuiltin)
        } else if lua::hover_item(&[name.to_owned()]).is_some() {
            Some(ScopeGlobalKind::LuaBuiltin)
        } else {
            None
        }?;

        let global = ScopeGlobal {
            name: name.to_owned(),
            kind,
        };

        self.globals.insert(name.to_owned(), global.clone());

        Some(ScopeResolution::Global(global))
    }

    fn push_global(&mut self, name: String, kind: ScopeGlobalKind) {
        if let Some(global) = self.globals.get_mut(&name) {
            if kind == ScopeGlobalKind::Assigned {
                global.kind = ScopeGlobalKind::Assigned;
            }
            return;
        }

        self.globals
            .insert(name.clone(), ScopeGlobal { name, kind });
    }
}

struct ScopeDeclarationInput {
    name: String,
    kind: ScopeDeclarationKind,
    name_span: SourceSpan,
    declaration_span: SourceSpan,
    scope_span: SourceSpan,
    parent: Option<ScopeSymbolId>,
}

fn module_specifier_from_expression(
    expression: &Expression,
    name_count: usize,
    expression_count: usize,
) -> Option<String> {
    let Expression::FunctionCall(call) = expression else {
        return None;
    };
    let Prefix::Name(name) = call.prefix() else {
        return None;
    };
    let call_name = token_text(name);

    match call_name.as_str() {
        "require" => {}
        "import" if name_count == 1 && expression_count == 1 => {}
        _ => return None,
    }

    let Some(Suffix::Call(Call::AnonymousCall(args))) = call.suffixes().next() else {
        return None;
    };

    match args {
        FunctionArgs::Parentheses { arguments, .. } => {
            let Some(Expression::String(token)) = arguments.iter().next() else {
                return None;
            };

            let module = unquote_string(&token.to_string());

            if call_name == "import" && !is_static_import_specifier(&module) {
                return None;
            }

            Some(module)
        }
        FunctionArgs::String(token) if call_name == "require" => {
            Some(unquote_string(&token.to_string()))
        }
        FunctionArgs::TableConstructor(_) => None,
        _ => None,
    }
}

fn is_static_import_specifier(module: &str) -> bool {
    is_child_relative_import_specifier(module) || is_project_import_specifier(module)
}

fn is_child_relative_import_specifier(module: &str) -> bool {
    module.starts_with("./") && !has_parent_path_component(module)
}

fn is_project_import_specifier(module: &str) -> bool {
    !module.is_empty() && !module.starts_with('.') && !has_parent_path_component(module)
}

fn has_parent_path_component(module: &str) -> bool {
    Path::new(module)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
}

fn unquote_string(raw: &str) -> String {
    raw.trim().trim_matches('"').trim_matches('\'').to_owned()
}

fn token_text(token: &TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn default_span() -> SourceSpan {
    SourceSpan {
        start: SourcePosition { line: 1, column: 1 },
        end: SourcePosition { line: 1, column: 1 },
    }
}

fn contains_position(span: SourceSpan, position: SourcePosition) -> bool {
    position_key(span.start) <= position_key(position)
        && position_key(position) <= position_key(span.end)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests;
