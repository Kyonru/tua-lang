---
name: tua-minimal-change
description: Keep Tua changes focused, maintainable, and proportional. Use before editing Tua code, examples, docs, roadmap, tests, or extension files when the request could sprawl, when a local fix may hide a systemic problem, or when user ideas need to be challenged and shaped into a better small step.
---

# Tua Minimal Change

Make the smallest change that genuinely solves the right problem.

## Before Editing

- Read nearby code and tests before deciding the shape of the fix.
- Identify whether the request is:
  - a single bug;
  - a missing shared abstraction;
  - stale documentation;
  - a roadmap/design question.
- Check for existing patterns before adding new modules or helpers.
- Keep unrelated dirty worktree changes untouched.

## Challenge The Request When Needed

Push back gently when:

- the requested feature is too broad for one commit;
- a one-off patch would duplicate semantic logic;
- the idea conflicts with LuaJIT/LOVE compatibility;
- docs or examples would promise behavior not implemented;
- plugin/runtime ideas would make Tua less predictable.

Offer a smaller commit-sized alternative.

## Scope Discipline

- Edit only files needed for the task.
- Prefer tests that cover the behavior over broad rewrites.
- Do not refactor while fixing unless the refactor is the fix.
- Update docs only when behavior or user-facing workflow changes.
- Run focused verification first; broaden when the blast radius crosses crates.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.
