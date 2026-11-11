# Test-oracle and CI review

At baseline `c08edbca`, the trusted release workflow in `.github/workflows/release.yml` runs `.github/workflows/ci.yml` as a required `verify` dependency before packaging. Main-branch CI includes Rust workspace tests on Linux, macOS, and Windows; Linux LuaJIT/LOVE runtime checks; extension host and webview suites; examples, docs, dependency audits, and benchmark smoke. The performance-trend workflow has larger representative workloads but is not a release prerequisite. Open-pull-request CI is intentionally absent under the repository's trust policy; this audit does not count that alone as a defect.

| Escaped defect class | Previous oracle and gap | Stronger oracle added or needed |
| --- | --- | --- |
| Imported protocol bound overwritten in a generic class method | Existing CLI test at `crates/tua-cli/tests/cli.rs:1005` exercises an imported bound on a generic function; runtime test near line 1251 uses an unbounded generic class plus a separate bounded function. Neither combines an imported class bound with a second method in the declaring file. | New analysis test first failed on the minimized declaring/consumer module case, then passed after the shared type-context fix; negative nonconformer retained. |
| Signal emit through an imported nested bus field | Same-file SignalBus behavior passed; no assertion preserved `emit`'s varargs through nested field method typing across files. | New `accepts_signal_emit_payloads_through_nested_bus_fields` core regression failed with TL1004 before the fix and passed after it. |
| Cross-module class runtime type identity | Pending language-lane cross-file Lua runtime oracle. | Require unrelated classes in separate modules to fail `is`; include inheritance positives and output inspection. |
| Owned bundled helper refresh | Existing tests protect project-supplied helper overrides, but an old generated helper appears to survive a new build. | Pending CLI regression: build with a prior owned helper, change required version/content, rebuild, run exported feature; verify manual override remains untouched. |
| Populated preview information and source actions | Empty/shape tests cannot establish that a populated Signal, ECS, or pool view presents useful relationships. | Installed-host populated view-model fixtures are being added; keep source-navigation and refresh-state assertions. |

This table records observed gaps and planned checks, not a test-coverage percentage. Recheck each final test against its intended regression and confirm its owning suite is included in trusted CI.
