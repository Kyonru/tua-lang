//! Small, immutable cold-start corpus for offline retrieval experiments.

use std::mem::size_of;
use std::sync::OnceLock;

use super::retrieval::{FilePatternTable, RetrievalLimits};

const LUA_IDIOMS: &str = include_str!("corpus/lua_idioms.lua");
const LOVE_PATTERNS: &str = include_str!("corpus/love_patterns.tua");

/// Language of a bundled retrieval source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundledCorpusLanguage {
    Lua,
    Tua,
}

/// One immutable file in the bundled corpus.
#[derive(Debug)]
pub struct BundledCorpusFile {
    pub id: &'static str,
    pub language: BundledCorpusLanguage,
    pub table: FilePatternTable,
}

/// The separately owned cold-start corpus. It is never merged into a project
/// table, so project rebuilds cannot mutate or duplicate it.
#[derive(Debug)]
pub struct BundledPatternCorpus {
    files: [BundledCorpusFile; 2],
    approximate_bytes: usize,
}

impl BundledPatternCorpus {
    pub fn files(&self) -> &[BundledCorpusFile] {
        &self.files
    }

    pub fn approximate_bytes(&self) -> usize {
        self.approximate_bytes
    }
}

/// Build the hand-written corpus once and return its frozen tables.
pub fn bundled_corpus() -> &'static BundledPatternCorpus {
    static CORPUS: OnceLock<BundledPatternCorpus> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let limits = RetrievalLimits::default();
        let lua = FilePatternTable::build(LUA_IDIOMS, limits)
            .expect("bundled Lua inline-completion corpus must fit its limits");
        let love = FilePatternTable::build(LOVE_PATTERNS, limits)
            .expect("bundled LOVE inline-completion corpus must fit its limits");
        // The outer value embeds both table headers. Replace those embedded
        // header bytes with each table's full retained-byte accounting.
        let approximate_bytes = size_of::<BundledPatternCorpus>()
            - 2 * size_of::<FilePatternTable>()
            + lua.approximate_bytes()
            + love.approximate_bytes();
        BundledPatternCorpus {
            files: [
                BundledCorpusFile {
                    id: "bundled/lua_idioms.lua",
                    language: BundledCorpusLanguage::Lua,
                    table: lua,
                },
                BundledCorpusFile {
                    id: "bundled/love_patterns.tua",
                    language: BundledCorpusLanguage::Tua,
                    table: love,
                },
            ],
            approximate_bytes,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_is_frozen_separate_and_retrievable() {
        let first = bundled_corpus();
        let second = bundled_corpus();
        assert!(std::ptr::eq(first, second));
        assert_eq!(first.files().len(), 2);
        assert!(first.approximate_bytes() > 0);

        let lua = &first.files()[0];
        assert_eq!(lua.language, BundledCorpusLanguage::Lua);
        assert!(!lua
            .table
            .query("for _, item in ipairs(", None, 4)
            .is_empty());

        let love = &first.files()[1];
        assert_eq!(love.language, BundledCorpusLanguage::Tua);
        assert!(!love
            .table
            .query("love.graphics.setColor(", None, 4)
            .is_empty());
    }

    #[test]
    fn bundled_sources_remain_parseable_examples() {
        crate::parse_lua_source("bundled/lua_idioms.lua", LUA_IDIOMS)
            .expect("bundled Lua idioms should parse as Lua");
        crate::parse_tua_source("bundled/love_patterns.tua", LOVE_PATTERNS)
            .expect("bundled LOVE patterns should parse as Tua");
    }
}
