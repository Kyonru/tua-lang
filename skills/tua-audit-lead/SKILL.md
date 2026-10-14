---
name: tua-audit-lead
description: Coordinate repository-wide Tua audits and turn findings from language correctness, security, tests, documentation, architecture, and product behavior into one evidence-backed improvement backlog. Use for broad health checks and audit planning, not ordinary diff review.
---

# Tua Audit Lead

Produce a trustworthy map of risk, not a large undifferentiated issue list.

## Audit Shape

- Establish the audit scope and current repository state before judging it.
- Read `README.md`, `ROADMAP.md`, `docs/specification.md`, workspace manifests,
  CI configuration, and the architecture documentation relevant to the scope.
- Verify which checks gate trusted integration and releases; the existence of a
  Make target or test suite is not evidence that it gates either path. Tua
  intentionally avoids GitHub Actions for open or untrusted pull requests, so
  do not report that absence alone as a defect. Look for a safe trusted-branch,
  merge-queue, pre-merge, or pre-release gate instead.
- When the user requests multiple agents and delegation is available, assign
  independent language, security, testing, and documentation lanes. Keep lanes
  read-only unless fixes were explicitly requested.
- For a full audit, also assess architectural coherence and product usability;
  the lead owns those cross-cutting lenses when no specialist is assigned.
- Use the corresponding Tua specialist skill for each lane. Use `tua-review`
  instead when reviewing a particular change or diff.
- Deduplicate symptoms that share one root cause. Prefer a shared semantic or
  architectural fix over many surface-specific tickets.

## Evidence Standard

Every finding must include:

- severity and affected user behavior;
- exact file and line evidence, or reproducible search, command, or
  configuration-absence evidence when the defect is something missing;
- why existing tests or safeguards do not cover it;
- the smallest credible remediation;
- a verification method.

Separate confirmed defects, credible risks, coverage gaps, and product ideas.
Do not present an unexecuted test, speculative exploit, or inferred docs drift
as confirmed fact.

## Synthesis

Rank work by impact, likelihood, reach, and fix leverage. Produce a short
ordered backlog with clear ownership boundaries and dependencies. Call out
cross-cutting causes, quick wins, and items that need a design decision.

Do not fix findings during an audit unless the user also asked for fixes. If
fixes are requested, take one coherent, testable slice at a time and use
`tua-minimal-change` plus `tua-verification`.
