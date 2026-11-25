use super::*;

impl AnalysisSnapshot {
    /// Authored dialogue structure only: no imports, handlers, assets, conditions,
    /// localization, or placeholders are evaluated. Missing or malformed source
    /// returns `None`; callers should display current diagnostics, not stale data.
    pub fn dialogue_preview(
        &self,
        path: impl AsRef<Path>,
    ) -> Option<crate::dialogue_asset::preview::DialoguePreview> {
        let file = self.file(path.as_ref())?;
        let document = file.dialogue_model.get()?.as_ref()?;
        Some(crate::dialogue_asset::preview::DialoguePreview::new(
            document.clone(),
        ))
    }

    pub fn inspect(
        &self,
        path: impl AsRef<Path>,
        position: Option<SourcePosition>,
    ) -> Vec<InspectorItem> {
        let _profile = profile::span("analysis.query.inspect");
        let path = path.as_ref();
        let Some(file) = self.file(path) else {
            return Vec::new();
        };
        let Ok(parsed) = &file.parsed else {
            return Vec::new();
        };
        let mut items = crate::sprite::inspections(
            path,
            &parsed.sprite_declarations,
            &file.assets,
            self.effective_asset_roots(),
            self.asset_index.as_deref(),
        );
        items.extend(
            crate::state_machine::inspections(parsed)
                .into_iter()
                .map(InspectorItem::StateMachine),
        );
        items.extend(
            crate::behavior_tree::inspections(parsed)
                .into_iter()
                .map(InspectorItem::BehaviorTree),
        );

        let Some(position) = position else {
            return items;
        };
        if items.is_empty() {
            return items;
        }
        if items
            .iter()
            .any(|item| contains_position(item.span(), position))
        {
            return items
                .into_iter()
                .filter(|item| contains_position(item.span(), position))
                .collect();
        }
        let definition = self.definition(path, position);

        items
            .into_iter()
            .filter(|item| {
                definition.as_ref().is_some_and(|definition| {
                    definition.path == path && definition.span == item.selection_span()
                })
            })
            .collect()
    }
}

fn contains_position(span: SourceSpan, position: SourcePosition) -> bool {
    let start = (span.start.line, span.start.column);
    let end = (span.end.line, span.end.column);
    let position = (position.line, position.column);

    start <= position && position < end
}
