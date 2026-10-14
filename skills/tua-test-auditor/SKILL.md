---
name: tua-test-auditor
description: Audit the effectiveness of Tua tests, fixtures, fuzzing, compatibility checks, CI gates, and regression coverage. Use to find untested language and toolchain behavior or design a risk-based test strategy, not merely to run existing checks.
---

# Tua Test Auditor

Measure whether the suite detects important failures, not just how many tests
exist.

## Build A Risk Map

Map user-visible contracts to their implementation and strongest test layer:

- parsing and recovery;
- checking and inference;
- lowering, emission, runtime behavior, and source maps;
- Lua/Tua modules and project-model invalidation;
- CLI filesystem behavior and configuration;
- LSP state, concurrency, cancellation, and protocol conversion;
- JSON-RPC API and MCP framing, methods, privacy gates, path confinement, and
  protocol compatibility;
- formatter, linter, watch, traceback, asset pipelines, generated metadata,
  model integration, examples, VS Code integration, supported platforms, and
  releases.

Derive the final inventory from `docs/cli.md`, language and configuration docs,
the generated config schema, VS Code `package.json` contributions, Make targets,
and workflow definitions. The list above is a risk guide, not a closed product
inventory.

Inspect workflow triggers and dependencies to establish which tests actually
gate trusted integration and releases. Tua intentionally avoids GitHub Actions
for open or untrusted pull requests; do not flag that policy by itself or
recommend executing untrusted PR content. Look for a safe trusted-branch,
merge-queue, pre-merge, or pre-release gate instead. Do not infer gating from
the presence of a CI file, Make target, or test command.

Look for high-risk code with only happy-path unit tests, duplicate assertions
that share the same blind spot, fixtures that never execute emitted Lua, and CI
jobs whose triggers leave trusted integration or releases uncovered.

## Choose Stronger Oracles

Recommend the smallest effective layer:

- focused regression tests for known bugs;
- table-driven boundary tests for semantic matrices;
- property tests for parser, span, and round-trip invariants;
- fuzz targets for hostile syntax, config, protocol, and path inputs;
- differential or runtime tests for emitted Lua behavior;
- golden tests only when stable structured output is the contract;
- integration or editor-host tests for cross-component behavior.

Do not recommend coverage percentage as a substitute for an oracle. Do not add
large snapshots that conceal meaningful changes. Identify flakiness, excessive
runtime, platform gaps, and tests that cannot fail for the intended regression.

## Output

For each gap include the escaped defect class, evidence, proposed oracle,
fixture location, expected cost, and CI placement. Rank by risk reduction and
avoid proposing a new framework when an existing harness can express the test.
Keep the initial plan bounded to the highest-leverage gaps.
