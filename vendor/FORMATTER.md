# Tua formatter dependencies

Tua maintains these local, unpublished forks:

| Directory | Upstream release | License |
| --- | --- | --- |
| `tua-full-moon` | full_moon 3.0.0, Kampfkarren/full-moon | MPL-2.0 |
| `tua-stylua` | StyLua 2.6.0, JohnnyMorganz/StyLua | MPL-2.0 |

The source baseline is the corresponding crates.io package. Original package
metadata and authors are retained in `Cargo.toml.orig`; local manifests rename
packages to keep the formatting AST separate from the compiler's full_moon AST.
Both directories include the MPL-2.0 license text. Tua modifications to these
files remain under that license. Keep the source of these forks available with
binary distributions; do not describe them as upstream releases or relicense
these files under the root MIT license.

Tua owns the dialect extensions and regression tests. Extensions retain authored
arrays, optional/default parameters, generic bounds, enums, protocols, classes,
type-only imports and runtime `is` tests. They must not lower or reconstruct
source. The compiler remains responsible for language acceptance and semantics.

For upgrades, compare against the pinned upstream source, retain license notices,
run upstream tests where available, and run Tua formatter, parser, emitter,
CLI and LSP suites. Review all AST, traversal, trivia and output-verification
changes. A dependency update must not silently expand the compiler's language.

The shared adapter is `crates/tua-core/src/formatter/stylua.rs`. Add regression
cases in the core formatter corpus, including second-pass stability and emitted
program checks. Range and on-type tests belong in core and LSP. The dialogue DSL
retains its own formatter.
