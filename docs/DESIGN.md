---

summary: Below is a **kb tool repository design** that implements the combined recommendation; **generated structural index as the primary navigation substrate**, with **thin human overlays**, **session capsules**, and **commit-gated freshness**. It explicitly targets your two churn classes (relevance + IO) and requires deterministic, in-session updates.

---

# Design

## Design intent

### What this repo provides

1. A local **`kb` CLI** that produces a **structured repo map** and **bounded context packs** using only **typed selectors** (paths, IDs, tags, enums)—no prompt/question free-text ingestion.
2. A **commit gate** (pre-commit + CI) that fails if:

   * generated KB artifacts are stale, or
   * the diff triggers “knowledge obligations” that are not satisfied.
3. A low-overhead integration path consistent with keeping `AGENTS.md` short as a TOC/map (Harness Engineering), but with a **first-class queryable substrate** beneath docs.

### Core principle

**Docs are UI; knowledge is data.** Markdown remains useful, but the agent’s “cheap navigation memory” is machine-addressable.

### Non-goals (interface hardening)

To keep the agent-facing surface area **clear, deterministic, and non-NLP**, the kb tool intentionally does **not** provide:

* free-text “task/question” inputs (no semantic ranking, no prompt interpretation),
* fuzzy search over repo content (“grep replacement”),
* network calls or remote backends (the tool is local and reproducible),
* selectors that depend on shell glob expansion or user locale (paths are normalized and repo-relative).

---

## Proposed repository layout (implementation target)

The layout below is the **intended end-state** for the kb tool repository. The current repo may start doc-only; the design requirements and contracts remain the same even if file names shift during implementation.

```text
kb-tool/
├── LICENSE
├── README.md
├── AGENTS.md
├── docs/
│   ├── OPERATIONS.md           # exact CLI contract, examples, exit codes
│   ├── DATA_MODEL.md           # schemas + invariants
│   ├── INTEGRATION_CODEX.md    # “drop-in” AGENTS.md snippet + hooks
│   ├── ENFORCEMENT.md          # pre-commit/CI gates + policy examples
│   └── BACKENDS.md             # symbol/deps extractors (ctags, tree-sitter, etc.)
├── schemas/                    # JSON Schemas for all persisted artifacts
│   ├── gen_tree.schema.json
│   ├── gen_symbols.schema.json
│   ├── gen_deps.schema.json
│   ├── atlas_module.schema.json
│   ├── fact.schema.json
│   └── session.schema.json
├── kb/
│   ├── config/
│   │   ├── kb.toml             # defaults + thresholds
│   │   ├── tags.toml           # tag vocabulary (validated)
│   │   └── obligations.toml    # diff-triggered requirements
│   ├── templates/
│   │   ├── module.toml         # atlas module card template
│   │   └── session.json        # capsule skeleton template
│   └── bin/
│       └── kb                  # installed CLI entrypoint (single binary or script)
├── src/                        # implementation (language-agnostic in this design)
│   ├── index/                  # generators: tree/symbols/deps
│   ├── query/                  # describe/list/pack planners
│   ├── policy/                 # obligations + validators
│   └── io/                     # stable serialization, sorting, hashing
└── scripts/
    ├── install.sh              # installs kb + git hooks into a target repo
    ├── hook-pre-commit.sh      # runs kb check/lint/obligations
    └── ci-check.sh             # CI entrypoint
```

This repo is the **tool**. A target codebase vendors it (subtree/submodule/copy) or installs the binary, then commits **KB artifacts inside the target repo** (see next section).

---

## Knowledge artifacts in a *target* repo

The kb tool generates and validates **deterministic, diffable** artifacts under a fixed directory (configurable):

```text
<target-repo>/
└── kb/
    ├── gen/                    # generated-first repo map (primary substrate)
    │   ├── kb_meta.json
    │   ├── tree.jsonl
    │   ├── symbols.jsonl
    │   ├── deps.jsonl
    │   └── xrefs.jsonl           # optional cross-reference edges (diffable text)
    ├── atlas/                  # thin human overlays (“why / edit points”)
    │   └── modules/
    │       ├── payments.core.toml
    │       └── auth.core.toml
    ├── facts/                  # optional lookup atoms (strictly typed)
    │   └── facts.jsonl
    ├── sessions/               # session capsules (thresholded requirement)
    │   └── 2026/04/PR-1234.json
    └── cache/                  # local-only derived caches (default: gitignored)
        └── xrefs.sqlite         # optional query acceleration; derived from gen/*
```

This is exactly the “second layer under `docs/`” implied by the proposals: small stable atoms, anchored to code, queryable in one call.

---

## Agent-facing CLI: operations and typed parameters

