use std::path::Path;

use crate::{
    references_parsed_source, references_tua_source, Diagnostic, ParsedSource, SourcePosition,
    SourceSpan,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentHighlight {
    pub span: SourceSpan,
    pub kind: DocumentHighlightKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentHighlightKind {
    Text,
}

pub fn document_highlights_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    position: SourcePosition,
) -> Result<Vec<DocumentHighlight>, Vec<Diagnostic>> {
    let references = references_tua_source(path, source, position, true)?;

    Ok(references
        .into_iter()
        .map(|reference| DocumentHighlight {
            span: reference.span,
            kind: DocumentHighlightKind::Text,
        })
        .collect())
}

pub fn document_highlights_parsed_source(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Result<Vec<DocumentHighlight>, Vec<Diagnostic>> {
    let references = references_parsed_source(parsed, position, true)?;

    Ok(references
        .into_iter()
        .filter(|reference| reference.path == parsed.path)
        .map(|reference| DocumentHighlight {
            span: reference.span,
            kind: DocumentHighlightKind::Text,
        })
        .collect())
}

#[cfg(test)]
mod tests;
