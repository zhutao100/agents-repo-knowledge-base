# agents-repo-knowledge-base-skill

This repository is a **Codex CLI / agent tool skill** that bootstraps the `kb` (repo knowledge base) workflow into an existing Git repository.

It is designed for **agentic development**: deterministic commands, typed parameters, and low IO churn.

## What it installs into a target repo

- A **local `kb` binary** (downloaded from the latest `kb-tool` GitHub release unless you provide one).
- A committed `kb/` artifact root:
  - `kb/config/obligations.toml` (required)
  - `kb/templates/session.json` (optional but recommended)
  - `kb/gen/*` (generated; committed)
- Gate scripts:
  - `kb/tooling/kb-gate.sh`
  - `kb/tooling/kb-pre-commit.sh` (auto-regens + auto-stages `kb/gen/*`)
  - `kb/tooling/kb-ci-check.sh`
  - `kb/tooling/install_kb.sh`
- A `pre-commit` hook wrapper that runs the canonical gate sequence.
- A `kb/AGENTS_kb.md` recipe + `AGENTS.md` snippet that teaches agents to use `kb` first.

## Quickstart

From this repo:

```bash
agents-repo-knowledge-base-skill/scripts/kb_onboard_repo.sh --repo /path/to/target-repo
```

To also install a CI workflow:

```bash
agents-repo-knowledge-base-skill/scripts/kb_onboard_repo.sh --repo /path/to/target-repo --install-ci true
```

Then, in the target repo, commit the new `kb/` artifacts and scripts.

## Notes

- The `kb` tool is local and repo-bounded (no network calls).
- Symbol indexing requires **Universal Ctags** (`ctags`) available in `PATH`.
