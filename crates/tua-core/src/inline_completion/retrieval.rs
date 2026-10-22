//! Immutable, source-local exact-prefix retrieval primitives.
//!
//! The table stores complete normalized lines as exact keys and uses a sorted
//! range for prefix lookup. Keeping the complete key avoids a quadratic table
//! of every possible prefix while still comparing the original key after the
//! binary search; a hash collision can therefore never produce source text.

use std::collections::BTreeMap;
use std::mem::size_of;

use super::eligibility::strip_trailing_comment;
use super::recall::{continuation_after, normalize};

/// Conservative limits for one immutable retrieval table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetrievalLimits {
    /// Sources larger than this are not indexed.
    pub max_source_bytes: usize,
    /// Individual lines larger than this are skipped before normalization.
    pub max_line_bytes: usize,
    /// Longer continuations are not returned.
    pub max_continuation_bytes: usize,
    /// Approximate retained bytes for this table, including keys and line IDs.
    pub max_table_bytes: usize,
}

impl Default for RetrievalLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 256 * 1024,
            max_line_bytes: 16 * 1024,
            max_continuation_bytes: 4 * 1024,
            max_table_bytes: 512 * 1024,
        }
    }
}

/// Why a source did not produce a retrieval table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetrievalBuildError {
    SourceTooLarge,
    TableBudgetTooSmall,
}

#[derive(Clone, Debug)]
struct LinePattern {
    normalized: Box<str>,
    original: Box<str>,
    line_indices: Box<[usize]>,
}

pub(crate) struct FilePatternEntry<'a> {
    pub normalized: &'a str,
    pub original: &'a str,
    pub line_indices: &'a [usize],
}

/// A continuation retrieved from original source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetrievalMatch {
    pub continuation: String,
    /// Zero-based source lines that support this exact continuation.
    pub line_indices: Vec<usize>,
}

impl RetrievalMatch {
    pub fn support(&self) -> usize {
        self.line_indices.len()
    }
}

/// An immutable per-file table for collision-safe normalized-prefix lookup.
#[derive(Clone, Debug)]
pub struct FilePatternTable {
    patterns: Box<[LinePattern]>,
    source_bytes: usize,
    approximate_bytes: usize,
    indexed_lines: usize,
    skipped_lines: usize,
    truncated: bool,
    max_query_prefix_bytes: usize,
    max_continuation_bytes: usize,
}

