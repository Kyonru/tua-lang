use std::collections::{BTreeMap, BTreeSet};

use crate::{ParsedSource, SourcePosition, SourceSpan};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoweringKind {
    Source,
    DefaultParameter,
    BuiltinCall,
    Import,
    RuntimeTypeCheck,
    Class,
    FinalClassCall,
    CompoundAssignment,
    Sprite,
    Annotation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LuaLineProvenance {
    pub generated_line: usize,
    pub source_span: SourceSpan,
    pub lowering: LoweringKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaEmission {
    pub code: String,
    pub provenance: Vec<LuaLineProvenance>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoweredLine {
    text: String,
    source_span: SourceSpan,
    lowering: LoweringKind,
}

#[derive(Clone, Debug)]
pub(crate) struct LoweringDocument {
    lines: Vec<LoweredLine>,
    fallback_span: SourceSpan,
}

impl LoweringDocument {
    pub(crate) fn from_erased_lua(lua: String, parsed: &ParsedSource) -> Self {
        let fallback_span = parsed.span.unwrap_or_else(|| line_span(&parsed.source, 1));
        let source_lines = initial_source_lines(parsed, &lua);
        let lines = split_lines(&lua)
            .into_iter()
            .zip(source_lines)
            .map(|(text, source_span)| LoweredLine {
                text: text.to_owned(),
                source_span,
                lowering: LoweringKind::Source,
            })
            .collect();

        let mut document = Self {
            lines,
            fallback_span,
        };
        document.refine_class_provenance(parsed);
        document
    }

    pub(crate) fn code(&self) -> String {
        self.lines.iter().map(|line| line.text.as_str()).collect()
    }

    pub(crate) fn rewrite(&mut self, lowering: LoweringKind, next: String) {
        let current = self.code();
        if current == next {
            return;
        }

        let old = std::mem::take(&mut self.lines);
        let new = split_lines(&next);
        self.lines = align_rewrite(&old, &new, lowering, self.fallback_span);
    }

    pub(crate) fn refine_default_parameter_provenance(&mut self, parsed: &ParsedSource) {
        let mut search_start = 0usize;

        for function in &parsed.function_annotations {
            for default in &function.default_parameters {
                let expected = format!("if {} == nil then", default.name);
                let Some(relative_index) = self.lines[search_start..]
                    .iter()
                    .position(|line| line.text.trim() == expected)
                else {
                    continue;
                };
                let index = search_start + relative_index;
                let source_span = default.span.or(function.span).unwrap_or(self.fallback_span);

                for line in self.lines.iter_mut().skip(index).take(3) {
                    if line.lowering == LoweringKind::DefaultParameter {
                        line.source_span = source_span;
                    }
                }

                search_start = index + 3;
            }
        }
    }

    pub(crate) fn refine_builtin_require_provenance(&mut self) {
        self.refine_generated_require("local __tua_vec2 = require(\"brinevector\")", "__tua_vec2(");
        self.refine_generated_require(
            "local __tua_signal = require(\"tuasignal\")",
            "__tua_signal(",
        );
    }

    pub(crate) fn refine_sprite_provenance(&mut self, parsed: &ParsedSource) {
        let mut search_start = 0usize;

        for (marker, line_count, source_span) in crate::sprite::lowering_provenance_blocks(parsed) {
            let Some(relative_start) = self.lines[search_start..]
                .iter()
                .position(|line| line.text.trim() == marker)
            else {
                continue;
            };
            let start = search_start + relative_start;

            for line in self.lines.iter_mut().skip(start).take(line_count) {
                line.source_span = source_span;
                line.lowering = LoweringKind::Sprite;
            }

            search_start = start + line_count;
        }
    }

    fn refine_class_provenance(&mut self, parsed: &ParsedSource) {
        let mut search_start = 0usize;

        for class in &parsed.class_declarations {
            let start_marker = format!("local {} = {{}}", class.name);
            let end_marker = format!("{}.new = {}_new", class.name, class.name);
            let Some(relative_start) = self.lines[search_start..]
                .iter()
                .position(|line| line.text.trim() == start_marker)
            else {
                continue;
            };
            let start = search_start + relative_start;
            let Some(relative_end) = self.lines[start..]
                .iter()
                .position(|line| line.text.trim() == end_marker)
            else {
                continue;
            };
            let end = start + relative_end;

            for line in &mut self.lines[start..=end] {
                line.source_span = class.span;
                line.lowering = LoweringKind::Class;
            }

            search_start = end + 1;
        }
    }

    fn refine_generated_require(&mut self, require: &str, constructor: &str) {
        let Some(source_span) = self
            .lines
            .iter()
            .find(|line| line.text.contains(constructor))
            .map(|line| line.source_span)
        else {
            return;
        };
        let Some(line) = self
            .lines
            .iter_mut()
            .find(|line| line.text.trim() == require)
        else {
            return;
        };

        line.source_span = source_span;
    }

    pub(crate) fn finish(self) -> LuaEmission {
        let raw = self.code();
        let trimmed = raw.trim_matches(char::is_whitespace);
        if trimmed.is_empty() {
            return LuaEmission {
                code: String::new(),
                provenance: Vec::new(),
            };
        }

        let leading = raw.len() - raw.trim_start_matches(char::is_whitespace).len();
        let code = format!("{trimmed}\n");
        let mut provenance = Vec::new();
        let mut raw_offset = leading;

        for (index, line) in split_lines(&code).into_iter().enumerate() {
            let lowered = line_at_offset(&self.lines, raw_offset).unwrap_or_else(|| {
                self.lines
                    .last()
                    .expect("non-empty lowering output should retain one provenance line")
            });

            provenance.push(LuaLineProvenance {
                generated_line: index + 1,
                source_span: lowered.source_span,
                lowering: lowered.lowering,
            });
            raw_offset += line.len();
        }

        LuaEmission { code, provenance }
    }
}

fn align_rewrite(
    old: &[LoweredLine],
    new: &[&str],
    lowering: LoweringKind,
    fallback_span: SourceSpan,
) -> Vec<LoweredLine> {
    let mut output = Vec::with_capacity(new.len());
    align_region(old, new, lowering, None, None, fallback_span, &mut output);

    output
}

fn align_region(
    old: &[LoweredLine],
    new: &[&str],
    lowering: LoweringKind,
    before_span: Option<SourceSpan>,
    after_span: Option<SourceSpan>,
    fallback_span: SourceSpan,
    output: &mut Vec<LoweredLine>,
) {
    if new.is_empty() {
        return;
    }

    if old.is_empty() {
        let source_span = after_span.or(before_span).unwrap_or(fallback_span);
        output.extend(
            new.iter()
                .map(|line| generated_line(line, source_span, lowering)),
        );
        return;
    }

    let prefix = common_prefix_len(old, new);
    output.extend(old[..prefix].iter().cloned());

    let old = &old[prefix..];
    let new = &new[prefix..];
    let before_span = output.last().map(|line| line.source_span).or(before_span);
    if new.is_empty() {
        return;
    }

    let suffix = common_suffix_len(old, new);
    let old_middle = &old[..old.len() - suffix];
    let new_middle = &new[..new.len() - suffix];
    let old_suffix = &old[old.len() - suffix..];
    let after_span = old_suffix
        .first()
        .map(|line| line.source_span)
        .or(after_span);

    if let Some((old_anchor, new_anchor)) = unique_anchor(old_middle, new_middle) {
        let anchor_span = old_middle[old_anchor].source_span;
        align_region(
            &old_middle[..old_anchor],
            &new_middle[..new_anchor],
            lowering,
            before_span,
            Some(anchor_span),
            fallback_span,
            output,
        );
        output.push(old_middle[old_anchor].clone());
        align_region(
            &old_middle[old_anchor + 1..],
            &new_middle[new_anchor + 1..],
            lowering,
            Some(anchor_span),
            after_span,
            fallback_span,
            output,
        );
    } else {
        align_changed_region(
            old_middle,
            new_middle,
            lowering,
            before_span,
            after_span,
            fallback_span,
            output,
        );
    }

    output.extend(old_suffix.iter().cloned());
}

fn align_changed_region(
    old: &[LoweredLine],
    new: &[&str],
    lowering: LoweringKind,
    before_span: Option<SourceSpan>,
    after_span: Option<SourceSpan>,
    fallback_span: SourceSpan,
    output: &mut Vec<LoweredLine>,
) {
    for (index, text) in new.iter().enumerate() {
        if old.get(index).is_some_and(|line| line.text == *text) {
            output.push(old[index].clone());
            continue;
        }

        let source_span = old
            .get(index)
            .or_else(|| old.last())
            .map(|line| line.source_span)
            .or(after_span)
            .or(before_span)
            .unwrap_or(fallback_span);
        output.push(generated_line(text, source_span, lowering));
    }
}

fn common_prefix_len(old: &[LoweredLine], new: &[&str]) -> usize {
    old.iter()
        .zip(new)
        .take_while(|(old, new)| old.text == **new)
        .count()
}

fn common_suffix_len(old: &[LoweredLine], new: &[&str]) -> usize {
    old.iter()
        .rev()
        .zip(new.iter().rev())
        .take_while(|(old, new)| old.text == ***new)
        .count()
}

fn unique_anchor(old: &[LoweredLine], new: &[&str]) -> Option<(usize, usize)> {
    let mut old_occurrences = BTreeMap::<&str, (usize, usize)>::new();
    let mut new_occurrences = BTreeMap::<&str, (usize, usize)>::new();

    for (index, line) in old.iter().enumerate() {
        if !line.text.trim().is_empty() {
            let occurrence = old_occurrences
                .entry(line.text.as_str())
                .or_insert((0, index));
            occurrence.0 += 1;
        }
    }
    for (index, line) in new.iter().enumerate() {
        if !line.trim().is_empty() {
            let occurrence = new_occurrences.entry(line).or_insert((0, index));
            occurrence.0 += 1;
        }
    }

    old_occurrences
        .into_iter()
        .filter_map(|(line, (old_count, old_index))| {
            let (new_count, new_index) = new_occurrences.get(line).copied()?;
            (old_count == 1 && new_count == 1).then_some((old_index, new_index))
        })
        .min_by_key(|(old_index, _)| old_index.abs_diff(old.len() / 2))
}

fn generated_line(text: &str, source_span: SourceSpan, lowering: LoweringKind) -> LoweredLine {
    LoweredLine {
        text: text.to_owned(),
        source_span,
        lowering,
    }
}

fn line_at_offset(lines: &[LoweredLine], offset: usize) -> Option<&LoweredLine> {
    let mut cursor = 0usize;

    for line in lines {
        let end = cursor + line.text.len();
        if offset < end || (line.text.is_empty() && offset == cursor) {
            return Some(line);
        }
        cursor = end;
    }

    lines.last()
}

fn split_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        Vec::new()
    } else {
        text.split_inclusive('\n').collect()
    }
}

fn initial_source_lines(parsed: &ParsedSource, lua: &str) -> Vec<SourceSpan> {
    let source_lines = parsed.source.lines().collect::<Vec<_>>();
    let skipped = erased_type_syntax_lines(parsed);
    let mut source_line = 1usize;
    let mut output = Vec::new();
    let mut class_index = 0usize;
    let mut active_class = None::<usize>;

    for generated in split_lines(lua) {
        let body = generated.trim_end_matches(['\r', '\n']);

        if let Some(index) = active_class {
            let class = &parsed.class_declarations[index];
            output.push(class.span);

            if body.trim() == format!("{}.new = {}_new", class.name, class.name) {
                source_line = class.span.end.line + 1;
                class_index = index + 1;
                active_class = None;
            }
            continue;
        }

        if let Some(class) = parsed.class_declarations.get(class_index) {
            if body.trim() == format!("local {} = {{}}", class.name) {
                output.push(class.span);
                active_class = Some(class_index);
                continue;
            }
        }

        if body.trim().is_empty() {
            while skipped.contains(&source_line) {
                source_line += 1;
            }

            if source_lines
                .get(source_line.saturating_sub(1))
                .is_some_and(|line| line.trim().is_empty())
            {
                output.push(line_span(&parsed.source, source_line));
                source_line += 1;
            } else {
                output.push(line_span(
                    &parsed.source,
                    source_line.saturating_sub(1).max(1),
                ));
            }
            continue;
        }

        source_line = next_source_line(&source_lines, &skipped, source_line, body);
        output.push(line_span(&parsed.source, source_line));
        source_line += 1;
    }

    output
}

fn erased_type_syntax_lines(parsed: &ParsedSource) -> BTreeSet<usize> {
    // Parser-normalized class blocks can shift AST positions, and table type ranges omit the
    // closing brace, so bootstrap erased declarations from the original source until class
    // normalization moves into lowering.
    erased_type_syntax_lines_from_source(&parsed.source)
}

fn erased_type_syntax_lines_from_source(source: &str) -> BTreeSet<usize> {
    let mut lines = BTreeSet::new();
    let mut type_brace_depth = 0isize;
    let mut in_enum = false;

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();

        if in_enum {
            lines.insert(line_number);
            if trimmed == "end" {
                in_enum = false;
            }
            continue;
        }

        if type_brace_depth > 0 {
            lines.insert(line_number);
            type_brace_depth += brace_delta(trimmed);
            continue;
        }

        if trimmed.starts_with("enum ") {
            lines.insert(line_number);
            in_enum = true;
            continue;
        }

        if trimmed.starts_with("type ") || trimmed.starts_with("export type ") {
            lines.insert(line_number);
            type_brace_depth = brace_delta(trimmed).max(0);
        }
    }

    lines
}

