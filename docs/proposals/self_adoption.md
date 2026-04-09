## What the repo already does (today)

In its current, doc-only state, **`AGENTS.md` is the session entrypoint** and the repo’s durable “memory” is the trio:

* `docs/MISSION.md` (why the tool exists; churn types; commit-gated freshness requirement)
* `docs/DESIGN.md` (intended end-state: generated index + typed retrieval + obligations + session capsules)
* `docs/SPECS.md` (normative determinism/format contracts; “spec wins”)

That is a workable baseline, but it still exhibits the same weakness the project is trying to solve: agents must re-read large free-text docs to regain a precise operational model.

The practical answer is: **dogfood the kb system inside kb-tool itself**.

---

## The core move: treat `kb-tool` as a “target repo” of `kb`

Once the `kb` CLI exists (DP-0001/0002 onward), the kb-tool repo should contain and enforce the same artifacts it expects in downstream repos.

### Add a committed `kb/` directory to *this* repo

```text
kb-tool/
  kb/
    config/
      obligations.toml
      tags.toml              # optional
      kb.toml                # optional
    gen/                     # committed, deterministically regenerated
      kb_meta.json
      tree.jsonl
      symbols.jsonl
      deps.jsonl
    atlas/
      modules/
        kb.cli.toml
        kb.index.toml
        kb.plan_pack.toml
        kb.enforcement.toml
        kb.specs.toml
    facts/
      facts.jsonl            # optional v1; grows later
    sessions/
      2026/04/<session>.json
    cache/                   # gitignored
```

This gives LLM agents the **same “cheap navigation memory”** (tree/symbols/deps + overlays) for the kb-tool codebase itself, across inference sessions.

---

## What “project context” should look like for kb-tool itself

You want to minimize what an agent must read at session start while keeping everything deterministic and commit-gated. For kb-tool, the most valuable durable context is:

### 1) Generated-first navigation substrate (no prose)

* `kb/gen/tree.jsonl`: stable repo map (paths, sizes, languages, top symbols)
* `kb/gen/symbols.jsonl`: stable symbol IDs + def sites
* `kb/gen/deps.jsonl`: deterministic import edges (even if best-effort)

**Why it matters for kb-tool specifically:** agents will frequently need to jump between CLI parsing, IO canonicalization, diff-source reading, indexers, pack planners, and enforcement. The generated index gives that map without exploratory “open 20 files” IO churn.

### 2) Thin “atlas” overlays (human intent, but structured)

Module cards should be the canonical place to encode *human-style navigation cues* that generation cannot infer:

* *Purpose* (“what this module is responsible for”)
* *Entrypoints* (paths/symbol IDs)
* *Edit points* (hotspots, invariants, common failure modes)
* *Verification hooks* (which tests/commands validate the module)

For kb-tool, a minimal module set that pays off quickly:

* `kb.cli` — command graph, typed flags/enums, output/error contract
* `kb.io` — canonical JSON/JSONL writers, stable sort helpers
* `kb.repo` — repo root detection, path normalization, diff-source reader
* `kb.index` — generators for tree/symbols/deps, plus `index check`
* `kb.plan_pack` — `plan diff` + pack budgeting/drop policy
* `kb.enforcement` — lint + obligations + hook/CI runner semantics
* `kb.specs` — how specs map to schemas, versioning rules, “spec wins”

### 3) Session capsules (structured “why” and “ruled-out options”)

For kb-tool itself, session capsules are particularly important because many changes are *contract-sensitive* (format, determinism, CLI surface). Capsules should capture:

* decision made (and why)
* alternatives explicitly ruled out
* backward-compat / migration notes (if any)
* verification performed (commands, outcomes)
* affected modules / paths / symbol IDs

This directly addresses the “off-session doc gardening cannot recover this” failure mode.

---

## Enforcement loop (so the repo stays a reliable memory)

The repo should apply its own enforcement model to itself:

### Pre-commit + CI gate for kb-tool repo

Run the canonical sequence (as described in the design/dev plans) against **this repo’s** `kb/`:

1. `kb index check --diff-source staged`
2. `kb lint all`
3. `kb obligations check --diff-source staged`

### A sensible `kb/config/obligations.toml` for kb-tool

Make obligations reflect what actually causes knowledge drift in this project:

* **CLI contract changes** ⇒ require a session capsule and an atlas module update
* **Spec/schema changes** ⇒ require updating the relevant module card (and any schema files if you add `schemas/`)
* **Index/planner/enforcement code changes** ⇒ require updating the corresponding module card (not necessarily editing prose docs)

Prefix-based, deterministic examples (illustrative):

* changes under `src/index/` → require `kb.index` module card
* changes under `src/query/` → require `kb.plan_pack` module card
* changes under `docs/SPECS.md` (or `schemas/`) → require `kb.specs` module card + session capsule

This keeps the repo’s “memory artifacts” aligned with code evolution without relying on best-effort doc gardening.

---

## Agent workflow inside kb-tool (how a new session should start)

A durable, low-churn start-of-session routine for agents working on kb-tool itself:

1. Read `AGENTS.md` (still the human TOC and hard constraints).

2. Use `kb pack selectors` (or an equivalent “start pack” convention) to fetch, in one call:

   * the atlas module cards for the area they’re working in,
   * the relevant slice of `kb/gen/*`,
   * and bounded definition snippets for a small number of key symbols.

3. After edits:

   * `kb plan diff --diff-source worktree`
   * update required module cards / facts / session capsule
   * `kb index regen …`
   * commit (gates enforce correctness)

This is the same loop you want downstream repos to follow, and it makes kb-tool itself progressively easier for agents to re-enter after context resets.

---

## Bootstrapping reality (because kb-tool starts doc-only)

Right now there is no `kb` binary in the repo, so you cannot immediately rely on self-generation. The clean bootstrap strategy is:

1. **Commit the `kb/config/` + `kb/atlas/modules/` skeleton early** (manual, small, stable).
2. Once DP-0001–DP-0004 land, **generate and commit `kb/gen/*` for this repo** and turn on the gates.
3. From that point forward, kb-tool becomes a living demonstration that the approach actually works: deterministic artifacts, typed retrieval, and commit-gated freshness—applied to itself.

---

## Concise reasoning narrative

I started from `AGENTS.md` and the repo’s mission/design/specs, and the key implication is that kb-tool must avoid the same “free-text re-reading” trap it is meant to solve. The straightforward solution is to **dogfood**: add a committed `kb/` directory to this repo, generate `kb/gen/*` deterministically for kb-tool itself, layer thin structured atlas modules plus session capsules for intent/rationale, and enforce freshness via the same pre-commit/CI gate sequence. This turns the repo into its own cross-session memory substrate for LLM agents.

---

## Validation and dev plan linkage

This proposal is consistent with the current `docs/DESIGN.md` / `docs/SPECS.md` constraints (typed selectors only, deterministic artifacts, repo-bounded behavior) and with the execution sequence described in DP-0001 through DP-0006.

Implementation is captured as DP-0007:

* `docs/dev_plans/0007-self-adoption.md`
