use super::*;

impl AnalysisSnapshot {
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
        let definition = position.and_then(|position| self.definition(path, position));

        crate::sprite::inspections(
            path,
            &parsed.sprite_declarations,
            &file.assets,
            self.effective_asset_roots(),
            self.asset_index.as_ref(),
        )
        .into_iter()
        .filter(|item| {
            let InspectorItem::SpriteSheet(sprite) = item;

            position.is_none_or(|position| {
                contains_position(sprite.span, position)
                    || definition.as_ref().is_some_and(|definition| {
                        definition.path == path && definition.span == sprite.name_span
                    })
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
