# kb-tool — agent notes

## Read first (project intent)

- `docs/MISSION.md` — problem statement + hard requirements (relevance churn, IO churn, commit-gated freshness).
- `docs/DESIGN.md` — current design draft for the `kb` CLI, artifacts, and enforcement model.
- `docs/SPECS.md` — normative determinism specs (formats, IDs, diff-source semantics, budgets).
- `docs/dev_plans/INDEX.md` — indexed, decision-complete implementation plans.

## Non-negotiables (agent-facing interface)

- **No free-text/NLP parameters** in the `kb` interface (no `--task`, `--question`, semantic ranking, or prompt interpretation).
- **No fuzzy search** as a primary interface. Discovery is via explicit `list` operations, then exact selectors.
- **Typed parameters only**: repo-relative paths, stable IDs, validated tags, enums, and numeric budgets.
- **Deterministic outputs** for generated artifacts: stable ordering, no timestamps in generated content, diff-friendly formats.
- **Local + repo-bounded**: no network calls; reject path traversal outside repo root after normalization.

## Docs + design hygiene

- Treat Markdown as a **view/UI**, not the canonical store. Prefer schemas + structured records for “knowledge as data”.
- If you change the CLI contract, data model, or enforcement rules, update `docs/DESIGN.md` in the same change.
- Keep examples copy/pasteable and consistent with the “typed ops only” constraint.

## Verification + commits

- Use Conventional Commits (e.g., `docs: ...`, `feat: ...`, `fix: ...`) and avoid leaking local paths (use `$HOME`/`~`).

## kb workflow (agent recipe)

- Follow `AGNETS_addon.md` for the canonical typed `kb` inspection/update loop.
- Fast local checks:
  - `kb index check --diff-source worktree`
  - `kb plan diff --diff-source staged --policy default --format json`
  - `scripts/kb-gate.sh staged`
