//! Expression authoring shares the parser's metadata attachment rules.
use super::{identifier, parser_for, span, CharacterImport, DialogueName};
use crate::{SourcePosition, SourceSpan};
use std::path::Path;

pub(crate) struct ExpressionTarget {
    pub import: CharacterImport,
    pub character: String,
    pub name: DialogueName,
}

fn name_at(source: &str, position: SourcePosition) -> Option<DialogueName> {
    let raw = source.lines().nth(position.line.checked_sub(1)?)?;
    let rest = raw.trim_start().strip_prefix("@expression ")?.trim_start();
    let value = rest.trim_end();
    if !value.is_empty() && !identifier(value) {
        return None;
    }
    let start = raw.chars().count() - rest.chars().count() + 1;
    let range = span(position.line, start, value.len());
    if position.column < start || position.column > range.end.column {
        return None;
    }
    Some(DialogueName {
        value: value.into(),
        span: range,
    })
}

pub(crate) fn target_at(
    path: &Path,
    source: &str,
    position: SourcePosition,
) -> Option<ExpressionTarget> {
    if source.len() > crate::input_limits::MAX_SOURCE_BYTES {
        return None;
    }
    let name = name_at(source, position)?;
    let mut parser = parser_for(path);
    for (index, line) in source.lines().take(position.line - 1).enumerate() {
        parser.line(index + 1, line);
    }
    // A valid temporary name lets incomplete input use the real attachment and
    // duplicate checks. No finish pass: subsequent choices/targets may be unfinished.
    parser.line(position.line, "@expression _tua_authoring_probe");
    if !parser.diagnostics.is_empty() {
        return None;
    }
    let line = parser.document.sections.last()?.lines.last()?;
    line.expression.as_ref()?;
    let (alias, character) = line.speaker.as_ref()?.value.split_once('.')?;
    let mut imports = parser
        .document
        .imports
        .iter()
        .filter(|import| import.alias.value == alias);
    let import = imports.next()?.clone();
    if imports.next().is_some() {
        return None;
    }
    Some(ExpressionTarget {
        import,
        character: character.into(),
        name,
    })
}

pub fn replacement_span(source: &str, position: SourcePosition) -> Option<SourceSpan> {
    name_at(source, position).map(|name| name.span)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "@dialogue 1\n@characters Cast \"cast\"\n[start]\n// @id hello\nCast.Guide: é🦆 Hello (end)\n";

    fn marked(source: &str) -> (String, SourcePosition) {
        let (before, after) = source.split_once('|').unwrap();
        let line = before.lines().count();
        let column = before.rsplit('\n').next().unwrap().chars().count() + 1;
        (format!("{before}{after}"), SourcePosition { line, column })
    }

    #[test]
    fn expression_authoring_uses_real_attachment_and_whole_name_ranges() {
        let (source, position) = marked(&format!("{HEADER}@timing hold 1 fade 0.2\n// keep metadata attached\n  @expression face.ha|ppy-id\n[unfinished]\n"));
        let target = target_at(Path::new("scene.tdialogue"), &source, position).unwrap();
        assert_eq!(target.import.module.value, "cast");
        assert_eq!(target.character, "Guide");
        assert_eq!(target.name.value, "face.happy-id");
        assert_eq!(target.name.span.start.column, 15);
        assert_eq!(replacement_span(&source, position), Some(target.name.span));
        let (empty, cursor) = marked(&format!("{HEADER}@expression |"));
        assert_eq!(
            target_at(Path::new("scene.tdialogue"), &empty, cursor)
                .unwrap()
                .name
                .value,
            ""
        );
    }

    #[test]
    fn expression_authoring_rejects_invalid_ownership_and_duplicates() {
        for middle in [
            "@expression happy\n",
            "// @id choice\n-> Go [target:start]\n",
            "[next]\n",
            "@wait 1 [target:start]\n",
            "@branch true [target:start] [else:start]\n",
            "@unknown ignored\n",
            "// @id next\n: Narrator (end)\n",
            "// @id next\n",
        ] {
            let (source, position) = marked(&format!("{HEADER}{middle}@expression ha|ppy"));
            assert!(
                target_at(Path::new("scene.tdialogue"), &source, position).is_none(),
                "{middle}"
            );
        }
        for replacement in [
            "@characters Cast \"cast\"\n@characters Cast \"other\"",
            "@characters Cast \"cast\"\n@character Cast.Guide \"Local\"",
        ] {
            let source =
                HEADER.replace("@characters Cast \"cast\"", replacement) + "@expression ha|ppy";
            let (source, position) = marked(&source);
            assert!(target_at(Path::new("scene.tdialogue"), &source, position).is_none());
        }
    }
}
