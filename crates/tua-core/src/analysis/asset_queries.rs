use super::*;

impl AnalysisSnapshot {
    pub fn asset_inventory(&self) -> AssetInventory {
        let _profile = profile::span("analysis.query.asset_inventory");
        let Some(index) = self.asset_index.as_ref() else {
            return AssetInventory::default();
        };
        let mut references_by_asset = BTreeMap::<PathBuf, Vec<AssetReference>>::new();

        for (source_path, file) in &self.files {
            if self.is_generated_path(source_path) {
                continue;
            }
            for metadata in &file.assets {
                let Some(asset_path) =
                    asset::resolved_asset_path(metadata, self.effective_asset_roots(), Some(index))
                else {
                    continue;
                };

                references_by_asset
                    .entry(asset_path)
                    .or_default()
                    .push(AssetReference {
                        path: source_path.clone(),
                        span: metadata.span,
                        loader_kind: metadata.kind,
                    });
            }
        }

        for references in references_by_asset.values_mut() {
            references.sort_by_key(asset_reference_key);
            references.dedup_by_key(|reference| asset_reference_key(reference));
        }

        let mut assets = index
            .files()
            .filter(|(path, ..)| !self.is_generated_path(path))
            .filter_map(|(path, relative_path, extension, dimensions)| {
                Some(ProjectAsset {
                    path: path.to_path_buf(),
                    relative_path: relative_path.to_owned(),
                    category: asset::asset_category_for_extension(extension)?,
                    extension: extension.to_owned(),
                    dimensions,
                    references: references_by_asset.remove(path).unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>();

        assets.sort_by(|left, right| {
            left.category
                .cmp(&right.category)
                .then_with(|| left.relative_path.cmp(&right.relative_path))
                .then_with(|| left.path.cmp(&right.path))
        });

        let diagnostics = assets
            .iter()
            .filter(|asset| asset.references.is_empty())
            .map(|asset| {
                Diagnostic::new(
                    asset.path.clone(),
                    1,
                    1,
                    crate::DiagnosticSeverity::Hint,
                    format!(
                        "asset `{}` has no static Tua or Lua references; dynamically loaded assets may still be used",
                        asset.relative_path
                    ),
                )
                .with_code(crate::DiagnosticCode::ASSET_UNUSED)
            })
            .collect();

        AssetInventory {
            assets,
            diagnostics,
        }
    }

    pub fn asset_file_rename_edits(
        &self,
        old_path: impl AsRef<Path>,
        new_path: impl AsRef<Path>,
    ) -> Vec<TextEdit> {
        let _profile = profile::span("analysis.query.asset_file_rename_edits");
        let old_path = normalize_path(old_path.as_ref());
        let new_path = normalize_path(new_path.as_ref());

        if old_path == new_path || self.asset_index.is_none() {
            return Vec::new();
        }

        let mut edits = Vec::new();

        for (source_path, file) in &self.files {
            for metadata in &file.assets {
                let Some(new_relative_path) = asset::renamed_asset_reference_path(
                    metadata,
                    &old_path,
                    &new_path,
                    self.effective_asset_roots(),
                    self.asset_index.as_ref(),
                ) else {
                    continue;
                };
                let Some(new_text) =
                    quoted_asset_path(&file.source, metadata.span, &new_relative_path)
                else {
                    continue;
                };

                edits.push(TextEdit {
                    path: source_path.clone(),
                    span: metadata.span,
                    new_text,
                });
            }
        }

        edits.sort_by_key(asset_text_edit_key);
        edits.dedup_by_key(|edit| asset_text_edit_key(edit));
        edits
    }
}

fn quoted_asset_path(source: &str, span: SourceSpan, path: &str) -> Option<String> {
    if span.start.line != span.end.line {
        return None;
    }

    let line = line_at(source, span.start.line)?;
    let start = byte_for_column(line, span.start.column);
    let end = byte_for_column(line, span.end.column);
    let original = line.get(start..end)?;
    let quote = if original.starts_with('\'') {
        '\''
    } else {
        '"'
    };
    let mut escaped = String::with_capacity(path.len());

    for character in path.chars() {
        if character == '\\' || character == quote {
            escaped.push('\\');
        }
        escaped.push(character);
    }

    Some(format!("{quote}{escaped}{quote}"))
}

fn asset_reference_key(reference: &AssetReference) -> (PathBuf, usize, usize, AssetLoaderKind) {
    (
        reference.path.clone(),
        reference.span.start.line,
        reference.span.start.column,
        reference.loader_kind,
    )
}

fn asset_text_edit_key(edit: &TextEdit) -> (PathBuf, usize, usize, usize, usize) {
    (
        edit.path.clone(),
        edit.span.start.line,
        edit.span.start.column,
        edit.span.end.line,
        edit.span.end.column,
    )
}
