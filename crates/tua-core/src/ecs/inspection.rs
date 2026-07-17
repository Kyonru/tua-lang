use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use full_moon::ast::{self, Call, Expression, Field, FunctionArgs, FunctionCall, Suffix, Var};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};
use full_moon::visitors::{Visit, Visitor};

use crate::ast_utils::{
    assignment_expression_for_local, field_chain_before_first_call,
    plain_field_chain_from_expression, plain_field_chain_from_var, token_text,
};
use crate::semantic::SemanticModel;
use crate::{ParsedSource, SourcePosition, SourceSpan, TypeEnvironment};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EcsGraph {
    pub components: Vec<EcsComponent>,
    pub systems: Vec<EcsSystem>,
    pub queries: Vec<EcsQuery>,
    pub insights: Vec<EcsInsight>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EcsComponent {
    pub id: String,
    pub path: PathBuf,
    pub local_name: String,
    pub runtime_name: String,
    pub fields: Vec<EcsComponentField>,
    pub span: SourceSpan,
    pub name_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EcsComponentField {
    pub name: String,
    pub type_label: String,
    pub span: SourceSpan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EcsAccessKind {
    Filter,
    Read,
    Write,
    ReadWrite,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EcsComponentUse {
    pub reference: String,
    pub component_id: Option<String>,
    pub access: EcsAccessKind,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EcsSystem {
    pub id: String,
    pub path: PathBuf,
    pub name: Option<String>,
    pub components: Vec<EcsComponentUse>,
    pub span: SourceSpan,
    pub name_span: Option<SourceSpan>,
    pub callback_span: Option<SourceSpan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EcsQueryKind {
    Standalone,
    System,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EcsQuery {
    pub id: String,
    pub path: PathBuf,
    pub name: Option<String>,
    pub kind: EcsQueryKind,
    pub owner_system_id: Option<String>,
    pub components: Vec<EcsComponentUse>,
    pub span: SourceSpan,
    pub name_span: Option<SourceSpan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EcsInsightKind {
    WideQueryAllocation,
    CallbackAllocation,
    ResourceConstruction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EcsInsight {
    pub id: String,
    pub kind: EcsInsightKind,
    pub title: String,
    pub message: String,
    pub path: PathBuf,
    pub span: SourceSpan,
    pub subject_id: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct EcsFileIndex {
    pub components: Vec<EcsComponent>,
    pub systems: Vec<EcsSystem>,
    pub queries: Vec<EcsQuery>,
    pub insights: Vec<EcsInsight>,
}

#[derive(Clone, Debug)]
struct LocalBinding {
    name: String,
    name_span: SourceSpan,
}

pub(crate) fn ecs_file_index(parsed: &ParsedSource, environment: &TypeEnvironment) -> EcsFileIndex {
    let mut bindings = LocalCallBindingVisitor::default();
    bindings.visit_ast(&parsed.ast);

    let mut visitor = EcsVisitor {
        path: &parsed.path,
        environment,
        semantic: SemanticModel::new(parsed, environment),
        bindings: bindings.bindings,
        index: EcsFileIndex::default(),
    };

    visitor.visit_ast(&parsed.ast);
    visitor.index
}

pub(crate) fn ecs_graph<'a>(indexes: impl IntoIterator<Item = &'a EcsFileIndex>) -> EcsGraph {
    let mut graph = EcsGraph::default();

    for index in indexes {
        graph.components.extend(index.components.iter().cloned());
        graph.systems.extend(index.systems.iter().cloned());
        graph.queries.extend(index.queries.iter().cloned());
        graph.insights.extend(index.insights.iter().cloned());
    }

    let component_ids = graph
        .components
        .iter()
        .map(|component| {
            (
                (component.path.clone(), component.local_name.clone()),
                component.id.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for system in &mut graph.systems {
        resolve_component_uses(&component_ids, &system.path, &mut system.components);
    }
    for query in &mut graph.queries {
        resolve_component_uses(&component_ids, &query.path, &mut query.components);
    }

    graph.components.sort_by(|left, right| {
        left.runtime_name
            .cmp(&right.runtime_name)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| {
                position_key(left.name_span.start).cmp(&position_key(right.name_span.start))
            })
    });
    graph.systems.sort_by(|left, right| {
        item_order(
            &left.path,
            left.span,
            &left.id,
            &right.path,
            right.span,
            &right.id,
        )
    });
    graph.queries.sort_by(|left, right| {
        item_order(
            &left.path,
            left.span,
            &left.id,
            &right.path,
            right.span,
            &right.id,
        )
    });
    graph.insights.sort_by(|left, right| {
        item_order(
            &left.path,
            left.span,
            &left.id,
            &right.path,
            right.span,
            &right.id,
        )
    });

    graph
}

fn resolve_component_uses(
    component_ids: &BTreeMap<(PathBuf, String), String>,
    path: &Path,
    uses: &mut [EcsComponentUse],
) {
    for component_use in uses {
        component_use.component_id = component_ids
            .get(&(path.to_path_buf(), component_use.reference.clone()))
            .cloned();
    }
}

fn item_order(
    left_path: &Path,
    left_span: SourceSpan,
    left_id: &str,
    right_path: &Path,
    right_span: SourceSpan,
    right_id: &str,
) -> std::cmp::Ordering {
    left_path
        .cmp(right_path)
        .then_with(|| position_key(left_span.start).cmp(&position_key(right_span.start)))
        .then_with(|| left_id.cmp(right_id))
}

#[derive(Default)]
struct LocalCallBindingVisitor {
    bindings: BTreeMap<(usize, usize), LocalBinding>,
}

impl Visitor for LocalCallBindingVisitor {
    fn visit_local_assignment(&mut self, local: &ast::LocalAssignment) {
        let names = local.names().iter().collect::<Vec<_>>();
        let expressions = local.expressions().iter().collect::<Vec<_>>();

        for (index, name) in names.into_iter().enumerate() {
            let Some((expression, _)) = assignment_expression_for_local(&expressions, index) else {
                continue;
            };
            let Expression::FunctionCall(call) = expression else {
                continue;
            };
            let (Some(name_span), Some(call_span)) = (node_span(name), node_span(call)) else {
                continue;
            };

            self.bindings.insert(
                position_key(call_span.start),
                LocalBinding {
                    name: token_text(name),
                    name_span,
                },
            );
        }
    }
}

struct EcsVisitor<'a> {
    path: &'a Path,
    environment: &'a TypeEnvironment,
    semantic: SemanticModel<'a>,
    bindings: BTreeMap<(usize, usize), LocalBinding>,
    index: EcsFileIndex,
}

impl Visitor for EcsVisitor<'_> {
    fn visit_function_call(&mut self, call: &FunctionCall) {
        let Some(parts) = field_chain_before_first_call(call) else {
            return;
        };
        let Some(span) = node_span(call) else {
            return;
        };
        let binding = self.bindings.get(&position_key(span.start)).cloned();

        match parts.as_slice() {
            [module, function]
                if module == "ecs"
                    && function == "component"
                    && self.receiver_has_ecs_member(&parts, span.start) =>
            {
                self.record_component(call, span, binding);
            }
            [module, function]
                if module == "ecs"
                    && function == "system"
                    && self.receiver_has_ecs_member(&parts, span.start) =>
            {
                self.record_system(call, span, binding);
            }
            _ if parts.last().is_some_and(|part| part == "query")
                && self.receiver_has_ecs_member(&parts, span.start) =>
            {
                self.record_query(call, span, binding);
            }
            _ => {}
        }
    }
}

impl EcsVisitor<'_> {
    fn receiver_has_ecs_member(&self, parts: &[String], position: SourcePosition) -> bool {
        let Some((member, receiver)) = parts.split_last() else {
            return false;
        };
        let Some(receiver_type) = self.semantic.type_of_field_chain(receiver, position) else {
            return false;
        };

        super::typed_member(self.environment, &receiver_type, member).is_some()
    }

    fn record_component(
        &mut self,
        call: &FunctionCall,
        span: SourceSpan,
        binding: Option<LocalBinding>,
    ) {
        let Some(binding) = binding else {
            return;
        };
        let Some((runtime_name, _)) = string_expression(argument_expression(call, 0)) else {
            return;
        };

        self.index.components.push(EcsComponent {
            id: graph_id("component", self.path, binding.name_span.start),
            path: self.path.to_path_buf(),
            local_name: binding.name,
            runtime_name,
            fields: component_fields(argument_expression(call, 1)),
            span,
            name_span: binding.name_span,
        });
    }

    fn record_system(
        &mut self,
        call: &FunctionCall,
        span: SourceSpan,
        binding: Option<LocalBinding>,
    ) {
        let mut components = component_uses(argument_expression(call, 1));
        let callback = match argument_expression(call, 2) {
            Some(Expression::Function(function)) => Some(function.as_ref()),
            _ => None,
        };
        let callback_span = callback.and_then(node_span);
        let callback_facts = callback.map(callback_facts).unwrap_or_default();
        let parameters = callback
            .map(|function| {
                function
                    .body()
                    .parameters()
                    .iter()
                    .filter_map(|parameter| match parameter {
                        ast::Parameter::Name(name) => Some(token_text(name)),
                        ast::Parameter::Ellipsis(_) => None,
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for (index, component) in components.iter_mut().enumerate() {
            component.access = parameters
                .get(index + 1)
                .map(|parameter| callback_facts.access_for(parameter))
                .unwrap_or(EcsAccessKind::Filter);
        }

        let id = graph_id("system", self.path, span.start);
        let name = binding.as_ref().map(|binding| binding.name.clone());
        let name_span = binding.as_ref().map(|binding| binding.name_span);
        let query_id = format!("{id}:query");

        self.index.systems.push(EcsSystem {
            id: id.clone(),
            path: self.path.to_path_buf(),
            name: name.clone(),
            components: components.clone(),
            span,
            name_span,
            callback_span,
        });
        self.index.queries.push(EcsQuery {
            id: query_id.clone(),
            path: self.path.to_path_buf(),
            name,
            kind: EcsQueryKind::System,
            owner_system_id: Some(id.clone()),
            components: components.clone(),
            span,
            name_span,
        });
        self.record_wide_query_insight(&query_id, span, components.len());

        let Some(callback_span) = callback_span else {
            return;
        };
        let allocation_count = callback_facts.table_allocations + callback_facts.nested_functions;

        if allocation_count > 0 {
            self.index.insights.push(EcsInsight {
                id: format!("{id}:callback-allocation"),
                kind: EcsInsightKind::CallbackAllocation,
                title: "Per-entity callback allocations".to_owned(),
                message: format!(
                    "This system creates {allocation_count} table or closure allocation(s) inside its per-entity callback. Consider reusing data or an objectPool if profiling shows garbage-collection pressure."
                ),
                path: self.path.to_path_buf(),
                span: callback_span,
                subject_id: id.clone(),
            });
        }

        if !callback_facts.resource_constructors.is_empty() {
            self.index.insights.push(EcsInsight {
                id: format!("{id}:resource-construction"),
                kind: EcsInsightKind::ResourceConstruction,
                title: "Resource construction in system update".to_owned(),
                message: format!(
                    "This per-entity callback calls {}. Load reusable LOVE resources outside the system update when possible.",
                    callback_facts.resource_constructors.join(", ")
                ),
                path: self.path.to_path_buf(),
                span: callback_span,
                subject_id: id,
            });
        }
    }

    fn record_query(
        &mut self,
        call: &FunctionCall,
        span: SourceSpan,
        binding: Option<LocalBinding>,
    ) {
        let components = component_uses(argument_expression(call, 0));
        let id = graph_id("query", self.path, span.start);
        let name = binding.as_ref().map(|binding| binding.name.clone());
        let name_span = binding.map(|binding| binding.name_span);

        self.index.queries.push(EcsQuery {
            id: id.clone(),
            path: self.path.to_path_buf(),
            name,
            kind: EcsQueryKind::Standalone,
            owner_system_id: None,
            components: components.clone(),
            span,
            name_span,
        });
        self.record_wide_query_insight(&id, span, components.len());
    }

    fn record_wide_query_insight(&mut self, subject_id: &str, span: SourceSpan, count: usize) {
        if count <= 3 {
            return;
        }

        self.index.insights.push(EcsInsight {
            id: format!("{subject_id}:wide-query"),
            kind: EcsInsightKind::WideQueryAllocation,
            title: "Wide query allocates per match".to_owned(),
            message: format!(
                "The bundled ECS runtime creates a temporary values table for each entity matched by this {count}-component query. Split the query only when profiling shows allocation pressure."
            ),
            path: self.path.to_path_buf(),
            span,
            subject_id: subject_id.to_owned(),
        });
    }
}

#[derive(Default)]
struct CallbackFacts {
    references: BTreeMap<String, usize>,
    writes: BTreeMap<String, usize>,
    compound_writes: BTreeMap<String, usize>,
    method_calls: BTreeMap<String, usize>,
    table_allocations: usize,
    nested_functions: usize,
    resource_constructors: Vec<String>,
}

impl CallbackFacts {
    fn access_for(&self, name: &str) -> EcsAccessKind {
        let references = self.references.get(name).copied().unwrap_or(0);
        let writes = self.writes.get(name).copied().unwrap_or(0);
        let compound_writes = self.compound_writes.get(name).copied().unwrap_or(0);
        let method_calls = self.method_calls.get(name).copied().unwrap_or(0);
        let reads = references > writes || compound_writes > 0;

        if method_calls > 0 {
            return EcsAccessKind::Unknown;
        }

        match (reads, writes > 0) {
            (true, true) => EcsAccessKind::ReadWrite,
            (true, false) => EcsAccessKind::Read,
            (false, true) => EcsAccessKind::Write,
            (false, false) => EcsAccessKind::Filter,
        }
    }
}

fn callback_facts(function: &ast::AnonymousFunction) -> CallbackFacts {
    let mut visitor = CallbackVisitor::default();
    function.body().block().visit(&mut visitor);
    visitor.facts.resource_constructors.sort();
    visitor.facts.resource_constructors.dedup();
    visitor.facts
}

#[derive(Default)]
struct CallbackVisitor {
    facts: CallbackFacts,
    nested_function_depth: usize,
}

impl Visitor for CallbackVisitor {
    fn visit_var(&mut self, var: &Var) {
        if self.nested_function_depth > 0 {
            return;
        }

        if let Some(root) = root_from_var(var) {
            *self.facts.references.entry(root).or_default() += 1;
        }
    }

    fn visit_assignment(&mut self, assignment: &ast::Assignment) {
        if self.nested_function_depth > 0 {
            return;
        }

        for variable in assignment.variables() {
            if let Some(root) = root_from_var(variable) {
                *self.facts.writes.entry(root).or_default() += 1;
            }
        }
    }

    fn visit_compound_assignment(&mut self, assignment: &ast::CompoundAssignment) {
        if self.nested_function_depth > 0 {
            return;
        }

        if let Some(root) = root_from_var(assignment.lhs()) {
            *self.facts.writes.entry(root.clone()).or_default() += 1;
            *self.facts.compound_writes.entry(root).or_default() += 1;
        }
    }

    fn visit_function_call(&mut self, call: &FunctionCall) {
        if self.nested_function_depth > 0 {
            return;
        }

        let Some(parts) = field_chain_before_first_call(call) else {
            return;
        };

        if call
            .suffixes()
            .any(|suffix| matches!(suffix, Suffix::Call(Call::MethodCall(_))))
        {
            if let Some(root) = parts.first() {
                *self.facts.method_calls.entry(root.clone()).or_default() += 1;
                *self.facts.references.entry(root.clone()).or_default() += 1;
            }
        }

        let name = parts.join(".");
        if is_expensive_resource_constructor(&name) {
            self.facts.resource_constructors.push(name);
        }
    }

    fn visit_table_constructor(&mut self, _table: &ast::TableConstructor) {
        if self.nested_function_depth > 0 {
            return;
        }

        self.facts.table_allocations += 1;
    }

    fn visit_anonymous_function(&mut self, _function: &ast::AnonymousFunction) {
        if self.nested_function_depth == 0 {
            self.facts.nested_functions += 1;
        }
        self.nested_function_depth += 1;
    }

    fn visit_anonymous_function_end(&mut self, _function: &ast::AnonymousFunction) {
        self.nested_function_depth = self.nested_function_depth.saturating_sub(1);
    }
}

fn is_expensive_resource_constructor(name: &str) -> bool {
    matches!(
        name,
        "love.graphics.newImage"
            | "love.graphics.newFont"
            | "love.audio.newSource"
            | "love.image.newImageData"
            | "love.sound.newSoundData"
    )
}

fn root_from_var(var: &Var) -> Option<String> {
    plain_field_chain_from_var(var).and_then(|parts| parts.into_iter().next())
}

fn component_fields(expression: Option<&Expression>) -> Vec<EcsComponentField> {
    let Some(Expression::TableConstructor(table)) = expression else {
        return Vec::new();
    };

    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NameKey { key, value, .. } = field else {
                return None;
            };
            let (type_label, span) = string_expression(Some(value))?;

            Some(EcsComponentField {
                name: token_text(key),
                type_label,
                span,
            })
        })
        .collect()
}

fn component_uses(expression: Option<&Expression>) -> Vec<EcsComponentUse> {
    let Some(Expression::TableConstructor(table)) = expression else {
        return Vec::new();
    };

    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NoKey(expression) = field else {
                return None;
            };
            let parts = plain_field_chain_from_expression(expression)?;

            Some(EcsComponentUse {
                reference: parts.join("."),
                component_id: None,
                access: EcsAccessKind::Filter,
                span: node_span(expression)?,
            })
        })
        .collect()
}

fn argument_expression(call: &FunctionCall, index: usize) -> Option<&Expression> {
    let call = call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;

    match call {
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            arguments.iter().nth(index)
        }
        Call::MethodCall(method) => match method.args() {
            FunctionArgs::Parentheses { arguments, .. } => arguments.iter().nth(index),
            _ => None,
        },
        _ => None,
    }
}

fn string_expression(expression: Option<&Expression>) -> Option<(String, SourceSpan)> {
    let Expression::String(token) = expression? else {
        return None;
    };

    Some((string_literal_value(token)?, node_span(token)?))
}

fn string_literal_value(token: &TokenReference) -> Option<String> {
    match token.token().token_type() {
        TokenType::StringLiteral { literal, .. } => Some(literal.to_string()),
        _ => None,
    }
}

fn graph_id(kind: &str, path: &Path, position: SourcePosition) -> String {
    format!(
        "{kind}:{}:{}:{}",
        path.to_string_lossy(),
        position.line,
        position.column
    )
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::parse_tua_source;

    use super::*;

    fn index(parsed: &ParsedSource) -> EcsFileIndex {
        let (environment, _) = crate::types::build_type_environment_with_builtin_aliases(
            parsed,
            super::super::builtin_aliases(),
        );
        let environment = environment.with_local_types(super::super::builtin_locals());

        ecs_file_index(parsed, &environment)
    }

    #[test]
    fn indexes_components_system_access_queries_and_allocations() {
        let parsed = parse_tua_source(
            Path::new("src/ecs.tua"),
            r#"local Position = ecs.component("Position", { x = "number", y = "number" })
local Velocity = ecs.component("Velocity", { x = "number", y = "number" })
local Sprite = ecs.component("Sprite", { image = "Image" })
local Health = ecs.component("Health", { value = "number" })
local world = ecs.world()
local movement = ecs.system(world, { Position, Velocity }, function(_, position, velocity, dt)
  position.x += velocity.x * dt
  local scratch = { x = position.x }
end)
local crowded = world:query({ Position, Velocity, Sprite, Health })
"#
            .to_owned(),
        )
        .expect("source should parse");
        let file = index(&parsed);
        let graph = ecs_graph([&file]);

        assert_eq!(graph.components.len(), 4);
        assert_eq!(graph.systems.len(), 1);
        assert_eq!(graph.queries.len(), 2);
        assert_eq!(
            graph.systems[0].components[0].access,
            EcsAccessKind::ReadWrite
        );
        assert_eq!(graph.systems[0].components[1].access, EcsAccessKind::Read);
        assert!(graph.systems[0]
            .components
            .iter()
            .all(|usage| usage.component_id.is_some()));
        assert!(graph
            .insights
            .iter()
            .any(|insight| insight.kind == EcsInsightKind::CallbackAllocation));
        assert!(graph
            .insights
            .iter()
            .any(|insight| insight.kind == EcsInsightKind::WideQueryAllocation));
    }

    #[test]
    fn leaves_cross_file_or_dynamic_component_references_unresolved() {
        let parsed = parse_tua_source(
            Path::new("src/systems.tua"),
            "local world = ecs.world()\nlocal visible = world:query({ Components.Position })\n"
                .to_owned(),
        )
        .expect("source should parse");
        let file = index(&parsed);
        let graph = ecs_graph([&file]);

        assert_eq!(
            graph.queries[0].components[0].reference,
            "Components.Position"
        );
        assert_eq!(graph.queries[0].components[0].component_id, None);
    }

    #[test]
    fn ignores_unrelated_query_methods() {
        let parsed = parse_tua_source(
            Path::new("src/search.tua"),
            "local search = { query = function(filters) return filters end }\nlocal result = search.query({ \"name\" })\n"
                .to_owned(),
        )
        .expect("source should parse");
        let file = index(&parsed);

        assert!(file.queries.is_empty());
    }
}
