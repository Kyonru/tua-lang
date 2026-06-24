---
name: tua-skill-maintainer
description: Maintain and update the repo-local Tua Codex skills. Use when skills are stale, too broad, missing new project standards, need renaming, need metadata regeneration, fail validation, or when a Tua workflow change should update one or more skills so future agents keep following current project practice.
---

# Tua Skill Maintainer

Keep the project skills useful, current, and concise.

## When To Update Skills

Update a skill when:

- project architecture or workflow changes make its guidance stale;
- a recurring mistake reveals missing guidance;
- a skill is too broad and should split into narrower skills;
- a skill is too narrow and misses an obvious trigger;
- `agents/openai.yaml` no longer matches `SKILL.md`;
- validation fails;
- docs, roadmap, examples, or tooling standards change in a way future agents
  should remember.

## Update Rules

- Read the affected `SKILL.md` before editing it.
- Keep each skill focused on one reusable behavior.
- Put trigger conditions in the frontmatter `description`.
- Keep the body procedural and concise.
- Do not add README or auxiliary docs inside skill folders.
- Regenerate `agents/openai.yaml` after renaming a skill or materially changing
  its purpose.
- Validate every touched skill with `quick_validate.py`.

## Commands

Use the repo venv because it has the validator dependencies:

```sh
.venv/docs/bin/python /Users/kyonru/.codex/skills/.system/skill-creator/scripts/quick_validate.py skills/<skill-name>
```

Regenerate metadata:

```sh
.venv/docs/bin/python /Users/kyonru/.codex/skills/.system/skill-creator/scripts/generate_openai_yaml.py skills/<skill-name> \
  --interface display_name="..." \
  --interface short_description="..." \
  --interface default_prompt="..."
```

Validate all repo-local skills:

```sh
for skill in skills/tua-*; do
  .venv/docs/bin/python /Users/kyonru/.codex/skills/.system/skill-creator/scripts/quick_validate.py "$skill" || exit 1
done
```

## Self-Maintenance Rule

When using any Tua skill, if the skill itself is stale, unclear, or missing a
standard needed for the task, update the skill as part of the same work when the
change is safe and scoped. If updating it would distract from the user request,
note the needed skill update explicitly.