### Design constraints (enforced)

* **No natural-language parameters** intended for NLP ranking or prompt-like interpretation (no `--task`, no `--question`).
* **No fuzzy discovery by free text.** Discovery is via `list` commands, then exact selectors.
* **Repo-relative addressing.** Path selectors must be repo-relative and must not escape the repo root (`..` is rejected after normalization).
* All selection is via:

  * exact IDs (module/fact/symbol),
  * paths and path prefixes (no shell-dependent globs),
  * tags from a validated vocabulary,
  * enums (include sets, formats, diff sources, policy modes),
  * numeric budgets (bytes/records/lines).

### Output + error contracts (agent-friendly)

* `--format json` is the **stability surface** (schema’d); `--format text` is a convenience view (may evolve).
* With `--format json`, successful commands print exactly one JSON object (or JSONL stream where specified) to stdout.
* With `--format json`, failures print exactly one JSON error object to stdout and exit non-zero (logs go to stderr).
* All record lists are stable-sorted to minimize churn and diff noise.

### Operations (minimal, explicit set)

#### 1) Index lifecycle

* `kb index regen --scope {all|changed} --diff-source {staged|worktree|commit:<sha>}`
* `kb index check --diff-source {staged|worktree|commit:<sha>}`

Contract:

* `regen` **writes** `kb/gen/*` deterministically (stable sort, no timestamps in content).
* `check` exits non-zero if regeneration would change tracked artifacts.

Notes:

* `--diff-source staged` refers to the Git index (what will be committed); `worktree` refers to the working tree.
* `--diff-source commit:<sha>` compares that commit to its parent by default (overrideable in future via explicit `--base` / `--head` revs).

#### 2) Describe (deterministic lookups)

* `kb describe path --path <PATH> --depth <N> --include {dirs,files,top_symbols,entrypoints} --format {json|text}`
* `kb describe module --id <MODULE_ID> --include {all,card,entrypoints,edit_points,related_facts} --format {json|text}`
* `kb describe symbol --id <SYMBOL_ID> --include {def,signature,uses,deps} --format {json|text}`
* `kb describe fact --id <FACT_ID> --format {json|text}`

Notes:

* `SYMBOL_ID` is a stable ID produced by the indexer and validated strictly (see `docs/SPECS.md` for the normative format).

#### 3) List (no fuzzy search)

* `kb list modules [--tag <TAG>] [--owner <OWNER>]`
* `kb list facts [--type <FACT_TYPE>] [--tag <TAG>]`
* `kb list symbols --path <PATH> [--kind <SYMBOL_KIND>]`
* `kb list tags`

All filters are exact-match; `TAG` must exist in `kb/config/tags.toml`.

#### 4) Plan-from-diff (obligations)

* `kb plan diff --diff-source {staged|worktree|commit:<sha>} --policy {default|strict} --format {json|text}`

Output is a structured plan:

* changed paths
* affected modules
* triggered obligation rules
* required updates (which module cards / which fact types / whether a session capsule is required)

This is the commit-gated “in-session update” engine.

#### 5) Pack (bounded context packs without free text)

* `kb pack diff [--diff-source {staged|worktree|commit:<sha>}] [--radius <DEP_RADIUS:int>] [--max-bytes <N>] [--snippet-lines <N>] --format {json|text}`
* `kb pack selectors [--path <PATH>]... [--module <MODULE_ID>]... [--symbol <SYMBOL_ID>]... [--fact <FACT_ID>]... [--max-bytes <N>] [--snippet-lines <N>] --format {json|text}`

`pack` is the **single-call retrieval** mechanism that reduces IO churn: it returns a bounded bundle of:

* module cards (if any)
* relevant generated index entries (tree/symbols/deps)
* optional code excerpts (definitions only, or def+uses if requested)

No prompt interpretation is required.

Selector expansion (v1):

* `pack selectors --module <MODULE_ID>` expands the module’s `entrypoints`/`edit_points` into `--path` selectors and the module’s `related_facts` into `--fact` selectors.
* `pack selectors --path <DIR/>` includes a bounded subtree under the directory prefix (stable, depth-limited, capped).

#### 6) Session capsules (thresholded)

* `kb session init --id <SESSION_ID> [--tag <TAG>]...`
* `kb session finalize --id <SESSION_ID> --diff-source {staged|worktree|commit:<sha>} [--verification {tests|bench|repro|lint}]...`
* `kb session check --id <SESSION_ID>`

These are structured records to capture session-only truth that doc-gardening cannot recover reliably.

#### 7) Overlay writes (deterministic; optional)

