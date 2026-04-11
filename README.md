# kb — repo knowledge base tool

`kb` is a local CLI that builds and enforces a deterministic, typed “knowledge base” for a git repository so LLM agents can navigate and recall context across sessions without dumping large files or running wide searches.

It targets the two churn classes in `docs/MISSION.md`:
- **Relevance churn**: reading big free-text docs/files to find a small fact.
- **IO churn**: many small tool calls just to discover repo layout and entrypoints.

## What you get

- A generated-first repo index under `kb/gen/*` (tree, symbols, deps; optional xrefs).
- Thin, human-authored overlays (`kb/atlas/modules/*.toml`, `kb/facts/facts.jsonl`, `kb/config/*`).
- Session capsules for “session-only truth” (`kb/sessions/YYYY/MM/<id>.json`).
- Commit-gated freshness: pre-commit + CI run `kb index check`, `kb lint all`, and `kb obligations check`.

Non-negotiables: no free-text/NLP inputs, no fuzzy search, typed selectors only, deterministic outputs, local + repo-bounded.

## Quickstart (this repo)

Prereqs: `cargo`, `git`, `ctags` (Universal Ctags recommended).

```bash
cargo install --path .
kb --format text version
kb index regen --scope all --diff-source worktree --format text
kb describe path --path . --depth 2 --include dirs,files,top-symbols --format text
kb plan diff --diff-source worktree --format text
kb pack diff --diff-source worktree --radius 1 --max-bytes 120000 --snippet-lines 80 --format text
```

Run the canonical gate locally:

```bash
scripts/kb-gate.sh worktree
```

## Onboard a target repo

This repo ships an onboarding skill that installs a committed `kb/` directory plus `kb/tooling/*` wrappers into a target repository:

```bash
skills/agents-repo-knowledge-base-skill/scripts/kb_onboard_repo.sh --repo <TARGET_REPO_PATH> --install-ci true
```

To avoid downloads, pass an existing binary:

```bash
skills/agents-repo-knowledge-base-skill/scripts/kb_onboard_repo.sh --repo <TARGET_REPO_PATH> --kb-bin "$(command -v kb)"
```

## Docs

- `docs/MISSION.md` — background + hard requirements.
- `docs/SPECS.md` — normative determinism specs (IDs, formats, diff-source semantics, budgets).
- `docs/DESIGN.md` — design overview + integration patterns.
- `docs/ENFORCEMENT.md` — pre-commit/CI gate sequence and scripts.

## Development

```bash
cargo test -q
```
