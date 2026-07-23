use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use full_moon::ast::{self, Call, Expression, FunctionArgs, FunctionCall, Suffix};
use full_moon::node::Node;
use full_moon::visitors::Visitor;

use crate::ast_utils::{
    assignment_expression_for_local, field_chain_before_first_call,
    plain_field_chain_from_expression, token_text,
};
use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticSeverity};
use crate::parser::{ParsedSource, SourcePosition, SourceSpan};
use crate::semantic::SemanticModel;
use crate::types::{FunctionType, TableField, Type, TypeEnvironment};
use crate::{TypedFunction, TypedParameter};

pub(crate) const TYPE_NAME: &str = "ObjectPool";
pub(crate) const OPTIONS_TYPE: &str = "ObjectPoolOptions";
pub(crate) const CONSTRUCTOR_NAME: &str = "objectPool";
pub(crate) const RUNTIME_MODULE: &str = "tuaobjectpool";
pub(crate) const EMITTED_CONSTRUCTOR_LOCAL: &str = "__tua_object_pool";

const METHOD_NAMES: &[&str] = &[
    "acquire",
    "release",
    "prewarm",
    "clear",
    "owns",
    "activeCount",
    "availableCount",
    "capacity",
];

const BUILTIN_TYPE_NAMES: &[&str] = &[TYPE_NAME, OPTIONS_TYPE];

pub(crate) fn builtin_type_names() -> &'static [&'static str] {
    BUILTIN_TYPE_NAMES
}

pub(crate) fn builtin_aliases() -> Vec<(String, Type)> {
    vec![
        (TYPE_NAME.to_owned(), pool_type(Type::Any)),
        (OPTIONS_TYPE.to_owned(), options_type()),
    ]
}

pub(crate) fn options_type() -> Type {
    Type::table([
        (
            "create",
            TableField::required(Type::Function(FunctionType::new([], Type::Any))),
        ),
        (
            "reset",
            TableField::optional(Type::Function(FunctionType::new([Type::Any], Type::nil()))),
        ),
        (
            "dispose",
            TableField::optional(Type::Function(FunctionType::new([Type::Any], Type::nil()))),
        ),
        ("initial", TableField::optional(Type::number())),
    ])
}

pub(crate) fn pool_type(item_type: Type) -> Type {
    Type::table(METHOD_NAMES.iter().map(|method| {
        (
            *method,
            TableField::required(
                typed_method_for_item(method, item_type.clone())
                    .expect("known object pool method")
                    .as_type()
                    .expect("object pool method has a function type"),
            ),
        )
    }))
}

pub(crate) fn pool_type_from_options(options: &Type) -> Type {
    let item_type = match options {
        Type::Table(table) => table
            .field("create")
            .and_then(|field| match &field.ty {
                Type::Function(function) => function.return_types_or_any().first().cloned(),
                _ => None,
            })
            .unwrap_or(Type::Any),
        _ => Type::Any,
    };

    pool_type(item_type)
}

pub(crate) fn pool_item_type(environment: &TypeEnvironment, candidate: &Type) -> Option<Type> {
    match candidate {
        Type::Alias(name) if name == TYPE_NAME => Some(Type::Any),
        Type::Alias(name) => environment
            .alias(name)
            .filter(|resolved| *resolved != candidate)
            .and_then(|resolved| pool_item_type(environment, resolved)),
        Type::Union(types) => {
            let items = types
                .iter()
                .filter_map(|ty| pool_item_type(environment, ty))
                .collect::<Vec<_>>();

            match items.len() {
                0 => None,
                1 => items.into_iter().next(),
                _ => Some(Type::union(items)),
            }
        }
        Type::Table(table)
            if METHOD_NAMES
                .iter()
                .all(|method| table.field(method).is_some()) =>
        {
            table.field("acquire").and_then(|field| match &field.ty {
                Type::Function(function) => function.return_types_or_any().first().cloned(),
                _ => None,
            })
        }
        _ => None,
    }
}

pub(crate) fn is_constructor(name: &str) -> bool {
    name == CONSTRUCTOR_NAME
}

pub(crate) fn constructor_type() -> FunctionType {
    FunctionType::new([Type::alias(OPTIONS_TYPE)], Type::alias(TYPE_NAME))
}

pub(crate) fn constructor_typed_function() -> TypedFunction {
    typed_function(
        CONSTRUCTOR_NAME,
        [parameter("options", Type::alias(OPTIONS_TYPE))],
        Type::alias(TYPE_NAME),
        documentation(),
    )
}

