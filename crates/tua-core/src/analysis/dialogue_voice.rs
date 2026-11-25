//! Voice navigation joins authored cue identities; it never resolves audio files.
use super::*;
use crate::dialogue_asset::voice;

impl AnalysisSnapshot {
    fn voice_occurrences(
        &self,
        path: &Path,
    ) -> Vec<(PathBuf, crate::dialogue_asset::DialogueName)> {
        self.files
            .iter()
            .filter(|(other, file)| {
                file.kind == AnalysisSourceKind::Dialogue
                    && self.workspace_root_for_path(other) == self.workspace_root_for_path(path)
                    && file.dialogue_model.get().is_some_and(Option::is_some)
            })
            .flat_map(|(other, file)| {
                voice::cues(&file.source)
                    .into_iter()
                    .map(|cue| (other.clone(), cue))
            })
            .collect()
    }

    pub(super) fn dialogue_voice_completion(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Vec<Completion>> {
        let cue = voice::cue_at(&self.file(path)?.source, position, true)?;
        let names: BTreeSet<_> = self
            .voice_occurrences(path)
            .into_iter()
            .map(|(_, name)| name.value)
            .filter(|name| name.starts_with(&cue.value))
            .collect();
        Some(
            names
                .into_iter()
                .map(|name| {
                    let quoted = serde_json::to_string(&name).expect("string JSON serialization");
                    Completion {
                        label: name,
                        kind: CompletionKind::Value,
                        detail: Some(
                            "Authored voice cue; audio availability is host-defined".into(),
                        ),
                        documentation: None,
                        deprecation: None,
                        insert_text: Some(quoted[1..quoted.len() - 1].into()),
                        filter_text: None,
                        is_snippet: false,
                        module_path: None,
                        source_module: None,
                        additional_text_edits: Vec::new(),
                    }
                })
                .collect(),
        )
    }

    pub(super) fn dialogue_voice_hover(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Hover> {
        let cue = voice::cue_at(&self.file(path)?.source, position, false)?;
        Some(Hover {
            is_markdown: false,
            contents: format!("Voice cue: {}", cue.value),
            span: Some(cue.span), documentation: None,
            context: vec!["Logical host-audio identifier; locale selection and audio availability are resolved by the host, not by Tua".into()],
        })
    }

    pub(super) fn dialogue_voice_references(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Vec<Reference>> {
        let file = self.file(path)?;
        file.dialogue_model.get()?.as_ref()?;
        let selected = voice::cue_at(&file.source, position, false)?;
        Some(
            self.voice_occurrences(path)
                .into_iter()
                .filter(|(_, cue)| cue.value == selected.value)
                .map(|(path, cue)| Reference {
                    path,
                    span: cue.span,
                    kind: ReferenceKind::Asset,
                })
                .collect(),
        )
    }
}
