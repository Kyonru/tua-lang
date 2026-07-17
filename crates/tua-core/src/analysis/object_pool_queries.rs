use super::*;

impl AnalysisSnapshot {
    pub fn object_pool_graph(&self) -> crate::ObjectPoolGraph {
        let _profile = profile::span("analysis.query.object_pool_graph");

        crate::object_pool::object_pool_graph(
            self.files
                .values()
                .map(|file| file.object_pool_index.as_ref()),
        )
    }
}
