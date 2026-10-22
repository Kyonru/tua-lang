---
name: tua-review
description: Review Tua code, docs, examples, LSP, CLI, compiler, or roadmap changes for bugs, regressions, missing tests, architectural drift, stale docs, and one-off fixes that should use shared Lua/LOVE semantic infrastructure. Use when the user asks for review or before merging substantial Tua changes.
---

# Tua Review

Review for concrete risk first. Summaries come after findings.

## Review Priorities

- Behavior regressions in parser, checker, emitter, CLI, LSP, examples, or VS
  Code extension.
- One-off fixes that should be shared through semantic resolver, project model,
  or analysis snapshot.
- Lost Lua defaults: missing Lua completions, `.lua` module inference, LuaDoc
  docs, standard library behavior, or permissive dynamic Lua.
- LOVE regressions: object method signatures, colon guidance, asset helpers,
  constructor propagation, path validation, or examples.
- Stale docs, roadmap numbers, nav entries, and examples.
- Missing tests for high-risk changes.

## Review Method

- Inspect staged, unstaged, and untracked work plus the relevant surrounding
  code. Do not rely on the author's summary as evidence.
- Prefer exact file/line findings.
- Separate actual defects from questions and follow-up suggestions.
- Challenge broad changes that do not pay for their complexity.
- Check whether a failing surface indicates a general resolver problem.
- Reproduce important claims with the narrowest useful test, then broaden
  verification in proportion to the blast radius.
- Validate test harness inputs before blaming product behavior. Report harness
  defects separately.
- In a review-only request, do not edit, stage, commit, or rewrite the work.

## Expected Output

Lead with findings ordered by severity. If there are no findings, say that
clearly and mention residual risk or test gaps. Keep the review concise and
actionable.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