fn brace_delta(line: &str) -> isize {
    line.chars()
        .map(|character| match character {
            '{' => 1,
            '}' => -1,
            _ => 0,
        })
        .sum()
}

fn next_source_line(
    source_lines: &[&str],
    skipped: &BTreeSet<usize>,
    mut source_line: usize,
    generated_line: &str,
) -> usize {
    while source_line <= source_lines.len() {
        if skipped.contains(&source_line) {
            source_line += 1;
            continue;
        }

        if !generated_line.trim().is_empty() && source_lines[source_line - 1].trim().is_empty() {
            source_line += 1;
            continue;
        }

        return source_line;
    }

    source_line.clamp(1, source_lines.len().max(1))
}

fn line_span(source: &str, line: usize) -> SourceSpan {
    let text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
    SourceSpan {
        start: SourcePosition { line, column: 1 },
        end: SourcePosition {
            line,
            column: text.chars().count() + 1,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_tua_source;

    #[test]
    fn rewrite_preserves_unchanged_lines_and_marks_generated_lines() {
        let parsed = parse_tua_source(
            "src/main.tua",
            "local value: number = 1\nvalue += 2\nprint(value)\n",
        )
        .expect("fixture should parse");
        let mut document = LoweringDocument::from_erased_lua(
            "local value = 1\nvalue += 2\nprint(value)\n".to_owned(),
            &parsed,
        );

        document.rewrite(
            LoweringKind::CompoundAssignment,
            "local value = 1\nvalue = value + 2\nprint(value)\n".to_owned(),
        );
        let emission = document.finish();

        assert_eq!(emission.provenance[0].lowering, LoweringKind::Source);
        assert_eq!(
            emission.provenance[1].lowering,
            LoweringKind::CompoundAssignment
        );
        assert_eq!(emission.provenance[1].source_span.start.line, 2);
        assert_eq!(emission.provenance[2].lowering, LoweringKind::Source);
    }

    #[test]
    fn rewrite_maps_inserted_lines_to_the_following_source_node() {
        let parsed = parse_tua_source(
            "src/main.tua",
            "local function greet(name: string = \"Ada\")\n  return name\nend\n",
        )
        .expect("fixture should parse");
        let mut document = LoweringDocument::from_erased_lua(
            "local function greet(name)\n  return name\nend\n".to_owned(),
            &parsed,
        );

        document.rewrite(
            LoweringKind::DefaultParameter,
            "local function greet(name)\n  if name == nil then\n    name = \"Ada\"\n  end\n  return name\nend\n"
                .to_owned(),
        );
        document.refine_default_parameter_provenance(&parsed);
        let emission = document.finish();

        assert!(emission.provenance[1..4]
            .iter()
            .all(|line| line.lowering == LoweringKind::DefaultParameter));
        assert!(emission.provenance[1..4]
            .iter()
            .all(|line| line.source_span.start.line == 1));
    }

    #[test]
    fn rewrite_preserves_following_source_after_large_expansion() {
        let source = "local before = 1\nlocal after = 2\n";
        let parsed = parse_tua_source("src/main.tua", source).expect("fixture should parse");
        let mut document = LoweringDocument::from_erased_lua(source.to_owned(), &parsed);
        let mut expanded = "local before = 1\n".to_owned();

        for index in 0..80 {
            expanded.push_str(&format!("-- generated {index}\n"));
        }
        expanded.push_str("local after = 2\n");

        document.rewrite(LoweringKind::Annotation, expanded);
        let emission = document.finish();

        assert_eq!(emission.provenance.len(), 82);
        assert!(emission.provenance[1..81]
            .iter()
            .all(|line| line.lowering == LoweringKind::Annotation));
        assert_eq!(emission.provenance[81].lowering, LoweringKind::Source);
        assert_eq!(emission.provenance[81].source_span.start.line, 2);
    }
}
