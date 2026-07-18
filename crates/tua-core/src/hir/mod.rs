use std::collections::BTreeSet;
use std::path::PathBuf;

use full_moon::ast::{self, Expression, Field, FunctionArgs, LastStmt, Prefix, Suffix};
use full_moon::node::Node;

use crate::{classes::ClassMethod, parser::parse_tua_type_text};
use crate::{
    require_bindings_for_parsed_source, ParsedSource, SourcePosition, SourceSpan, TableFieldType,
    TuaType,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirModule {
    pub path: PathBuf,
    pub symbols: Vec<HirSymbol>,
    pub declarations: Vec<HirDeclaration>,
    pub require_bindings: Vec<HirRequireBinding>,
    pub exports: Vec<HirModuleExport>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct HirSymbolId(pub usize);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSymbol {
    pub id: HirSymbolId,
    pub name: String,
    pub kind: HirSymbolKind,
    pub name_span: SourceSpan,
    pub origin_span: SourceSpan,
    pub parent: Option<HirSymbolId>,
    pub ty: Option<TuaType>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirSymbolKind {
    Class,
    TypeAlias,
    TableField,
    Local,
    Function,
    Parameter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDeclaration {
    pub symbol: HirSymbolId,
    pub kind: HirDeclarationKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirDeclarationKind {
    Class,
    TypeAlias,
    TableField,
    Local,
    Function,
    Parameter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirRequireBinding {
    pub alias: String,
    pub module: String,
    pub kind: crate::ModuleSpecifierKind,
    pub alias_span: SourceSpan,
    pub module_span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirModuleExport {
    pub name: String,
    pub name_span: SourceSpan,
    pub origin_span: SourceSpan,
    pub value: HirExportValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirExportValue {
    Symbol(HirSymbolId),
    Unknown,
}

impl HirModule {
    pub fn symbol(&self, id: HirSymbolId) -> Option<&HirSymbol> {
        self.symbols.get(id.0)
    }

    pub fn symbols(&self) -> &[HirSymbol] {
        &self.symbols
    }

    pub fn definition_for_value(&self, name: &str, position: SourcePosition) -> Option<&HirSymbol> {
        self.symbols
            .iter()
            .filter(|symbol| {
                matches!(
                    symbol.kind,
                    HirSymbolKind::Class
                        | HirSymbolKind::Local
                        | HirSymbolKind::Function
                        | HirSymbolKind::Parameter
                ) && symbol.name == name
                    && starts_before_or_at(symbol.name_span, position)
            })
            .max_by_key(|symbol| position_key(symbol.name_span.start))
    }

    pub fn definition_for_type_alias(&self, name: &str) -> Option<&HirSymbol> {
        self.symbols.iter().find(|symbol| {
            matches!(symbol.kind, HirSymbolKind::Class | HirSymbolKind::TypeAlias)
                && symbol.name == name
        })
    }

    pub fn symbol_at_span(&self, span: SourceSpan) -> Option<&HirSymbol> {
        self.symbols.iter().find(|symbol| symbol.name_span == span)
    }
}

pub fn lower_parsed_source(parsed: &ParsedSource) -> HirModule {
    let class_generated_lines = parsed
        .raw
        .parser_provenance
        .iter()
        .enumerate()
        .filter_map(|(index, provenance)| provenance.class_generated.then_some(index + 1))
        .collect();
    let mut lower = HirLowerer {
        module: HirModule {
            path: parsed.path.clone(),
            symbols: Vec::new(),
            declarations: Vec::new(),
            require_bindings: Vec::new(),
            exports: Vec::new(),
        },
        class_generated_lines,
    };

    lower.collect_classes(parsed);
    lower.collect_type_aliases(parsed);
    lower.collect_block(parsed.ast.nodes(), None);
    lower.collect_require_bindings(parsed);
    lower.collect_exports(parsed);
    lower.module
}

struct HirLowerer {
    module: HirModule,
    class_generated_lines: BTreeSet<usize>,
}

impl HirLowerer {
    fn collect_classes(&mut self, parsed: &ParsedSource) {
        for class in &parsed.class_declarations {
            let class_id = self.push_symbol(HirSymbolInput {
                name: class.name.clone(),
                kind: HirSymbolKind::Class,
                name_span: class.name_span,
                origin_span: class.span,
                parent: None,
                ty: Some(TuaType::Named(class.name.clone())),
            });
            self.push_declaration(class_id, HirDeclarationKind::Class);

            for field in &class.fields {
                let field_id = self.push_symbol(HirSymbolInput {
                    name: field.name.clone(),
                    kind: HirSymbolKind::TableField,
                    name_span: field.name_span,
                    origin_span: field.span,
                    parent: Some(class_id),
                    ty: Some(field.parsed_ty.clone()),
                });
                self.push_declaration(field_id, HirDeclarationKind::TableField);
            }

            for method in class.init.iter().chain(class.methods.iter()) {
                let method_id = self.push_symbol(HirSymbolInput {
                    name: method.name.clone(),
                    kind: HirSymbolKind::Function,
                    name_span: method.name_span,
                    origin_span: method.span,
                    parent: Some(class_id),
                    ty: Some(class_method_type(method)),
                });
                self.push_declaration(method_id, HirDeclarationKind::Function);
            }
        }
    }

    fn collect_type_aliases(&mut self, parsed: &ParsedSource) {
        for alias in &parsed.type_aliases {
            if parsed
                .class_declarations
                .iter()
                .any(|class| class.name == alias.name)
            {
                continue;
            }

            let Some(name_span) = alias.name_span.or(alias.span) else {
                continue;
            };
            let origin_span = alias.span.unwrap_or(name_span);
            let alias_id = self.push_symbol(HirSymbolInput {
                name: alias.name.clone(),
                kind: HirSymbolKind::TypeAlias,
                name_span,
                origin_span,
                parent: None,
                ty: Some(alias.ty.clone()),
            });

            self.push_declaration(alias_id, HirDeclarationKind::TypeAlias);
            self.collect_table_fields(alias_id, &alias.ty);
        }
    }

    fn collect_table_fields(&mut self, parent: HirSymbolId, ty: &TuaType) {
        let TuaType::Table(fields) = ty else {
            return;
        };

        for field in fields {
            let Some(name_span) = field.name_span.or(field.span) else {
                continue;
            };
            let origin_span = field.span.unwrap_or(name_span);
            let field_id = self.push_table_field(parent, field, name_span, origin_span);

            self.push_declaration(field_id, HirDeclarationKind::TableField);
            self.collect_table_fields(field_id, &field.ty);
        }
    }

    fn push_table_field(
        &mut self,
        parent: HirSymbolId,
        field: &TableFieldType,
        name_span: SourceSpan,
        origin_span: SourceSpan,
    ) -> HirSymbolId {
        self.push_symbol(HirSymbolInput {
            name: field.name.clone(),
            kind: HirSymbolKind::TableField,
            name_span,
            origin_span,
            parent: Some(parent),
            ty: Some(field.ty.clone()),
        })
    }

    fn collect_block(&mut self, block: &ast::Block, parent: Option<HirSymbolId>) {
        for stmt in block.stmts() {
            self.collect_stmt(stmt, parent);
        }
    }

    fn collect_stmt(&mut self, stmt: &ast::Stmt, parent: Option<HirSymbolId>) {
        match stmt {
            ast::Stmt::LocalAssignment(local) => self.collect_local_assignment(local, parent),
            ast::Stmt::LocalFunction(function) => {
                if self.is_class_generated(function.name()) {
                    return;
                }
                let function_id = self.collect_local_function(function, parent);
                self.collect_function_body(function.body(), function_id.or(parent));
            }
            ast::Stmt::FunctionDeclaration(function) => {
                let function_id = self.collect_function_declaration(function, parent);
                self.collect_function_body(function.body(), function_id.or(parent));
            }
            ast::Stmt::Do(block) => self.collect_block(block.block(), parent),
            ast::Stmt::If(if_stmt) => {
                self.collect_block(if_stmt.block(), parent);
                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        self.collect_block(else_if.block(), parent);
                    }
                }
                if let Some(else_block) = if_stmt.else_block() {
                    self.collect_block(else_block, parent);
                }
            }
            ast::Stmt::GenericFor(for_stmt) => self.collect_block(for_stmt.block(), parent),
            ast::Stmt::NumericFor(for_stmt) => self.collect_block(for_stmt.block(), parent),
            ast::Stmt::Repeat(repeat) => self.collect_block(repeat.block(), parent),
            ast::Stmt::While(while_stmt) => self.collect_block(while_stmt.block(), parent),
            _ => {}
        }
    }

    fn collect_local_assignment(
        &mut self,
        local: &ast::LocalAssignment,
        parent: Option<HirSymbolId>,
    ) {
        for name in local.names() {
            if self.is_class_generated(name) {
                continue;
            }

            let Some(name_span) = node_span(name) else {
                continue;
            };
            let symbol_id = self.push_symbol(HirSymbolInput {
                name: token_text(name),
                kind: HirSymbolKind::Local,
                name_span,
                origin_span: node_span(local).unwrap_or(name_span),
                parent,
                ty: None,
            });

            self.push_declaration(symbol_id, HirDeclarationKind::Local);
        }
    }

    fn collect_local_function(
        &mut self,
        function: &ast::LocalFunction,
        parent: Option<HirSymbolId>,
    ) -> Option<HirSymbolId> {
        let name_span = node_span(function.name())?;
        let symbol_id = self.push_symbol(HirSymbolInput {
            name: token_text(function.name()),
            kind: HirSymbolKind::Function,
            name_span,
            origin_span: node_span(function).unwrap_or(name_span),
            parent,
            ty: None,
        });

        self.push_declaration(symbol_id, HirDeclarationKind::Function);
        Some(symbol_id)
    }

    fn collect_function_declaration(
        &mut self,
        function: &ast::FunctionDeclaration,
        parent: Option<HirSymbolId>,
    ) -> Option<HirSymbolId> {
        let name_span = node_span(function.name())?;
        let name = function.name().to_string().trim().to_owned();
        let symbol_id = self.push_symbol(HirSymbolInput {
            name,
            kind: HirSymbolKind::Function,
            name_span,
            origin_span: node_span(function).unwrap_or(name_span),
            parent,
            ty: None,
        });

        self.push_declaration(symbol_id, HirDeclarationKind::Function);
        Some(symbol_id)
    }

    fn collect_function_body(&mut self, body: &ast::FunctionBody, parent: Option<HirSymbolId>) {
        for parameter in body.parameters() {
            let ast::Parameter::Name(name) = parameter else {
                continue;
            };
            let Some(name_span) = node_span(name) else {
                continue;
            };
            let symbol_id = self.push_symbol(HirSymbolInput {
                name: token_text(name),
                kind: HirSymbolKind::Parameter,
                name_span,
                origin_span: node_span(body).unwrap_or(name_span),
                parent,
                ty: None,
            });

            self.push_declaration(symbol_id, HirDeclarationKind::Parameter);
        }

        self.collect_block(body.block(), parent);
    }

    fn collect_require_bindings(&mut self, parsed: &ParsedSource) {
        for binding in require_bindings_for_parsed_source(parsed) {
            let alias_span = self
                .module
                .symbols
                .iter()
                .find(|symbol| symbol.name == binding.alias)
                .map(|symbol| symbol.name_span);
            let Some(alias_span) = alias_span else {
                continue;
            };

            self.module.require_bindings.push(HirRequireBinding {
                alias: binding.alias,
                module: binding.module,
                kind: binding.kind,
                alias_span,
                module_span: None,
            });
        }
    }

    fn collect_exports(&mut self, parsed: &ParsedSource) {
        let Some(LastStmt::Return(return_stmt)) = parsed.ast.nodes().last_stmt() else {
            return;
        };
        let Some(Expression::TableConstructor(table)) = return_stmt.returns().iter().next() else {
            return;
        };

        for field in table.fields() {
            let Field::NameKey { key, value, .. } = field else {
                continue;
            };
            let Some(name_span) = node_span(key) else {
                continue;
            };

            self.module.exports.push(HirModuleExport {
                name: token_text(key),
                name_span,
                origin_span: node_span(field).unwrap_or(name_span),
                value: self.export_value(value),
            });
        }
    }

    fn export_value(&self, expression: &Expression) -> HirExportValue {
        let Some(name) = simple_expression_name(expression) else {
            return HirExportValue::Unknown;
        };

        self.module
            .symbols
            .iter()
            .find(|symbol| {
                matches!(
                    symbol.kind,
                    HirSymbolKind::Class
                        | HirSymbolKind::Local
                        | HirSymbolKind::Function
                        | HirSymbolKind::Parameter
                ) && symbol.name == name
            })
            .map(|symbol| HirExportValue::Symbol(symbol.id))
            .unwrap_or(HirExportValue::Unknown)
    }

    fn push_symbol(&mut self, input: HirSymbolInput) -> HirSymbolId {
        let id = HirSymbolId(self.module.symbols.len());

        self.module.symbols.push(HirSymbol {
            id,
            name: input.name,
            kind: input.kind,
            name_span: input.name_span,
            origin_span: input.origin_span,
            parent: input.parent,
            ty: input.ty,
        });

        id
    }

    fn push_declaration(&mut self, symbol: HirSymbolId, kind: HirDeclarationKind) {
        self.module
            .declarations
            .push(HirDeclaration { symbol, kind });
    }

    fn is_class_generated(&self, node: &impl Node) -> bool {
        node.start_position()
            .is_some_and(|position| self.class_generated_lines.contains(&position.line()))
    }
}

fn class_method_type(method: &ClassMethod) -> TuaType {
    parse_tua_type_text(&format!(
        "({}) -> {}",
        method.parameters,
        method.return_type.as_deref().unwrap_or("any")
    ))
}

struct HirSymbolInput {
    name: String,
    kind: HirSymbolKind,
    name_span: SourceSpan,
    origin_span: SourceSpan,
    parent: Option<HirSymbolId>,
    ty: Option<TuaType>,
}

fn simple_expression_name(expression: &Expression) -> Option<String> {
    let text = expression.to_string();
    let text = text.trim();

    if is_identifier(text) {
        return Some(text.to_owned());
    }

    match expression {
        Expression::FunctionCall(call) => {
            let Prefix::Name(name) = call.prefix() else {
                return None;
            };
            if call.suffixes().any(|suffix| {
                !matches!(
                    suffix,
                    Suffix::Call(ast::Call::AnonymousCall(FunctionArgs::Parentheses { .. }))
                )
            }) {
                return None;
            }

            Some(token_text(name))
        }
        _ => None,
    }
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn starts_before_or_at(span: SourceSpan, position: SourcePosition) -> bool {
    position_key(span.start) <= position_key(position)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

#[cfg(test)]
mod tests;
