# Agent notes (agents-repo-knowledge-base-skill)

## Intent

This repo is a **skill package**. It exists to install and bootstrap the `kb` tool into *other* repositories.

## Primary operation

- Run: `agents-repo-knowledge-base-skill/scripts/kb_onboard_repo.sh --repo <PATH>`

This will:
- copy a minimal `kb/` skeleton into the target repo,
- install or locate a `kb` binary (from GitHub releases by default),
- generate `kb/gen/*`,
- install hook and gate scripts,
- patch target `AGENTS.md` with a `kb`-first navigation recipe.

## Editing guidance

- Prefer editing templates under `agents-repo-knowledge-base-skill/assets/templates/target_repo/`.
- Keep scripts **idempotent** and avoid repo-specific assumptions.
