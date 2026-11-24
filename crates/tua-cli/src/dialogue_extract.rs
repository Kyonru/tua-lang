//! Read-only extraction: the host decides where to store/merge this manifest.
use crate::{
    error::CliError,
    source::{discover_project_files, read_source_file, SourceKind},
};
use std::{collections::BTreeMap, path::Path};

pub(crate) fn run(cwd: &Path) -> Result<String, CliError> {
    let loaded = tua_core::load_config_from_dir(cwd).map_err(CliError::from_config_error)?;
    let workspace = tua_core::ProjectWorkspace::from_loaded(cwd, loaded);
    let files = discover_project_files(&workspace, false, &[])
        .map_err(|e| CliError::from_diagnostics("dialogue discovery failed", vec![e]))?;
    let mut entries = BTreeMap::new();
    for file in files
        .iter()
        .filter(|file| file.kind == SourceKind::Dialogue)
    {
        let source = read_source_file(&file.path)
            .map_err(|e| CliError::from_diagnostics("dialogue read failed", vec![e]))?;
        let model = tua_core::dialogue_asset::parse_dialogue_source(&file.path, &source)
            .map_err(|errors| CliError::from_diagnostics("dialogue extraction failed", errors))?;
        for line in model.sections.iter().flat_map(|section| &section.lines) {
            for text in std::iter::once(&line.text).chain(
                line.choices
                    .iter()
                    .flat_map(|choice| std::iter::once(&choice.text).chain(choice.reason.iter())),
            ) {
                let value = serde_json::json!({ "text": text.value,
                    "path": file.path.strip_prefix(workspace.primary_source_root()).unwrap_or(&file.path),
                    "line": text.id.span.start.line, "documentId": model.document_id.as_ref().map(|id| &id.value) });
                if entries.insert(text.id.value.clone(), value).is_some() {
                    return Err(CliError::usage(format!(
                        "duplicate dialogue text ID: {}",
                        text.id.value
                    )));
                }
            }
        }
    }
    serde_json::to_string_pretty(&serde_json::json!({ "schemaVersion": 1, "entries": entries }))
        .map_err(|error| CliError::usage(error.to_string()))
}
