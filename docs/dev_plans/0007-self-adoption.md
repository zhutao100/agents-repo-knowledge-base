# DP-0007 — Self adoption: dogfood `kb` inside `kb-tool`

## Summary

Make the `kb-tool` repository itself a first-class **target repo** of `kb` (“dogfood”):

* commit a `kb/` directory in this repo containing the same **typed, deterministic** artifacts/configs expected downstream, and
* run the same **commit-gated freshness** loop (index check → lint → obligations) on `kb-tool` itself.

This reduces “re-entry cost” for agents working on kb-tool by ensuring this repo has the same cheap navigation substrate (`kb/gen/*`) plus thin overlays (`kb/atlas/*`) and session capsules (`kb/sessions/*`) as any other kb-enabled repo.

---

## Goals (must be true at end of DP-0007)

* This repo contains a committed `kb/` artifact root that follows `docs/SPECS.md` (repo-bounded paths, deterministic formats, no timestamps).
* `kb/config/obligations.toml` exists for kb-tool and maps path prefixes → required updates deterministically (no heuristics).
* `kb/atlas/modules/*.toml` exists for kb-tool itself, using stable module IDs.
* `kb index regen --diff-source worktree` produces committed `kb/gen/*` for this repo, and `kb index check --diff-source worktree` passes.
* `scripts/hook-pre-commit.sh` and `scripts/ci-check.sh` can be run in this repo to enforce the canonical gate sequence on kb-tool itself.

---

## Non-goals

* No new user-facing CLI surface or artifact formats.
* No “automatic doc gardening” or free-text population of module cards/facts.
* No network calls.

---

## Inputs and prerequisites

This plan assumes DP-0001 through DP-0006 are complete:

* DP-0001: `kb` binary + deterministic IO + diff-source reader
* DP-0002: `kb index regen` / `kb index check`
* DP-0003: `kb plan diff` (obligation planning)
* DP-0004: `kb lint all` + `kb obligations check` + gate runner scripts
* DP-0005: module cards (`kb/atlas/modules/*.toml`) and typed discovery
* DP-0006: session capsules (`kb/sessions/**.json`)

---

## Decisions (locked)

### 1) kb-tool is both “tool repo” and “target repo”

For dogfooding, this repo MUST contain a target-repo `kb/` artifact root at the repo root, and it MUST be treated like any other kb-enabled repo for indexing and enforcement.

This implies:

* `kb/gen/*` is committed and regenerated deterministically.
* `kb/cache/` and `kb/.tmp/` are local-only derived directories and MUST be gitignored.

### 2) Stable module IDs for kb-tool itself

kb-tool MUST ship initial module cards with these module IDs:

* `kb.cli` — command graph + typed flags/enums + output contract
* `kb.io` — canonical JSON/JSONL writers, stable sorting, hashing helpers
* `kb.repo` — repo root detection, path normalization, diff-source reader
* `kb.index` — generators for `kb/gen/*` and `index check`
* `kb.plan_pack` — `plan diff` and `pack` planners + budgeting/drop policy
* `kb.enforcement` — lint + obligations evaluation + hook/CI runner semantics
* `kb.specs` — how `docs/SPECS.md` maps to schemas, versioning, “spec wins”

Each card MUST live at:

* `kb/atlas/modules/<MODULE_ID>.toml`

and MUST have `id` equal to the filename stem (DP-0005 rule).

### 3) Tag vocabulary is explicit

kb-tool MUST commit `kb/config/tags.toml` and keep it aligned with tags used in the kb-tool module cards.

v1 tag set (stable-sorted, locked):

* `cli`
* `enforcement`
* `index`
* `io`
* `pack`
* `plan`
* `repo`
* `specs`

### 4) Obligations policy for kb-tool (v1)

kb-tool MUST commit `kb/config/obligations.toml` with deterministic, prefix-based rules:

* code changes require the corresponding module card updated in the same diff,
* contract-sensitive changes also require a session capsule updated in the same diff.

Rules (v1, locked; `when_path_prefix` values are normalized repo-relative prefixes):

* `src/main.rs` and `src/cli/` ⇒ require `kb.cli` and a session capsule
* `src/io/` ⇒ require `kb.io`
* `src/repo/` ⇒ require `kb.repo`
* `src/index/` ⇒ require `kb.index`
* `src/query/` ⇒ require `kb.plan_pack`
* `src/policy/` and `scripts/` ⇒ require `kb.enforcement` and a session capsule
* `docs/SPECS.md` and `schemas/` ⇒ require `kb.specs` and a session capsule

No other heuristics are permitted in v1.

---

## Implementation steps

### 1) Add kb-tool’s target-repo skeleton (`kb/`)

Add (commit) the repo’s kb artifacts root:

* `kb/config/obligations.toml` (per Decisions §4)
* `kb/config/tags.toml` (per Decisions §3)
* `kb/atlas/modules/*.toml` module cards (per Decisions §2)

Also ensure:

* `kb/cache/` is gitignored
* `kb/.tmp/` is gitignored (used by `index check`)

### 2) Generate and commit `kb/gen/*` for kb-tool

In this repo:

1. Run `kb index regen --scope all --diff-source worktree`.
2. Confirm outputs are stable by running regen twice and verifying `git diff` is empty.
3. Commit `kb/gen/*`.

### 3) Turn on self-gating for kb-tool

Use the canonical gate sequence on this repo itself:

1. `kb index check --diff-source staged`
2. `kb lint all`
3. `kb obligations check --diff-source staged`

Required outcomes:

* the pre-commit runner script from DP-0004 executes this sequence for kb-tool, and
* the CI runner script from DP-0004 executes this sequence for kb-tool.

---

## Acceptance tests (required)

Run in this repo (worktree clean):

* `kb index check --diff-source worktree`
* `kb lint all`
* `kb plan diff --diff-source worktree --policy default --format json`

Obligations behavior (minimal check):

1. Make a small change under `src/index/` and stage it.
2. Run `kb plan diff --diff-source staged --format json` and confirm it requires module card `kb.index`.
3. Update `kb/atlas/modules/kb.index.toml`, stage it, and confirm `kb obligations check --diff-source staged` passes.
