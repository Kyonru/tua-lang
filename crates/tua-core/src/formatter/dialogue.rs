//! Conservative, whole-document layout for the dialogue DSL, never Lua layout.
use super::{FormatOptions, LineEnding};
use crate::{dialogue_asset::parse_dialogue_source, Diagnostic};
use std::path::Path;

pub(super) fn format(
    path: &Path,
    source: &str,
    options: &FormatOptions,
) -> Result<String, Vec<Diagnostic>> {
    parse_dialogue_source(path, source)?;
    let mut lines = Vec::new();
    let mut pending_blank = false;
    for raw in source.lines() {
        // The DSL parser discards only this structural indentation. Whitespace
        // following a speaker/choice/reason prefix belongs to the authored text.
        let line = raw.trim_start_matches([' ', '\t']);
        if line.trim().is_empty() {
            pending_blank = !lines.is_empty();
            continue;
        }
        if pending_blank {
            lines.push(String::new());
        }
        pending_blank = false;
        let formatted = if line.starts_with('[') || line.starts_with("//") {
            line.trim_end().to_owned()
        } else if line.starts_with('@') && !line.starts_with("@reason ") {
            let line = line.trim_end();
            if let Some((directive, rest)) = line.split_once(char::is_whitespace) {
                format!("{directive} {}", rest.trim_start())
            } else {
                line.to_owned()
            }
        } else {
            // Do not reflow prose, normalize spaces, rewrite escapes or serialize
            // JSON. In particular, preserve trailing spaces in all text fields.
            line.to_owned()
        };
        lines.push(formatted);
    }
    let ending = match options.line_ending {
        LineEnding::Crlf => "\r\n",
        LineEnding::Preserve if source.contains("\r\n") => "\r\n",
        _ => "\n",
    };
    let mut formatted = lines.join(ending);
    if options.final_newline {
        formatted.push_str(ending);
    }
    // A future grammar change must not turn a layout operation into invalid code.
    parse_dialogue_source(path, &formatted)?;
    Ok(formatted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatter::{
        format_tua_source, format_tua_source_on_type, format_tua_source_range, QuoteStyle,
    };
    use crate::{SourcePosition, SourceSpan};

    #[test]
    fn dialogue_layout_preserves_text_and_metadata_attachment() {
        let source = "\r\n  @dialogue 1\r\n\r\n\r\n  @character guide \"Guide\"  \r\n  [start]  \r\n  // @id hello   \r\n  guide:  é🦆  %{name}  \r\n  @timing  hold 2 fade 0.5  \r\n  // @id choice\r\n  ->  Keep  spaces [target:done]\r\n\r\n  [done]\r\n  // @id done\r\n  : Bye (end)\r\n\r\n";
        let expected = "@dialogue 1\n\n@character guide \"Guide\"\n[start]\n// @id hello\nguide:  é🦆  %{name}  \n@timing hold 2 fade 0.5\n// @id choice\n->  Keep  spaces [target:done]\n\n[done]\n// @id done\n: Bye (end)\n";
        let options = FormatOptions {
            line_width: 20,
            quote_style: QuoteStyle::Single,
            ..FormatOptions::default()
        };
        let result = format_tua_source("scene.tdialogue", source, &options).unwrap();
        assert_eq!(result, expected);
        assert_eq!(
            format_tua_source("scene.tdialogue", &result, &options).unwrap(),
            result
        );
        let crlf = FormatOptions {
            line_ending: LineEnding::Preserve,
            final_newline: false,
            ..options
        };
        assert_eq!(
            format_tua_source("scene.tdialogue", source, &crlf).unwrap(),
            expected.trim_end_matches('\n').replace('\n', "\r\n")
        );
        assert!(format_tua_source(
            "bad.tdialogue",
            "@dialogue 1\n[start]\n// @id unfinished",
            &FormatOptions::default()
        )
        .is_err());
        let range = SourceSpan {
            start: SourcePosition { line: 1, column: 1 },
            end: SourcePosition { line: 3, column: 1 },
        };
        assert!(format_tua_source_range(
            "scene.tdialogue",
            source,
            &FormatOptions::default(),
            range
        )
        .unwrap()
        .is_empty());
        assert!(format_tua_source_on_type(
            "scene.tdialogue",
            source,
            &FormatOptions::default(),
            range.start,
            "\n"
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn dialogue_example_corpus_round_trips_emitted_semantics() {
        fn scripts(root: &Path, found: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(root).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    scripts(&path, found);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension == "tdialogue")
                {
                    found.push(path);
                }
            }
        }
        let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        let mut files = Vec::new();
        for entry in std::fs::read_dir(examples).unwrap() {
            let path = entry.unwrap().path();
            if path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("dialogue")
                && path.join("src").is_dir()
            {
                scripts(&path.join("src"), &mut files);
            }
        }
        assert!(files.len() >= 10);
        fn without_coordinates(code: &str) -> String {
            let mut result = String::new();
            let mut rest = code;
            while let Some((before, after)) = rest.split_once("source = { line = ") {
                result.push_str(before);
                let (line, after) = after.split_once(", column = ").unwrap();
                let (column, after) = after.split_once(" }").unwrap();
                assert!(line.bytes().all(|byte| byte.is_ascii_digit()));
                assert!(column.bytes().all(|byte| byte.is_ascii_digit()));
                result.push_str("source = {}");
                rest = after;
            }
            result.push_str(rest);
            result
        }
        for path in files {
            let original = std::fs::read_to_string(&path).unwrap();
            let indented = "\r\n".to_owned()
                + &original
                    .lines()
                    .map(|line| format!("  {line}\r\n"))
                    .collect::<String>();
            let formatted = format_tua_source(&path, &indented, &FormatOptions::default()).unwrap();
            assert_eq!(
                format_tua_source(&path, &formatted, &FormatOptions::default()).unwrap(),
                formatted,
                "{}",
                path.display()
            );
            let before = crate::emit::emit_dialogue_source(&path, &indented).unwrap();
            let after = crate::emit::emit_dialogue_source(&path, &formatted).unwrap();
            // Source coordinates intentionally follow the new layout; all emitted
            // text, payloads, graph edges and stable identities must stay identical.
            assert_eq!(
                without_coordinates(&before.code),
                without_coordinates(&after.code),
                "{}",
                path.display()
            );
            crate::parse_lua_source("formatted.lua", after.code).unwrap();
            assert!(after
                .provenance
                .iter()
                .all(|origin| origin.source_span.start.line <= formatted.lines().count()));
        }
    }
}
