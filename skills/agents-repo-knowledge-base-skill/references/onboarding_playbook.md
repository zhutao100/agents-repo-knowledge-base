# Onboarding playbook (target repo)

This playbook expands the one-command onboarding into explicit, verifiable steps.

## 0) Preconditions

- You are in a Git worktree.
- `ctags` is installed (Universal Ctags recommended).
- You have either:
  - network access to download the latest `kb-tool` GitHub release, or
  - a local `kb` binary to provide via `--kb-bin`.

## 1) Install / locate `kb`

The onboarding script resolves `kb` in this order:

1. `<repo>/.kb-tool/bin/kb` if present (previous install).
2. `--kb-bin <PATH>` if provided (copied to `.kb-tool/bin/kb`).
3. Download from the latest GitHub release (or `--kb-tag vX.Y.Z`) into `.kb-tool/bin/kb`.

## 2) Create the `kb/` artifact root

The script ensures these directories exist:

- `kb/config/` (required)
- `kb/gen/` (generated; committed)
- `kb/templates/` (optional)
- `kb/atlas/modules/` (optional)
- `kb/facts/` (optional)
- `kb/sessions/` (optional)

It writes `kb/config/obligations.toml` (required). The default file is intentionally empty (comment-only) so it does not impose obligations before you map the repo.

## 3) Generate the initial deterministic index

Run in the target repo root:

- `kb index regen --scope all --diff-source worktree --format text`
- `kb index check --diff-source worktree --format text`
- `kb lint all --format text`

This yields committed artifacts under `kb/gen/*`.

## 4) Enable commit-gated freshness

The gate sequence is:

1. `kb index check --diff-source staged`
2. `kb lint all`
3. `kb obligations check --diff-source staged`

The script installs:

- `kb/tooling/install_kb.sh` (downloads the `kb` binary from releases)
- `kb/tooling/kb-gate.sh` (single-source-of-truth for the above)
- `kb/tooling/kb-pre-commit.sh` (mechanically regenerates + auto-stages `kb/gen/*`, then runs the gate on `staged`)
- `kb/tooling/kb-ci-check.sh` (runs the gate on `worktree`)

It then installs a `pre-commit` hook wrapper that executes the gate.
If the repo already has a `.pre-commit-config.yaml`, the script instead inserts a local `kb-gate` hook into that config (so `prek`/`pre-commit` runs the gate).

### CI integration (optional)

If you onboard with `--install-ci true`, the script installs `.github/workflows/kb-ci.yml` so CI downloads the latest `kb` release and runs the same gate.

## 5) Patch `AGENTS.md`

A marker-bounded snippet is inserted so agents prefer `kb` operations before wide scans.

## 6) What to commit

From `git status`, commit the KB artifacts and scripts (including `kb/AGENTS_kb.md`). The script also adds ignore rules for local-only paths.

If you enabled CI, also commit `.github/workflows/kb-ci.yml`.