These commands write the KB overlay files deterministically (stable ordering, no timestamps). They exist to support repo backfills and in-session updates without requiring manual edits to `kb/` files.

* Tag vocabulary:
  * `kb tags upsert --id <TAG> [--description <TEXT>]`
* Module cards:
  * `kb module init --id <MODULE_ID> [--title <TITLE>] [--owner <OWNER>]... [--tag <TAG>]... [--entrypoint <PATH>]... [--edit-point <PATH>]... [--related-fact <FACT_ID>]...`
  * `kb module upsert ...` (same flags; overwrites deterministically)
* Facts:
  * `kb fact upsert --id <FACT_ID> --type <FACT_TYPE> [--tag <TAG>]... [--path <PATH>]... [--data-json <JSON>]`
* Obligations:
  * `kb obligations upsert-rule --id <RULE_ID> --when-path-prefix <PREFIX> [--require-module-card <MODULE_ID>] [--require-fact-type <FACT_TYPE>]... [--require-session-capsule true]`

All selectors are typed (exact IDs, repo-relative paths/prefixes, validated tags, enums, numeric budgets).

#### 8) Lint / policy checks

* `kb lint all`
* `kb obligations check --diff-source {staged|worktree|commit:<sha>}`

`obligations check` is what pre-commit and CI run to block merges when knowledge updates are missing.

---

## Policy model: obligations.toml (deterministic, no heuristics)

Example `kb/config/obligations.toml` in a target repo:

```toml
# If anything under src/payments changes, require the payments.core module card updated in the same diff.
[[rule]]
id = "module_card.payments"
when_path_prefix = "src/payments/"
require_module_card = "payments.core"

# If public API surface changes, require facts of type api_endpoint updated and a session capsule.
[[rule]]
id = "api.requires_facts_and_session"
when_path_prefix = "src/api/"
require_fact_types = ["api_endpoint"]
require_session_capsule = true

# If migrations change, require a capsule and a fact update.
[[rule]]
id = "migrations"
when_path_prefix = "migrations/"
require_fact_types = ["data_migration"]
require_session_capsule = true
```

The `kb plan diff` command reports which rules fired and what artifacts must be present/updated.

---

## Enforcement (pre-commit + CI)

### Pre-commit hook (hard gate)

`scripts/hook-pre-commit.sh` runs, in order:

1. `kb index check --diff-source staged`
2. `kb lint all`
3. `kb obligations check --diff-source staged`

Failing any step blocks the commit, forcing **in-session** KB updates alongside code changes.

### CI (anti-bypass)

CI repeats the same checks on the merge commit (or PR head) and fails on any discrepancy. This matches the Harness Engineering “mechanical enforcement” philosophy while addressing the missing “cheap navigation memory” layer.

---

## Codex CLI integration recipe (AGENTS.md snippet)

This is the “lowest-overhead” integration path you asked for: a brief instruction block in the target repo’s master `AGENTS.md`, teaching agents to use `kb` *before* manual browsing.

```markdown
## Repo navigation (use kb first)

Before opening many files or running wide searches, use the local kb tool:

- Directory map: `kb describe path --path <PATH> --depth 2 --include dirs,files,top_symbols --format text`
- Module intent + edit points: `kb describe module --id <MODULE_ID> --format text`
- Diff-driven obligations (what knowledge must update): `kb plan diff --diff-source worktree --format text`
- Single-call context pack for current changes: `kb pack diff --diff-source worktree --radius 1 --max-bytes 120000 --snippet-lines 80 --format text`

Only after kb output indicates likely files/symbols should you open code files directly.
```

No MCP is required; this is standard local-command usage with minimal ceremony.

---

## Typical agent workflows

### Start-of-task (navigation without IO churn)

1. `kb describe path --path . --depth 2 --include dirs,files --format text`
2. `kb list modules --tag <TAG>`
3. `kb describe module --id <MODULE_ID>`

### After edits (deterministic “what must I update?”)

1. `kb plan diff --diff-source worktree --format text`
2. Update the required artifacts (module card / facts / session capsule).
3. `kb index regen --scope changed --diff-source worktree`
4. Commit (pre-commit enforces).

This directly satisfies the background requirement that updates must happen in-session and be mechanically enforced.

---

## Concise reasoning narrative

I converted the recommendation into a repo design by (1) making the generated index the canonical “repo map” to eliminate navigation churn, (2) adding thin overlays and session capsules for the “why” that generation cannot recover, and (3) putting all freshness and obligation logic behind deterministic diff-based rules enforced in pre-commit/CI. This aligns with Harness Engineering’s “AGENTS.md as map + mechanical enforcement” while closing the explicit “b/c navigation” gap with a queryable substrate and a single-call pack workflow.
