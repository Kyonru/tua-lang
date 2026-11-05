//! One catalog for recognized tags, editor explanations and generated guides.
use std::sync::OnceLock;

#[derive(serde::Deserialize)]
pub(crate) struct AnnotationTag {
    pub name: String,
    pub detail: String,
    pub syntax: String,
    pub description: String,
}

impl AnnotationTag {
    pub fn documentation(&self) -> String {
        format!(
            "```lua\n{}\n```\n\n{}\n\n[Open Tua annotation guide](command:tua.openAnnotationGuide)",
            self.syntax, self.description
        )
    }
}

pub(crate) fn all() -> &'static [AnnotationTag] {
    static TAGS: OnceLock<Vec<AnnotationTag>> = OnceLock::new();
    TAGS.get_or_init(|| {
        serde_json::from_str(include_str!("../resources/annotation-tags.json"))
            .expect("bundled annotation catalog must be valid")
    })
}

pub(crate) fn get(name: &str) -> Option<&'static AnnotationTag> {
    all().iter().find(|tag| tag.name == name)
}

#[cfg(test)]
mod tests {
    #[test]
    fn annotation_catalog_examples_parse_and_tags_are_unique() {
        let mut names = std::collections::BTreeSet::new();
        for tag in super::all() {
            assert!(names.insert(&tag.name));
            let result = crate::parser::parse_lua_doc_annotations("guide.lua", &tag.syntax);
            assert_eq!(result.annotations.len(), 1, "{}", tag.syntax);
        }
    }
}