pub(crate) fn typed_member(
    environment: &TypeEnvironment,
    receiver_type: &Type,
    method: &str,
) -> Option<TypedFunction> {
    let item_type = pool_item_type(environment, receiver_type)?;

    typed_method_for_item(method, item_type)
}

fn typed_method_for_item(method: &str, item_type: Type) -> Option<TypedFunction> {
    let self_parameter = parameter("self", Type::alias(TYPE_NAME));

    let mut function = match method {
        "acquire" => Some(typed_function(
            method,
            [self_parameter],
            item_type,
            "Take an available object or create one when the pool is empty.",
        )),
        "release" => Some(typed_function(
            method,
            [self_parameter, parameter("object", item_type)],
            Type::boolean(),
            "Reset and return an active object. Returns false for foreign or duplicate releases.",
        )),
        "prewarm" => Some(typed_function(
            method,
            [self_parameter, parameter("count", Type::number())],
            Type::number(),
            "Ensure at least count objects are available and return the number created.",
        )),
        "clear" => Some(typed_function(
            method,
            [self_parameter],
            Type::nil(),
            "Dispose every owned object and reset the pool to empty.",
        )),
        "owns" => Some(typed_function(
            method,
            [self_parameter, parameter("object", item_type)],
            Type::boolean(),
            "Return whether this pool created the object.",
        )),
        "activeCount" => Some(typed_function(
            method,
            [self_parameter],
            Type::number(),
            "Return the number of objects currently checked out.",
        )),
        "availableCount" => Some(typed_function(
            method,
            [self_parameter],
            Type::number(),
            "Return the number of objects ready for immediate reuse.",
        )),
        "capacity" => Some(typed_function(
            method,
            [self_parameter],
            Type::number(),
            "Return the total number of objects owned by the pool.",
        )),
        _ => None,
    }?;

    function.name = Some(format!("{TYPE_NAME}:{method}"));
    Some(function)
}

pub(crate) const fn detail() -> &'static str {
    "function objectPool(options: ObjectPoolOptions): ObjectPool"
}

pub(crate) const fn documentation() -> &'static str {
    "Create a reusable object pool for allocation-heavy gameplay objects such as bullets, particles, and temporary effects."
}

pub(crate) const fn workflow_snippet() -> &'static str {
    "type ${1:Bullet} = { x: number, y: number, active: boolean }\n\nlocal ${2:bulletPool} = objectPool({\n  create = function(): ${1:Bullet}\n    return { x = ${3:0}, y = ${4:0}, active = false }\n  end,\n  reset = function(object: ${1:Bullet})\n    object.active = false\n  end,\n  initial = ${5:64},\n})\n\nlocal ${6:bullet} = ${2:bulletPool}:acquire()\n${0}"
}

