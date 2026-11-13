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
| Provenance on local machine | Blocked by `gh` 2.52.0: `gh attestation verify` cannot load the current trusted root (`unsupported tlog public key type: PKIX_ED25519`). Its older CLI also lacks `--source-ref`, removed from the scaffold. A GitHub-hosted run must exercise the final command. |
| Font unit regressions | `node --test editors/vscode/test/unit/font-preview.test.js editors/vscode/test/unit/asset-explorer.test.js`: 7 passed, including repeated-open tab reuse. |
| Installed real-server host | Passed with valid bundled OTF: rendered sample at 18 px, glyphs present. Existing generic, protocol, hover, completion, navigation, diagnostic, and formatting assertions passed. An initial placeholder TTF fixture was corrected. An initial iframe screenshot attempt failed (`Command can only be executed on top-level targets`); a top-level capture succeeded and the dark, narrow-pane view was visually inspected. |
| Large-workspace baseline | Released 0.8 server: 1,000 modules/14,001 lines; cold completion 2,920.51 ms, warm 9.15 ms, sampled peak RSS 1,437.39 MiB. The one-round profile exited nonzero because it checked no valid-document contention completion. |
| `make ci-check` | First run passed formatting and strict Clippy, then failed because the sandbox denied `~/.cargo/advisory-db..lock`; the same gate is being rerun with access to that cache. |
| Local 0.9.0 VSIX | Pending final source edit and packaging. |

The representative trace emitted 43,138 parser-model spans across multiple
requests and sessions. Entry-file export indexing had 18 recorded spans
totaling about 11.1 seconds, with a maximum single span of 788 ms. Snapshot
construction had 11 spans totaling about 9.8 seconds, with a maximum of 1.62
seconds. The totals are nested and overlapping durations; do not add them to
request latency. The trace narrows the next investigation to repeated project
entry-file work but does not prove its cause.
