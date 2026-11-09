use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::classes::{class_method_owner, ClassDeclaration};
use crate::module_graph::ModuleGraph;
use crate::vfs::normalize_path;
use crate::{Definition, DefinitionKind, ParsedSource, ProjectTypeDatabase};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ProjectProtocolIndex {
    methods: BTreeMap<ProtocolIdentity, BTreeMap<String, Definition>>,
    implementations: BTreeMap<ProtocolMethodIdentity, Vec<Definition>>,
    class_conformances: BTreeMap<String, BTreeSet<ProtocolIdentity>>,
}

type ProtocolIdentity = (PathBuf, String);
type ProtocolMethodIdentity = (PathBuf, String, String);

impl ProjectProtocolIndex {
    pub(super) fn build<'a>(
        sources: impl IntoIterator<Item = (PathBuf, &'a ParsedSource)>,
        module_graph: &ModuleGraph,
        type_database: &ProjectTypeDatabase,
    ) -> Self {
        let sources = sources
            .into_iter()
            .map(|(path, parsed)| (normalize_path(path), parsed))
            .collect::<Vec<_>>();
        let mut index = Self::default();

        for (path, parsed) in &sources {
            for protocol in &parsed.protocol_declarations {
                let identity = (path.clone(), protocol.name.clone());
                let methods = protocol
                    .methods
                    .iter()
                    .map(|method| {
                        (
                            method.name.clone(),
                            Definition {
                                path: path.clone(),
                                span: method.name_span,
                                kind: DefinitionKind::Function,
                            },
                        )
                    })
                    .collect();
                index.methods.insert(identity, methods);
            }
        }

        for (path, parsed) in &sources {
            let classes = &parsed.class_declarations;
            for class in classes {
                let protocols = implemented_protocol_names(class, classes);
                for protocol_name in protocols {
                    let Some(binding) =
                        type_database.resolve_type_name(module_graph, path, &protocol_name)
                    else {
                        continue;
                    };
                    let protocol_identity = (binding.module_path, binding.name);
                    let Some(requirements) = index.methods.get(&protocol_identity) else {
                        continue;
                    };
                    index
                        .class_conformances
                        .entry(crate::classes::nominal_class_identity(path, &class.name))
                        .or_default()
                        .insert(protocol_identity.clone());

                    for method_name in requirements.keys() {
                        let Some(owner) = class_method_owner(classes, &class.name, method_name)
                        else {
                            continue;
                        };
                        let Some(method) = classes
                            .iter()
                            .find(|candidate| candidate.name == owner)
                            .and_then(|owner| {
                                owner
                                    .methods
                                    .iter()
                                    .find(|candidate| candidate.name == *method_name)
                            })
                        else {
                            continue;
                        };
                        let implementation = Definition {
                            path: path.clone(),
                            span: method.name_span,
                            kind: DefinitionKind::Function,
                        };
                        index
                            .implementations
                            .entry((
                                protocol_identity.0.clone(),
                                protocol_identity.1.clone(),
                                method_name.clone(),
                            ))
                            .or_default()
                            .push(implementation);
                    }
                }
            }
        }

        for implementations in index.implementations.values_mut() {
            implementations.sort_by_key(definition_sort_key);
            implementations.dedup();
        }

        index
    }

    pub(super) fn contains_protocol(&self, path: &Path, name: &str) -> bool {
        self.methods
            .contains_key(&(normalize_path(path), name.to_owned()))
    }

    pub(super) fn class_conformances(
        &self,
        class_identity: &str,
    ) -> impl Iterator<Item = &(PathBuf, String)> {
        self.class_conformances
            .get(class_identity)
            .into_iter()
            .flat_map(BTreeSet::iter)
    }

    pub(super) fn all_class_conformances(&self) -> Vec<(String, PathBuf, String)> {
        self.class_conformances
            .iter()
            .flat_map(|(class, protocols)| {
                protocols
                    .iter()
                    .map(|(path, name)| (class.clone(), path.clone(), name.clone()))
            })
            .collect()
    }

    pub(super) fn method_definition(
        &self,
        identity: &ProtocolIdentity,
        method: &str,
    ) -> Option<&Definition> {
        self.methods.get(identity)?.get(method)
    }