pub(crate) fn runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/tuaobjectpool.lua")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectPoolOperationKind {
    Acquire,
    Release,
    Clear,
    Prewarm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectPoolInsightKind {
    UnreleasedAcquire,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObjectPoolGraph {
    pub pools: Vec<ObjectPoolGraphPool>,
    pub operations: Vec<ObjectPoolGraphOperation>,
    pub insights: Vec<ObjectPoolGraphInsight>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectPoolGraphPool {
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub item_type_label: String,
    pub span: SourceSpan,
    pub name_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectPoolGraphOperation {
    pub id: String,
    pub path: PathBuf,
    pub pool_id: Option<String>,
    pub pool_name: String,
    pub kind: ObjectPoolOperationKind,
    pub item_name: Option<String>,
    pub span: SourceSpan,
    pub item_span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectPoolGraphInsight {
    pub id: String,
    pub kind: ObjectPoolInsightKind,
    pub title: String,
    pub message: String,
    pub path: PathBuf,
    pub span: SourceSpan,
    pub pool_id: String,
    pub acquire_operation_id: String,
    pub item_name: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ObjectPoolFileIndex {
    pub pools: Vec<ObjectPoolGraphPool>,
    pub operations: Vec<ObjectPoolGraphOperation>,
}

#[derive(Clone, Debug)]
struct LocalCallBinding {
    name: String,
    name_span: SourceSpan,
}

pub(crate) fn object_pool_file_index(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
) -> ObjectPoolFileIndex {
    let mut bindings = LocalCallBindingVisitor::default();
    bindings.visit_ast(&parsed.ast);

    let mut visitor = ObjectPoolVisitor {
        path: &parsed.path,
        environment,
        semantic: SemanticModel::new(parsed, environment),
        bindings: bindings.bindings,
        index: ObjectPoolFileIndex::default(),
    };

    visitor.visit_ast(&parsed.ast);
    visitor.index
}

pub(crate) fn object_pool_graph<'a>(
    indexes: impl IntoIterator<Item = &'a ObjectPoolFileIndex>,
) -> ObjectPoolGraph {
    let mut graph = ObjectPoolGraph::default();

    for index in indexes {
        graph.pools.extend(index.pools.iter().cloned());
        graph.operations.extend(index.operations.iter().cloned());
    }

    let pool_ids = graph
        .pools
        .iter()
        .map(|pool| ((pool.path.clone(), pool.name.clone()), pool.id.clone()))
        .collect::<BTreeMap<_, _>>();

    for operation in &mut graph.operations {
        operation.pool_id = pool_ids
            .get(&(operation.path.clone(), operation.pool_name.clone()))
            .cloned();
    }

    graph.insights = unreleased_acquire_insights(&graph.operations);

    graph.pools.sort_by(|left, right| {
        left.name.cmp(&right.name).then_with(|| {
            item_order(
                &left.path,
                left.name_span,
                &left.id,
                &right.path,
                right.name_span,
                &right.id,
            )
        })
    });
    graph.operations.sort_by(|left, right| {
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

pub(crate) fn diagnostics_for_graph(graph: &ObjectPoolGraph, path: &Path) -> Vec<Diagnostic> {
    graph
        .insights
        .iter()
        .filter(|insight| insight.path == path)
        .map(|insight| {
            Diagnostic::new(
                insight.path.clone(),
                insight.span.start.line,
                insight.span.start.column,
                DiagnosticSeverity::Warning,
                insight.message.clone(),
            )
            .with_range(
                insight.span.start.line,
                insight.span.start.column,
                insight.span.end.line,
                insight.span.end.column,
            )
            .with_code(DiagnosticCode::DOMAIN_OBJECT_POOL_LIFECYCLE)
        })
        .collect()
}

fn unreleased_acquire_insights(
    operations: &[ObjectPoolGraphOperation],
) -> Vec<ObjectPoolGraphInsight> {
    operations
        .iter()
        .filter(|operation| operation.kind == ObjectPoolOperationKind::Acquire)
        .filter_map(|acquire| {
            let pool_id = acquire.pool_id.as_ref()?;
            let item_name = acquire.item_name.as_ref()?;
            let released = operations.iter().any(|candidate| {
                candidate.pool_id.as_ref() == Some(pool_id)
                    && candidate.path == acquire.path
                    && position_key(candidate.span.start) > position_key(acquire.span.start)
                    && (candidate.kind == ObjectPoolOperationKind::Clear
                        || (candidate.kind == ObjectPoolOperationKind::Release
                            && candidate.item_name.as_ref() == Some(item_name)))
            });

            if released {
                return None;
            }

            Some(ObjectPoolGraphInsight {
                id: format!("{}:unreleased", acquire.id),
                kind: ObjectPoolInsightKind::UnreleasedAcquire,
                title: "Acquired object has no visible release".to_owned(),
                message: format!(
                    "Object `{item_name}` is acquired from `{}` but no later same-file `{}:release({item_name})` or `{}:clear()` is visible.",
                    acquire.pool_name, acquire.pool_name, acquire.pool_name
                ),
                path: acquire.path.clone(),
                span: acquire.item_span.unwrap_or(acquire.span),
                pool_id: pool_id.clone(),
                acquire_operation_id: acquire.id.clone(),
                item_name: item_name.clone(),
            })
        })
        .collect()
}

#[derive(Default)]
struct LocalCallBindingVisitor {
    bindings: BTreeMap<(usize, usize), LocalCallBinding>,
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
                LocalCallBinding {
                    name: token_text(name),
                    name_span,
                },
            );
        }
    }
}

struct ObjectPoolVisitor<'a> {
    path: &'a Path,
    environment: &'a TypeEnvironment,
    semantic: SemanticModel<'a>,
    bindings: BTreeMap<(usize, usize), LocalCallBinding>,
    index: ObjectPoolFileIndex,
}

impl Visitor for ObjectPoolVisitor<'_> {
    fn visit_function_call(&mut self, call: &FunctionCall) {
        let Some(parts) = field_chain_before_first_call(call) else {
            return;
        };
        let Some(span) = node_span(call) else {
            return;
        };
        let binding = self.bindings.get(&position_key(span.start)).cloned();

        if parts.len() == 1 && is_constructor(&parts[0]) {
            self.record_pool(span, binding);
            return;
        }

        let Some(method) = parts.last() else {
            return;
        };
        let Some(kind) = operation_kind(method) else {
            return;
        };
        if !self.receiver_has_object_pool_member(&parts, span.start) {
            return;
        }

        self.record_operation(call, span, &parts[..parts.len() - 1], kind, binding);
    }
}

impl ObjectPoolVisitor<'_> {
    fn receiver_has_object_pool_member(&self, parts: &[String], position: SourcePosition) -> bool {
        let Some((method, receiver)) = parts.split_last() else {
            return false;
        };
        let Some(receiver_type) = self.semantic.type_of_field_chain(receiver, position) else {
            return false;
        };

        typed_member(self.environment, &receiver_type, method).is_some()
    }

    fn record_pool(&mut self, span: SourceSpan, binding: Option<LocalCallBinding>) {
        let Some(binding) = binding else {
            return;
        };
        let Some(item_type) = self
            .semantic
            .type_of_field_chain(std::slice::from_ref(&binding.name), span.start)
            .and_then(|ty| pool_item_type(self.environment, &ty))
        else {
            return;
        };

        self.index.pools.push(ObjectPoolGraphPool {
            id: graph_id("pool", self.path, binding.name_span.start),
            path: self.path.to_path_buf(),
            name: binding.name,
            item_type_label: item_type.to_string(),
            span,
            name_span: binding.name_span,
        });
    }

    fn record_operation(
        &mut self,
        call: &FunctionCall,
        span: SourceSpan,
        receiver_parts: &[String],
        kind: ObjectPoolOperationKind,
        binding: Option<LocalCallBinding>,
    ) {
        let (item_name, item_span) = match kind {
            ObjectPoolOperationKind::Acquire => binding
                .map(|binding| (Some(binding.name), Some(binding.name_span)))
                .unwrap_or((None, None)),
            ObjectPoolOperationKind::Release => argument_expression(call, 0)
                .and_then(|expression| {
                    let span = node_span(expression)?;
                    let item_name = plain_field_chain_from_expression(expression)?.join(".");
                    Some((Some(item_name), Some(span)))
                })
                .unwrap_or((None, None)),
            ObjectPoolOperationKind::Clear | ObjectPoolOperationKind::Prewarm => (None, None),
        };

        self.index.operations.push(ObjectPoolGraphOperation {
            id: graph_id(operation_kind_name(kind), self.path, span.start),
            path: self.path.to_path_buf(),
            pool_id: None,
            pool_name: receiver_parts.join("."),
            kind,
            item_name,
            span,
            item_span,
        });
    }
}

fn operation_kind(method: &str) -> Option<ObjectPoolOperationKind> {
    match method {
        "acquire" => Some(ObjectPoolOperationKind::Acquire),
        "release" => Some(ObjectPoolOperationKind::Release),
        "clear" => Some(ObjectPoolOperationKind::Clear),
        "prewarm" => Some(ObjectPoolOperationKind::Prewarm),
        _ => None,
    }
}

fn operation_kind_name(kind: ObjectPoolOperationKind) -> &'static str {
    match kind {
        ObjectPoolOperationKind::Acquire => "acquire",
        ObjectPoolOperationKind::Release => "release",
        ObjectPoolOperationKind::Clear => "clear",
        ObjectPoolOperationKind::Prewarm => "prewarm",
    }
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

fn graph_id(kind: &str, path: &Path, position: SourcePosition) -> String {
    format!(
        "{}:{}:{}:{}",
        kind,
        path.to_string_lossy(),
        position.line,
        position.column
    )
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn node_span<T: Node>(node: &T) -> Option<SourceSpan> {
    Some(SourceSpan {
        start: SourcePosition::from_full_moon(node.start_position()?),
        end: SourcePosition::from_full_moon(node.end_position()?),
    })
}

fn typed_function(
    name: &str,
    parameters: impl IntoIterator<Item = TypedParameter>,
    return_type: Type,
    documentation: &str,
) -> TypedFunction {
    let parameters = parameters.into_iter().collect::<Vec<_>>();

    TypedFunction {
        name: Some(name.to_owned()),
        parameter_count: parameters.len(),
        required_parameter_count: parameters.len(),
        parameters,
        has_varargs: false,
        vararg_type: None,
        return_type: Some(return_type.clone()),
        return_types: vec![return_type],
        documentation: Some(documentation.to_owned()),
        overloads: Vec::new(),
    }
}

fn parameter(name: &str, ty: Type) -> TypedParameter {
    TypedParameter {
        name: name.to_owned(),
        ty,
        documentation: None,
    }
}