impl FilePatternTable {
    /// Build a table without retaining the source buffer.
    pub fn build(source: &str, limits: RetrievalLimits) -> Result<Self, RetrievalBuildError> {
        if source.len() > limits.max_source_bytes {
            return Err(RetrievalBuildError::SourceTooLarge);
        }
        if limits.max_table_bytes < size_of::<Self>() {
            return Err(RetrievalBuildError::TableBudgetTooSmall);
        }

        // Both strings are part of the identity. Lines that normalize alike
        // but use different quote or whitespace styles remain distinct so a
        // query can return the exact authored continuation.
        let mut grouped = BTreeMap::<(String, String), Vec<usize>>::new();
        let mut retained_bytes = size_of::<Self>();
        let mut indexed_lines = 0;
        let mut skipped_lines = 0;
        let mut truncated = false;

        for (line_index, raw_line) in source.split('\n').enumerate() {
            let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
            if line.len() > limits.max_line_bytes {
                skipped_lines += 1;
                continue;
            }
            let original = line.trim_start();
            let (normalized, _) = normalize(original);
            if normalized.trim().is_empty() {
                continue;
            }

            let key = (normalized, original.to_owned());
            if let Some(lines) = grouped.get_mut(&key) {
                if retained_bytes.saturating_add(size_of::<usize>()) > limits.max_table_bytes {
                    skipped_lines += 1;
                    truncated = true;
                    continue;
                }
                lines.push(line_index);
                retained_bytes += size_of::<usize>();
                indexed_lines += 1;
                continue;
            }

            let entry_bytes = size_of::<LinePattern>()
                .saturating_add(key.0.len())
                .saturating_add(key.1.len())
                .saturating_add(size_of::<usize>());
            if retained_bytes.saturating_add(entry_bytes) > limits.max_table_bytes {
                skipped_lines += 1;
                truncated = true;
                continue;
            }
            retained_bytes += entry_bytes;
            indexed_lines += 1;
            grouped.insert(key, vec![line_index]);
        }

        let patterns = grouped
            .into_iter()
            .map(|((normalized, original), line_indices)| LinePattern {
                normalized: normalized.into_boxed_str(),
                original: original.into_boxed_str(),
                line_indices: line_indices.into_boxed_slice(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let approximate_bytes = size_of::<Self>()
            + patterns.len() * size_of::<LinePattern>()
            + patterns
                .iter()
                .map(|pattern| {
                    pattern.normalized.len()
                        + pattern.original.len()
                        + pattern.line_indices.len() * size_of::<usize>()
                })
                .sum::<usize>();
        debug_assert_eq!(approximate_bytes, retained_bytes);
        debug_assert!(approximate_bytes <= limits.max_table_bytes);

        Ok(Self {
            patterns,
            source_bytes: source.len(),
            approximate_bytes,
            indexed_lines,
            skipped_lines,
            truncated,
            max_query_prefix_bytes: limits.max_line_bytes,
            max_continuation_bytes: limits.max_continuation_bytes,
        })
    }

    /// Retrieve original-text continuations for a normalized line prefix.
    ///
    /// `active_line` is excluded from support so an old copy of the line being
    /// edited cannot suggest its own hidden suffix.
    pub fn query(
        &self,
        prefix: &str,
        active_line: Option<usize>,
        max_results: usize,
    ) -> Vec<RetrievalMatch> {
        if max_results == 0 || prefix.len() > self.max_query_prefix_bytes {
            return Vec::new();
        }
        let (normalized_prefix, _) = normalize(prefix.trim_start());
        if normalized_prefix.trim().is_empty() {
            return Vec::new();
        }

        let start = self
            .patterns
            .partition_point(|pattern| pattern.normalized.as_ref() < normalized_prefix.as_str());
        let mut continuations = BTreeMap::<String, Vec<usize>>::new();
        for pattern in &self.patterns[start..] {
            if !pattern.normalized.starts_with(&normalized_prefix) {
                break;
            }
            // Strip comments while the entire authored line is still
            // available. Stripping only the continuation would lose quote
            // state when the prefix ends inside a string.
            let code = strip_trailing_comment(&pattern.original);
            let Some(continuation) = continuation_after(code, &normalized_prefix) else {
                continue;
            };
            if continuation.is_empty() || continuation.len() > self.max_continuation_bytes {
                continue;
            }
            let supporting_lines = pattern
                .line_indices
                .iter()
                .copied()
                .filter(|line| Some(*line) != active_line);
            continuations
                .entry(continuation.to_owned())
                .or_default()
                .extend(supporting_lines);
        }

        let mut matches = continuations
            .into_iter()
            .filter_map(|(continuation, mut line_indices)| {
                if line_indices.is_empty() {
                    return None;
                }
                line_indices.sort_unstable();
                line_indices.dedup();
                Some(RetrievalMatch {
                    continuation,
                    line_indices,
                })
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| {
            right
                .support()
                .cmp(&left.support())
                .then(left.continuation.cmp(&right.continuation))
        });
        matches.truncate(max_results);
        matches
    }

    pub fn source_bytes(&self) -> usize {
        self.source_bytes
    }

    pub fn approximate_bytes(&self) -> usize {
        self.approximate_bytes
    }

    pub fn indexed_lines(&self) -> usize {
        self.indexed_lines
    }

    pub fn skipped_lines(&self) -> usize {
        self.skipped_lines
    }

    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = FilePatternEntry<'_>> {
        self.patterns.iter().map(|pattern| FilePatternEntry {
            normalized: &pattern.normalized,
            original: &pattern.original,
            line_indices: &pattern.line_indices,
        })
    }

    pub(crate) fn prefix_start(&self, normalized_prefix: &str) -> usize {
        self.patterns
            .partition_point(|pattern| pattern.normalized.as_ref() < normalized_prefix)
    }

    pub(crate) fn max_continuation_bytes(&self) -> usize {
        self.max_continuation_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_original_continuations_and_ranks_support() {
        let source = concat!(
            "  love.graphics.setColor(1, 1, 1, 1)\n",
            "\tlove.graphics.setColor(1, 1, 1, 1)\n",
            "love.graphics.setColor(0, 0, 0, 1)\n",
        );
        let table = FilePatternTable::build(source, RetrievalLimits::default()).unwrap();
        let matches = table.query("love.graphics.setColor(", None, 8);
        assert_eq!(matches[0].continuation, "1, 1, 1, 1)");
        assert_eq!(matches[0].line_indices, vec![0, 1]);
        assert_eq!(matches[1].continuation, "0, 0, 0, 1)");
    }

    #[test]
    fn excludes_the_active_line_without_losing_other_support() {
        let source = "draw(player)\ndraw(player)\ndraw(pla";
        let table = FilePatternTable::build(source, RetrievalLimits::default()).unwrap();
        let matches = table.query("draw(pla", Some(0), 8);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].continuation, "yer)");
        assert_eq!(matches[0].line_indices, vec![1]);

        assert!(table
            .query("draw(pla", Some(2), 8)
            .iter()
            .all(|hit| { !hit.line_indices.contains(&2) }));
    }

    #[test]
    fn exact_keys_preserve_distinct_formatting() {
        let source = "print(\"hello\")\nprint( 'hello' )\n";
        let table = FilePatternTable::build(source, RetrievalLimits::default()).unwrap();
        let matches = table.query("print(", None, 8);
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().any(|hit| hit.continuation == "\"hello\")"));
        assert!(matches.iter().any(|hit| hit.continuation == " 'hello' )"));
    }

    #[test]
    fn strips_comments_without_mistaking_string_contents_for_comments() {
        let source = "print(value) -- explanation\nprint(\"a -- b\") -- explanation\n";
        let table = FilePatternTable::build(source, RetrievalLimits::default()).unwrap();
        assert_eq!(table.query("print(v", None, 8)[0].continuation, "alue)");
        assert_eq!(
            table.query("print(\"", None, 8)[0].continuation,
            "a -- b\")"
        );
    }

    #[test]
    fn handles_unicode_prefixes_on_character_boundaries() {
        let source = "print(éclair)\nprint(élan)\n";
        let table = FilePatternTable::build(source, RetrievalLimits::default()).unwrap();
        let matches = table.query("print(é", None, 8);
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().any(|hit| hit.continuation == "clair)"));
        assert!(matches.iter().any(|hit| hit.continuation == "lan)"));
    }

