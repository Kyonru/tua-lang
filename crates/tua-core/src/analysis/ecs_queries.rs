use super::*;

impl AnalysisSnapshot {
    pub fn ecs_graph(&self) -> crate::EcsGraph {
        let _profile = profile::span("analysis.query.ecs_graph");
        let mut graph =
            crate::ecs::ecs_graph(self.files.values().map(|file| file.ecs_index.as_ref()));
        let component_ids = graph
            .components
            .iter()
            .map(|component| {
                (
                    component_definition_key(&component.path, component.name_span),
                    component.id.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();

        for system in &mut graph.systems {
            self.resolve_project_ecs_components(
                &component_ids,
                &system.path,
                &mut system.components,
            );
        }
        for query in &mut graph.queries {
            self.resolve_project_ecs_components(&component_ids, &query.path, &mut query.components);
        }

        graph
    }

    fn resolve_project_ecs_components(
        &self,
        component_ids: &BTreeMap<(PathBuf, usize, usize, usize, usize), String>,
        path: &Path,
        uses: &mut [crate::EcsComponentUse],
    ) {
        for component_use in uses {
            if component_use.component_id.is_some() {
                continue;
            }

            let position = SourcePosition {
                line: component_use.span.end.line,
                column: component_use.span.end.column.saturating_sub(1).max(1),
            };
            let Some(definition) = self.definition(path, position) else {
                continue;
            };

            component_use.component_id = component_ids
                .get(&component_definition_key(&definition.path, definition.span))
                .cloned();
        }
    }
}

fn component_definition_key(
    path: &Path,
    span: SourceSpan,
) -> (PathBuf, usize, usize, usize, usize) {
    (
        normalize_path(path),
        span.start.line,
        span.start.column,
        span.end.line,
        span.end.column,
    )
}
