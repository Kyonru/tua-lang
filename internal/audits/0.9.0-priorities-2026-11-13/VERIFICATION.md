# 0.9.0 candidate verification log

The candidate uses synchronized local version `0.9.0`; no tag, push, or
release was made. A passed check below was actually run. Pending and blocked
items are not counted as passed.

| Check | Outcome |
| --- | --- |
| Base state | Clean tracked tree at `d19ec214`, published `v0.8.0`; ignored build and earlier audit artifacts existed. |
| Workflow CLI regression | `cargo test -p tua-cli --test cli actions_init_adds_a_reviewable_workflow_without_overwriting_it -- --exact` passed before and after version synchronization. |
| Generated workflow syntax | Ruby YAML parsed the install step; `bash -n` passed. |
| Release-asset contract | The public `v0.8.0` Linux archive URL resolved. Its downloaded archive matched the `.sha256` file. The tar contains `tua-0.8.0-x86_64-unknown-linux-gnu/tua` and release metadata. |
| Provenance on local machine | Blocked by `gh` 2.52.0: `gh attestation verify` cannot load the current trusted root (`unsupported tlog public key type: PKIX_ED25519`). Its older CLI also lacks `--source-ref`, removed from the scaffold. A checksum-verified temporary `gh` 2.103.3 could parse the command but anonymous local verification required login. The generated job supplies `GH_TOKEN` and `attestations: read`; a GitHub-hosted run must exercise the final command. The [official Ubuntu runner image](https://github.com/actions/runner-images/blob/main/images/ubuntu/Ubuntu2404-Readme.md) lists `gh` 2.103.3 at this audit date. |
| Font unit regressions | `node --test editors/vscode/test/unit/font-preview.test.js editors/vscode/test/unit/asset-explorer.test.js`: 7 passed, including repeated-open tab reuse. |
| Installed real-server host | Passed with valid bundled OTF: rendered sample at 18 px, glyphs present. Existing generic, protocol, hover, completion, navigation, diagnostic, and formatting assertions passed. Repeated after installing the exact final 0.9.0 VSIX; `REAL_SERVER_EVIDENCE` reported `fontPreview: "sniglet.otf rendered at 18px"`, Tua/Lua/LOVE completions, and zero fixture diagnostics. An initial placeholder TTF fixture was corrected. An initial iframe screenshot attempt failed (`Command can only be executed on top-level targets`); a top-level capture succeeded and the dark, narrow-pane view was visually inspected. |
| Large-workspace baseline | Released 0.8 server: 1,000 modules/14,001 lines; cold completion 2,920.51 ms, warm 9.15 ms, sampled peak RSS 1,437.39 MiB. The one-round profile exited nonzero because it checked no valid-document contention completion. |
| Matched 0.9 performance comparison | Same 1,000-module fixture hash, release binaries, two cold and three warm requests, three contention rounds. Before semantic-model reuse: cold completion 3,022.47 ms, hover 3,097.54 ms, warm completion 9.76 ms, sampled peak RSS 2,607.69 MiB. After: 2,658.05 ms, 2,640.99 ms, 9.41 ms, and 2,685.80 MiB respectively. Both probes exited 0. These are protocol timings, not visible VS Code menu timings; sampled RSS is across different process/session moments. |
| Export inference regression | `CARGO_INCREMENTAL=0 cargo test -p tua-core many_imported_expressions_keep_distinct_export_types_after_an_edit --lib`: passed. It verifies 64 imported expressions preserve distinct types and an edited dependency updates its exported type. |
| `make ci-check` | **Passed** on final product source with `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0` and required host permissions. Included strict Clippy, allowed Rust advisories, 148 CLI integration tests, 1,949 core tests, 455 LSP tests, emitted Lua, examples, editor regression/smoke, extension checks, docs, and benchmark smoke. Earlier attempts failed for sandboxed advisory-db access, 116 MiB free disk, and one LSP timeout during concurrent test compilation. After clearing only disposable incremental cache, an exclusive default-job Clippy attempt slept without CPU; single-job Clippy passed, then the complete single-job gate exited 0. Log: `/tmp/tua-090-ci-check-final3.log`. |
| LÖVE runtime smoke | `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 TUA_LOVE_BIN=/Applications/love.app/Contents/MacOS/love make love-runtime-test`: **3 passed**, launching minimal, full, and LOVE/Tua examples. |
| npm dependency audit | `npm audit --prefix editors/vscode --audit-level=high --json` with temporary npm cache: passed, zero vulnerabilities at all severities. Initial sandboxed request could not resolve the registry. |
| Screenshot artifact check | `make extension-screenshots-check` passed for five existing 0.8.0 README captures. The new font view is absent from that gallery; a separate actual-host dark narrow-pane capture was visually inspected but remains in `/tmp`. |
| Local 0.9.0 VSIX | `make extension-package` passed with loopback access; the sandboxed attempt failed two Runtime Inspector tests because `listen 127.0.0.1` returned EPERM. `editors/vscode/tua-0.9.0.vsix` has 41 files, 8.93 MB, `unzip -t` passes, and extracted `extension/bin/tua --version` is `tua 0.9.0`. SHA-256: `4d94ef8851747d3adde40c5060c1af4e55363f2c86a36dc1f745074a3616d996`. The installed-VSIX host suite exited 0. No tag, push, or publication occurred. |

The representative trace emitted 43,138 parser-model spans across multiple
requests and sessions. Entry-file export indexing had 18 recorded spans
totaling about 11.1 seconds, with a maximum single span of 788 ms. Snapshot
construction had 11 spans totaling about 9.8 seconds, with a maximum of 1.62
seconds. The totals are nested and overlapping durations; do not add them to
request latency. The trace narrows the next investigation to repeated project
entry-file work but does not prove its cause.