    pub(super) fn method_definitions(
        &self,
        identity: &ProtocolIdentity,
    ) -> Option<&BTreeMap<String, Definition>> {
        self.methods.get(identity)
    }

    pub(super) fn method_definition_at_position(
        &self,
        path: &Path,
        position: crate::SourcePosition,
    ) -> Option<Definition> {
        let path = normalize_path(path);
        self.methods
            .values()
            .flat_map(BTreeMap::values)
            .find(|definition| {
                definition.path == path
                    && definition.span.start.line == position.line
                    && position.column >= definition.span.start.column
                    && position.column <= definition.span.end.column
            })
            .cloned()
    }

    pub(super) fn method_implementations(
        &self,
        identity: &ProtocolIdentity,
        method: &str,
    ) -> Option<&[Definition]> {
        self.methods.get(identity)?;
        Some(
            self.implementations
                .get(&(identity.0.clone(), identity.1.clone(), method.to_owned()))
                .map(Vec::as_slice)
                .unwrap_or_default(),
        )
    }

    pub(super) fn related_method_definitions(
        &self,
        target: &Definition,
    ) -> Option<Vec<Definition>> {
        let target_path = normalize_path(&target.path);
        let mut identities = BTreeSet::<ProtocolMethodIdentity>::new();

        for (identity, methods) in &self.methods {
            for (method, definition) in methods {
                if definition.path == target_path && definition.span == target.span {
                    identities.insert((identity.0.clone(), identity.1.clone(), method.clone()));
                }
            }
        }

        for (identity, implementations) in &self.implementations {
            if implementations
                .iter()
                .any(|definition| definition.path == target_path && definition.span == target.span)
            {
                identities.insert(identity.clone());
            }
        }

        if identities.is_empty() {
            return None;
        }

        let mut pending = identities.clone();
        while let Some(identity) = pending.iter().next().cloned() {
            pending.remove(&identity);
            let Some(implementations) = self.implementations.get(&identity) else {
                continue;
            };
            for implementation in implementations {
                for (other_identity, other_implementations) in &self.implementations {
                    if other_implementations.contains(implementation)
                        && identities.insert(other_identity.clone())
                    {
                        pending.insert(other_identity.clone());
                    }
                }
            }
        }

        let mut definitions = Vec::new();
        for (path, protocol, method) in identities {
            let identity = (path.clone(), protocol.clone());
            if let Some(definition) = self.method_definition(&identity, &method) {
                definitions.push(definition.clone());
            }
            if let Some(implementations) = self.method_implementations(&identity, &method) {
                definitions.extend(implementations.iter().cloned());
            }
        }
        definitions.sort_by_key(definition_sort_key);
        definitions.dedup();
        Some(definitions)
    }
}

fn implemented_protocol_names(
    class: &ClassDeclaration,
    classes: &[ClassDeclaration],
) -> BTreeSet<String> {
    fn visit(
        class: &ClassDeclaration,
        classes: &[ClassDeclaration],
        visited: &mut BTreeSet<String>,
        protocols: &mut BTreeSet<String>,
    ) {
        if !visited.insert(class.name.clone()) {
            return;
        }
        if let Some(parent) = class
            .parent
            .as_deref()
            .and_then(|parent| classes.iter().find(|candidate| candidate.name == parent))
        {
            visit(parent, classes, visited, protocols);
        }
        protocols.extend(
            class
                .implemented_protocols
                .iter()
                .map(|(name, _)| name.clone()),
        );
    }

    let mut protocols = BTreeSet::new();
    visit(class, classes, &mut BTreeSet::new(), &mut protocols);
    protocols
}

fn definition_sort_key(definition: &Definition) -> (PathBuf, usize, usize, usize, usize, u8) {
    let kind = match definition.kind {
        DefinitionKind::Function => 0,
        DefinitionKind::TypeAlias => 1,
        DefinitionKind::TypeParameter => 2,
        DefinitionKind::TableField => 2,
        DefinitionKind::Local => 3,
        DefinitionKind::Parameter => 4,
        DefinitionKind::Module => 5,
        DefinitionKind::Builtin => 6,
        DefinitionKind::Asset => 7,
        DefinitionKind::SignalEvent => 8,
    };
    (
        definition.path.clone(),
        definition.span.start.line,
        definition.span.start.column,
        definition.span.end.line,
        definition.span.end.column,
        kind,
    )
}
