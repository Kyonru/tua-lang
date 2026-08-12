use super::*;

impl AnalysisSnapshot {
    pub fn audio_graph(&self) -> crate::AudioGraph {
        let _profile = profile::span("analysis.query.audio_graph");
        crate::audio::audio_graph(
            self.files
                .values()
                .filter_map(|file| file.parsed.as_ref().ok()),
        )
    }
}
