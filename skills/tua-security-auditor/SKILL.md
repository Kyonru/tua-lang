---
name: tua-security-auditor
description: Threat-model and audit Tua's compiler, CLI, API/MCP servers, LSP, VS Code extension, project loading, filesystem output, generated Lua, assets, configuration, dependencies, and release automation. Use for explicit security or adversarial robustness audits, not general code review.
---

# Tua Security Auditor

Audit Tua as a developer tool processing an untrusted project and untrusted
editor input. Avoid generic checklists; trace concrete data to privileged
effects.

## Trust Boundaries

- project source, config, annotations, asset paths, module names, and symlinks;
- LSP, newline-delimited JSON-RPC API, and MCP messages; open-document
  contents, overlays, workspace roots, framing, capabilities, privacy gates,
  and cancellation;
- VS Code workspace trust, configured executable provenance, spawned commands,
  webview messages and URIs, filesystem changes, and local-model data flow;
- build and clean destinations, generated filenames, source maps, and copied
  assets;
- emitted Lua and any compiler-generated helper or injected text;
- subprocesses, environment variables, downloaded artifacts, archives, and
  release credentials;
- dependency and CI permissions, version pinning, checksums, and provenance.

Prioritize arbitrary file read/write/delete, path escape and symlink behavior,
command or code injection, unsafe generated Lua, archive traversal, secret
exposure, denial of service from hostile source, and panic paths reachable from
normal CLI, API/MCP, LSP, or extension input.

Check explicit ceilings and backpressure for protocol frames, request queues,
source nesting, decoded media, captured subprocess output, and long-running
children. Check numeric conversions and arithmetic at protocol boundaries.

## Method

- Define attacker capability and security property before naming a finding.
- Follow validation through canonicalization and the final filesystem,
  process, or emission sink. A string check alone is not proof of safety.
- Confirm reachability with a safe minimal reproduction when practical.
- Separate exploitable findings, defense-in-depth improvements, robustness
  bugs, intentional trusted-configuration behavior, and dependency hygiene.
- Preserve Tua's policy of not running GitHub Actions for open or untrusted pull
  requests. Evaluate trusted integration and release gates instead, and treat
  any proposal involving `pull_request_target`, secrets, or execution of PR
  content as a separate high-risk design.
- Do not access secrets, attack external services, or mutate a live release.

For each finding provide severity, preconditions, impact, exact evidence,
reproduction or reasoning, a bounded remediation, and a regression test.
State residual uncertainty when platform-specific path or process behavior was
not exercised.
