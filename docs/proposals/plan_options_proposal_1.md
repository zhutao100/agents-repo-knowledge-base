## What the harness-engineering pattern solves—and what it leaves unsolved

The “repository knowledge as system of record” pattern makes two strong moves:

1. **`AGENTS.md` becomes a short map / table of contents**, not an encyclopedia.
2. **`docs/` becomes the versioned knowledge base**, with mechanical validation (linters/CI) and a background “doc-gardening” loop to reduce drift.

That addresses the “one giant manual rots / can’t be verified / crowds out context” failure mode.

However, the approach still leaves your two hardest gaps largely intact:

### Gap A: “b) and c)” navigation is not encoded as *cheap, queryable memory*

Humans navigate via an internal **hierarchical index** (“this dir is X; those files are Y; open Z”), not by reading prose. The harness pattern provides *conceptual* progressive disclosure, but it does **not** inherently provide a *deterministic directory/file index* that can be queried under a tight token/tool budget. So agents still fall back to:

* expensive grep-and-open loops (relevance churn), or
* under-sampling and missing the right locus (accuracy risk).

### Gap B: Markdown is not a good “selective read / selective write” substrate

Even if `docs/` is well curated, **free-text** forces full-doc reads to avoid missing the one needed sentence, and makes correctness-preserving updates hard to mechanize.

### Gap C: Off-session doc gardening cannot capture “session-only” truth

The article’s doc-gardening agent helps find stale docs , but it’s structurally disadvantaged versus in-session updates: the most valuable artifacts (ruled-out hypotheses, why-not-X, pitfalls, experiment outputs) are discovered *while doing the work*, not while scanning later.

---

## Design target: a knowledge base that is **queryable, structured, low-churn, and commit-gated**

To close “b/c navigation” and markdown drift, you need a second layer under `docs/`:

* **Machine-readable repository memory** (index/graph/records), optimized for selective retrieval and deterministic checks.
* **Human-readable views** (markdown) generated from that memory where useful—not the other way around.

Think of it like: **“docs-as-UI; knowledge-as-data.”**

---

## Option 1 (recommended): Generated Repo Graph + a single “KB query” tool call

### Core idea

Maintain a **generated, structured repo index** (tree + symbols + dependency edges + doc obligations) and expose it through a **single local command** (or MCP tool) that can answer:

* “What’s in this directory?”
* “Where is symbol X defined and used?”
* “What are the entry points for feature Y?”
* “Given this git diff, what knowledge records must be updated?”

This directly attacks:

* **relevance churn**: no more dumping whole docs; query returns only the minimal slice.
* **IO churn**: agent makes *one* tool call to the KB tool, not 10–50 filesystem calls.

### What to store (deterministic, diffable)

Commit generated artifacts under something like `docs/kb/generated/`:

* `repo_tree.json`

  * nodes: directory/file, size, language, short purpose label, top symbols
* `symbols.jsonl` (or sqlite + exported jsonl)

  * symbol → definition location, signature, kind, visibility
* `deps.json`

  * import/module dependency edges; optionally “layer”/domain boundaries
* `topics.jsonl` (optional)

  * stable “concept nodes” (Auth, Telemetry, Billing) and links to code/doc nodes
* `obligations.json`

  * mapping from paths/modules → required KB shards / docs that must be touched when changed

### How agents use it (one call)

A single command, e.g.:

* `kb describe path src/foo --budget-tokens 1200`
* `kb describe symbol PaymentOrchestrator --include deps --budget-tokens 1600`
* `kb plan-from-diff --budget-tokens 1800` (reads `git diff` locally, emits required loci + minimal excerpts)

### Commit-time enforcement (pre-commit + CI)

* **Pre-commit** runs `kb regen --changed-only` and fails if:

  * generated KB outputs differ from what’s staged
  * obligations triggered by the diff are not satisfied (e.g., module manifest not updated)
