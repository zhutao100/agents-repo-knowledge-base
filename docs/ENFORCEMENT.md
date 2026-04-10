# Enforcement (pre-commit + CI)

The kb tool is designed to be **commit-gated**: generated KB artifacts and required overlays must be updated in the same change.

## Canonical gate sequence

Run these checks in order:

1. `kb index check --diff-source staged`
2. `kb lint all`
3. `kb obligations check --diff-source staged`

For CI, use `--diff-source worktree` (or `commit:<sha>` in a commit-scoped model).

## Provided scripts

This repo ships a single-source-of-truth gate runner:

- `scripts/kb-gate.sh <diff-source>`

And thin wrappers for common entrypoints:

- `scripts/hook-pre-commit.sh`
- `scripts/ci-check.sh`

`scripts/hook-pre-commit.sh` also performs a mechanical self-heal: if `kb/gen/*` is stale for the staged set, it regenerates and auto-stages those artifacts before running the canonical gate.