    #[test]
    fn enforces_source_line_continuation_and_retained_byte_caps() {
        let source_limit = RetrievalLimits {
            max_source_bytes: 3,
            ..RetrievalLimits::default()
        };
        assert_eq!(
            FilePatternTable::build("four", source_limit).unwrap_err(),
            RetrievalBuildError::SourceTooLarge
        );

        let line_limit = RetrievalLimits {
            max_line_bytes: 5,
            ..RetrievalLimits::default()
        };
        let table = FilePatternTable::build("long line\nx()", line_limit).unwrap();
        assert_eq!(table.indexed_lines(), 1);
        assert_eq!(table.skipped_lines(), 1);

        let continuation_limit = RetrievalLimits {
            max_continuation_bytes: 2,
            ..RetrievalLimits::default()
        };
        let table = FilePatternTable::build("print(long)", continuation_limit).unwrap();
        assert!(table.query("print(", None, 8).is_empty());

        let entry_bytes = size_of::<FilePatternTable>()
            + size_of::<LinePattern>()
            + "a()".len() * 2
            + size_of::<usize>();
        let table_limit = RetrievalLimits {
            max_table_bytes: entry_bytes,
            ..RetrievalLimits::default()
        };
        let table = FilePatternTable::build("a()\nb()", table_limit).unwrap();
        assert!(table.is_truncated());
        assert!(table.approximate_bytes() <= entry_bytes);
    }

    #[test]
    fn rejects_oversized_query_prefixes_before_normalizing_them() {
        let limits = RetrievalLimits {
            max_line_bytes: 8,
            ..RetrievalLimits::default()
        };
        let table = FilePatternTable::build("print(x)", limits).unwrap();
        assert!(table.query("123456789", None, 8).is_empty());
        assert!(!table.query("print(", None, 8).is_empty());
    }

    #[test]
    fn reports_explicit_retained_bytes() {
        let source = "alpha()\nalpha()\nbeta()";
        let table = FilePatternTable::build(source, RetrievalLimits::default()).unwrap();
        assert_eq!(table.source_bytes(), source.len());
        assert_eq!(table.indexed_lines(), 3);
        assert!(table.approximate_bytes() >= size_of::<FilePatternTable>());
        assert!(table.approximate_bytes() < source.len() * 8 + 1024);
    }
}