* **CI** repeats the check to prevent bypass.

This satisfies your “must enforce in-session knowledge updates” requirement because the *only* time code can land is when the knowledge artifacts for the touched surface area have been regenerated/updated.

### Why this works in practice

* Most navigation memory becomes **generated**, so it stays current with low human/agent effort.
* The agent no longer needs to “remember” the repo; it queries a repo-native memory substrate.

---

## Option 2: Co-located Module Manifests (“knowledge shards”), not global markdown

### Core idea

Put a small, structured manifest next to each module/package/directory (e.g., `module.kb.yaml`). This shard contains:

* purpose / responsibilities
* key entrypoints
* invariants / contracts
* “owned docs” pointers
* API surface summary

Then generate a repo-wide index from these shards.

### Why it helps

* Updates are **localized** (no giant doc edits).
* You can enforce: “if any file under `packages/auth/**` changes, `packages/auth/module.kb.yaml` must be touched or auto-regenerated.”

### Tradeoffs

* Higher up-front authoring burden than Option 1 (unless you auto-seed + keep fields minimal).
* Still best paired with a single query tool to avoid IO churn from many shards.

---

## Option 3: Docs with **verifiable assertions** (make prose mechanically checkable)

### Core idea

Keep markdown, but embed **structured “assertion blocks”** that a linter can verify against code/index:

* “This endpoint exists”
* “This config key is supported”
* “This module may only depend on X/Y layers”
* “This directory owns these CLI commands”

If a doc claims something that’s no longer true, CI fails.

### Why it helps

This attacks the “it rots instantly” problem the article calls out  by making *rot observable* and *blocking*.

### Tradeoffs

* Doesn’t fully solve navigation “b/c” unless paired with Option 1/2.
* Requires building/maintaining the assertion linter.

---

## Option 4: Session-captured decision logs as structured records (fix off-session blindness)

### Core idea

Add a **structured, append-only worklog** per task/PR, designed for selective retrieval:

* `docs/worklog/<pr-or-ticket>.jsonl`

  * entries: timestamp, type (hypothesis/experiment/decision), summary, evidence refs (paths/commands/output hashes)

Then:

* KB tool can answer: “show me prior decisions for this module” without reading full free-text plans.
* Pre-commit can enforce presence/updates for “high-impact” diffs (e.g., public API change, architecture boundary change).

### Why it helps

It captures the “ruled out hypotheses / pitfalls” that doc gardening cannot reliably rediscover later.

### Tradeoffs

* Requires discipline (but you can automate: agent writes entries as it goes; hook checks existence).

---

## A practical combined solution (minimal moving parts, high leverage)

If you want the smallest set that fully meets your requirements:

1. **Option 1 (Generated repo graph + KB query tool)** as the primary navigation substrate.
2. **Option 4 (structured worklog)** only for “decision-heavy” changes.
3. Keep `AGENTS.md` as the map—*but point it at the KB query tool as the first navigation step*, consistent with “give a map, not a manual.”

This preserves the harness-engineering strengths (progressive disclosure, mechanical enforcement)  while eliminating the two biggest churn sources you identified (free-text scanning and many tiny tool calls).

---

## Key implementation choices (the decisions that matter)

* **Determinism over cleverness:** prioritize symbol/tree/deps indexing and obligation checks that are stable and reproducible.
* **One-query interface:** hide “10 small files” behind one KB command that returns a directory digest + per-file one-liners + top symbols.
* **Generated-first:** treat human-authored prose as a *view*, not the canonical store, wherever possible.
* **Commit-gated obligations:** compute “knowledge that must change” from the diff + dependency graph, and block commits when it’s missing.

If you want, I can sketch a concrete schema for `repo_tree.json / symbols.jsonl / obligations.json`, plus an example pre-commit contract that enforces “changed surface area ⇒ updated knowledge shards/worklog,” without requiring agents to read or rewrite large markdown files.
