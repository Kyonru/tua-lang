//! Resolve imported class identities from the indexed project, including
//! recovered editor buffers. File-local export inference cannot see overlays.

use super::*;
use crate::parser::TuaType;
use crate::types::{build_type_environment_with_diagnostics, TypeEnvironment};
use crate::Type;
use full_moon::visitors::Visitor;

#[derive(Clone)]
struct FunctionReturnForward {
    source: PathBuf,
    public_name: String,
    target_module: String,
    target_field: String,
}

#[derive(Default)]
struct ModuleAliasMutations(BTreeSet<String>);

impl Visitor for ModuleAliasMutations {
    fn visit_assignment(&mut self, assignment: &ast::Assignment) {
        for variable in assignment.variables().iter() {
            if let Some(root) = crate::ast_utils::plain_field_chain_from_var(variable)
                .and_then(|parts| parts.into_iter().next())
            {
                self.0.insert(root);
            }
        }
    }
}

impl ModuleGraph {
    pub(super) fn enrich_imported_class_returns(
        &mut self,
        inputs: &[ModuleGraphInput<'_>],
    ) -> bool {
        if !inputs.iter().any(|input| {
            input.parsed.is_some_and(|parsed| {
                !parsed.type_imports.is_empty() && !parsed.function_annotations.is_empty()
            })
        }) {
            return false;
        }
        let parsed_by_path = inputs
            .iter()
            .filter_map(|input| Some((normalize_path(input.path), input.parsed?)))
            .collect::<BTreeMap<_, _>>();
        let mut environments = BTreeMap::<PathBuf, TypeEnvironment>::new();
        let mut changed_any = false;

        for input in inputs {
            let Some(parsed) = input.parsed else {
                continue;
            };
            if parsed.type_imports.is_empty() || parsed.function_annotations.is_empty() {
                continue;
            }
            let path = normalize_path(input.path);
            let Some(module) = self.module(&path) else {
                continue;
            };
            let exports = module.exports.clone();

            for mut export in exports {
                let Type::Function(function) = &mut export.field.ty else {
                    continue;
                };
                let Some(annotation) = parsed.function_annotations.iter().find(|annotation| {
                    annotation.name_span == Some(export.definition_span)
                        && annotation.return_type.is_some()
                }) else {
                    continue;
                };
                let authored = annotation.return_type.as_ref().expect("checked above");
                let authored_returns = match authored {
                    TuaType::Tuple(items) => items.iter().collect::<Vec<_>>(),
                    other => vec![other],
                };
                let mut changed = false;
                let mut resolved = Vec::with_capacity(authored_returns.len());
                for (index, ty) in authored_returns.iter().enumerate() {
                    if let Some(nominal) = self.resolve_authored_imported_class(
                        parsed,
                        ty,
                        &parsed_by_path,
                        &mut environments,
                    ) {
                        resolved.push(nominal);
                        changed = true;
                    } else {
                        resolved.push(
                            function
                                .return_types_or_any()
                                .get(index)
                                .cloned()
                                .unwrap_or_else(Type::nil),
                        );
                    }
                }
                if !changed || resolved.is_empty() {
                    continue;
                }
                function.return_type = Box::new(resolved[0].clone());
                function.return_types = resolved;
                changed_any |= self.upsert_provenance_export(export);
            }
        }
        changed_any
    }

    fn resolve_authored_imported_class(
        &self,
        parsed: &ParsedSource,
        authored: &TuaType,
        parsed_by_path: &BTreeMap<PathBuf, &ParsedSource>,
        environments: &mut BTreeMap<PathBuf, TypeEnvironment>,
    ) -> Option<Type> {
        match authored {
            TuaType::Named(name) => self.resolve_imported_class_alias(
                parsed,
                name,
                parsed_by_path,
                environments,
                &mut BTreeSet::new(),
            ),
            TuaType::Union(items) => {
                let mut changed = false;
                let resolved = items
                    .iter()
                    .map(|item| {
                        if let Some(ty) = self.resolve_authored_imported_class(
                            parsed,
                            item,
                            parsed_by_path,
                            environments,
                        ) {
                            changed = true;
                            ty
                        } else {
                            crate::types::type_from_tua_type(item)
                        }
                    })
                    .collect::<Vec<_>>();
                changed.then(|| Type::union(resolved))
            }
            TuaType::Array(item) => self
                .resolve_authored_imported_class(parsed, item, parsed_by_path, environments)
                .map(Type::array),
            _ => None,
        }
    }

    fn resolve_imported_class_alias(
        &self,
        parsed: &ParsedSource,
        name: &str,
        parsed_by_path: &BTreeMap<PathBuf, &ParsedSource>,
        environments: &mut BTreeMap<PathBuf, TypeEnvironment>,
        visited: &mut BTreeSet<(PathBuf, String)>,
    ) -> Option<Type> {
        let key = (normalize_path(&parsed.path), name.to_owned());
        if visited.len() >= 32 || !visited.insert(key.clone()) {
            return None;
        }
        let result = (|| {
            let (import, binding) = parsed
                .type_imports
                .iter()
                .flat_map(|import| import.bindings.iter().map(move |binding| (import, binding)))
                .find(|(_, binding)| binding.local == name)?;
            let target_path = self.resolved_require_path(&parsed.path, &import.module)?;
            let target = parsed_by_path.get(&target_path)?;
            let imported = binding.imported.as_str();

            if target
                .type_aliases
                .iter()
                .any(|alias| alias.name == imported && alias.exported)
            {
                if !environments.contains_key(&target_path) {
                    let (environment, _) = build_type_environment_with_diagnostics(target);
                    environments.insert(target_path.clone(), environment);
                }
                let environment = environments.get(&target_path)?;
                let Type::Table(table) =
                    environment.resolve_aliases(&Type::Alias(imported.to_owned()))
                else {
                    return None;
                };
                return table
                    .nominal_identity()
                    .is_some()
                    .then_some(Type::Table(table));
            }
            target
                .type_imports
                .iter()
                .any(|candidate| {
                    candidate.reexport
                        && candidate
                            .bindings
                            .iter()
                            .any(|candidate| candidate.local == imported)
                })
                .then(|| {
                    self.resolve_imported_class_alias(
                        target,
                        imported,
                        parsed_by_path,
                        environments,
                        visited,
                    )
                })
                .flatten()
        })();
        visited.remove(&key);
        result
    }

    pub(super) fn enrich_forwarded_function_returns(&mut self, inputs: &[ModuleGraphInput<'_>]) {
        let forwards = inputs
            .iter()
            .filter_map(|input| {
                let parsed = input.parsed?;
                let module = self.module(input.path)?;
                let owned_hir;
                let hir = if let Some(hir) = input.hir {
                    hir
                } else {
                    owned_hir = lower_parsed_source(parsed);
                    &owned_hir
                };
                Some(function_return_forwards(parsed, hir, module))
            })
            .flatten()
            .collect::<Vec<_>>();

        // A wrapper can itself be wrapped. Bound the passes so a cyclic module
        // graph cannot turn an editor query into unbounded work.
        for _ in 0..forwards.len().min(64) {
            let mut changed = false;
            for forward in &forwards {
                let Some(target_path) =
                    self.resolved_require_path(&forward.source, &forward.target_module)
                else {
                    continue;
                };
                let Some(Type::Function(target)) = self
                    .export(&target_path, &forward.target_field)
                    .map(|export| &export.field.ty)
                else {
                    continue;
                };
                if !target.generic_parameters.is_empty()
                    || !target
                        .return_types_or_any()
                        .iter()
                        .any(type_has_nominal_class)
                {
                    continue;
                }
                let returns = target.return_types_or_any().to_vec();
                let Some(mut export) = self.export(&forward.source, &forward.public_name).cloned()
                else {
                    continue;
                };
                let Type::Function(function) = &mut export.field.ty else {
                    continue;
                };
                if function.return_types_or_any() == returns.as_slice() {
                    continue;
                }
                function.return_type = Box::new(returns[0].clone());
                function.return_types = returns;
                changed |= self.upsert_provenance_export(export);
            }
            if !changed {
                break;
            }
        }
    }

    fn upsert_provenance_export(&mut self, entry: ModuleExportEntry) -> bool {
        let changed = self.upsert_forwarded_export(entry.clone());
        if changed {
            if let Some(symbol_id) = entry.symbol_id {
                if let Some(entries) = self
                    .exports_by_symbol
                    .get_mut(&(entry.module_path.clone(), symbol_id))
                {
                    if let Some(existing) = entries
                        .iter_mut()
                        .find(|existing| existing.public_name == entry.public_name)
                    {
                        *existing = entry;
                    }
                }
            }
        }
        changed
    }
}

fn type_has_nominal_class(ty: &Type) -> bool {
    match ty {
        Type::Table(table) => table.nominal_identity().is_some(),
        Type::Union(items) => items.iter().any(type_has_nominal_class),
        Type::Array(item) => type_has_nominal_class(item),
        _ => false,
    }
}

fn function_return_forwards(
    parsed: &ParsedSource,
    hir: &HirModule,
    module: &ModuleGraphModule,
) -> Vec<FunctionReturnForward> {
    let scope = crate::scope_map_parsed_source(parsed);
    let mut reassigned_modules = ModuleAliasMutations::default();
    reassigned_modules.visit_ast(&parsed.ast);
    let bindings = hir
        .require_bindings
        .iter()
        .filter(|binding| binding.kind == ModuleSpecifierKind::Require)
        .map(|binding| {
            (
                binding.alias.clone(),
                (binding.module.as_str(), binding.alias_span),
            )
        })
        .collect::<BTreeMap<_, _>>();

    parsed
        .ast
        .nodes()
        .stmts()
        .filter_map(|stmt| match stmt {
            ast::Stmt::LocalFunction(function) => Some(function),
            _ => None,
        })
        .filter_map(|function| {
            if function.body().return_type().is_some()
                || function.body().block().stmts().next().is_some()
            {
                return None;
            }
            let Some(LastStmt::Return(returns)) = function.body().block().last_stmt() else {
                return None;
            };
            let mut returned = returns.returns().iter();
            let Expression::FunctionCall(call) = returned.next()? else {
                return None;
            };
            if returned.next().is_some() {
                return None;
            }
            let Prefix::Name(receiver) = call.prefix() else {
                return None;
            };
            let receiver_name = token_text(receiver);
            if reassigned_modules.0.contains(&receiver_name) {
                return None;
            }
            let (target_module, binding_span) = bindings.get(&receiver_name)?;
            let binding_symbol = scope.declaration_at_span(*binding_span)?.id;
            if scope.symbol_at_position(node_span(receiver)?.start) != Some(binding_symbol) {
                return None;
            }
            let mut suffixes = call.suffixes();
            let Suffix::Index(ast::Index::Dot { name, .. }) = suffixes.next()? else {
                return None;
            };
            if !matches!(suffixes.next()?, Suffix::Call(_)) || suffixes.next().is_some() {
                return None;
            }
            let definition_span = node_span(function.name())?;
            let export = module
                .exports
                .iter()
                .find(|export| export.definition_span == definition_span)?;
            Some(FunctionReturnForward {
                source: module.path.clone(),
                public_name: export.public_name.clone(),
                target_module: (*target_module).to_owned(),
                target_field: token_text(name),
            })
        })
        .collect()
}
