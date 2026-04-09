# DP-0003 — Deterministic `plan diff` and `pack` planners

## Summary

Implement the query layer that converts the generated index plus a diff into **single-call, bounded, deterministic** outputs:

* `kb plan diff` computes which policy obligations are triggered by the diff.
* `kb pack diff` produces a bounded context pack derived from the diff + index edges (no fuzzy search, no NLP).
* `kb pack selectors` produces a bounded context pack derived from exact selectors (paths, IDs).

This plan defines the planner algorithms, output schemas, and budgeting behavior so implementation is decision-complete.

---

## Goals (must be true at end of DP-0003)

Commands exist and are deterministic:

* `kb plan diff --diff-source {staged|worktree|commit:<sha>} --policy {default|strict} --format {json|text}`
* `kb pack diff --diff-source {staged|worktree|commit:<sha>} --radius <DEP_RADIUS:int> --max-bytes <N> --snippet-lines <N> --format {json|text}`
* `kb pack selectors [--path <PATH>]... [--symbol <SYMBOL_ID>]... --max-bytes <N> --snippet-lines <N> --format {json|text}`

All three MUST:

* accept only typed selectors (no free text),
* be repo-bounded,
* produce stable ordering,
* respect `--max-bytes` and `--snippet-lines` deterministically.

---

## Non-goals

* No “fuzzy discovery” or NLP ranking.
* No xrefs backend required; if `kb/gen/xrefs.jsonl` is absent, planners must still work using `deps.jsonl` and symbols-by-path.

---

## Inputs and prerequisites (must exist)

In the target repo:

* `kb/gen/kb_meta.json`
* `kb/gen/tree.jsonl`
* `kb/gen/symbols.jsonl` (unless disabled)
* `kb/gen/deps.jsonl` (unless disabled)
* `kb/config/obligations.toml` (for `plan diff` and enforcement planning)

---

## Decisions (locked)

### 1) “No fuzzy search” interpretation for planners

Planners MAY compute “relevance” only from:

* changed paths in the diff,
* deterministic graph edges in committed artifacts (`deps.jsonl`, `xrefs.jsonl` when available),
* explicit obligation rules in `kb/config/obligations.toml`,
* explicit selectors provided by the user.

They MUST NOT use:

* free-text queries,
* embedding/semantic similarity,
* grep-like substring search as the primary selection mechanism.

### 2) Stable ordering and drop policy (budgeting)

All planners follow the global drop policy in `docs/SPECS.md`:

1. include required plan metadata,
2. include overlays (if any exist; may be empty in v1),
3. include generated index entries,
4. include code excerpts last, dropping excerpts first when budgets are hit.

Within each section, records are stable-sorted as defined in `docs/SPECS.md` (or by the comparator defined below when output-only records are introduced).

### 3) Rename handling

Diff parsing MUST treat renames deterministically:

* if the diff provides both old and new paths, both paths are included in `changed_paths` with a `change_kind` of `rename`.
* obligation matching is applied to both old and new paths.

This is conservative and prevents missing required updates when a path prefix changes due to rename/move.

---

## `kb plan diff` (policy planner)

### Purpose

Compute “what must be reviewed/updated” deterministically from the diff and `kb/config/obligations.toml`. This is the input to enforcement and a human-readable checklist for in-session updates.

### Obligations config (v1, minimal)

`kb/config/obligations.toml` is a list of `[[rule]]` tables:

```toml
[[rule]]
id = "module_card.payments"
when_path_prefix = "src/payments/"
require_module_card = "payments.core"

[[rule]]
id = "api.requires_facts_and_session"
when_path_prefix = "src/api/"
require_fact_types = ["api_endpoint"]
require_session_capsule = true
```

Rules:

* `id` is required and must be unique.
* `when_path_prefix` is required and must be a normalized repo-relative prefix.
* `require_module_card` is optional (string module id).
* `require_fact_types` is optional (array of strings; exact-match types).
* `require_session_capsule` is optional (bool; default false).

This is intentionally “dumb”: prefix match only, no heuristics.

### Output schema (`--format json`)

One JSON object (key order is significant):

```json
{
  "diff_source": "staged",
  "policy": "default",
  "changed_paths": [
    { "path": "src/payments/core.rs", "change_kind": "modify" }
  ],
  "triggered_rules": [
    { "id": "module_card.payments", "when_path_prefix": "src/payments/" }
  ],
  "required": {
    "module_cards": ["payments.core"],
    "fact_types": ["api_endpoint"],
    "session_capsule": false
  }
}
```

Rules:

* `changed_paths` is stable-sorted by `path`, then `change_kind`.
* `triggered_rules` is stable-sorted by `id`.
* `required.module_cards` and `required.fact_types` are stable-sorted unique arrays.

`change_kind` allowed set (v1): `add`, `modify`, `delete`, `rename`, `unknown`.

### Text output (`--format text`)

Produce a deterministic, line-oriented summary:

* header includes diff_source and policy
* lists changed paths
* lists triggered rule IDs
* lists required artifacts (module cards / fact types / session capsule)

No free prose paragraphs; keep it grep-friendly and stable.

---

## `kb pack diff` (diff-driven context pack)

### Purpose

Return a bounded bundle of machine-addressable context for the current changes in **one call** (IO churn reduction), without fuzzy search. It is a deterministic planner over:

* `changed_paths` from the diff,
* dependency edges (`deps.jsonl`, optionally `xrefs.jsonl`),
* symbol definitions for included paths (`symbols.jsonl`),
* optional code excerpts from included symbol definitions.

### Algorithm (locked)

Inputs:

* `changed_paths` from `--diff-source`
* `radius` (int >= 0)
* `deps.jsonl` edges with `to_path`

Steps:

1. Seed path set `S0` with the changed file paths that are:
   * present in `kb/gen/tree.jsonl` as `kind=file`, and
   * not deleted at the selected diff-source (for deletes, include the path in metadata but do not attempt to read contents).
2. Build an undirected adjacency list `G` over file paths using `deps.jsonl` edges where `to_path` is present:
   * add neighbor `from_path -> to_path`
   * add neighbor `to_path -> from_path`
   * ignore edges whose endpoints are not present in the current `tree.jsonl` file set
3. Expand `S` by BFS up to `radius` hops from `S0`:
   * neighbors are visited in stable lexicographic order by path
   * ties are broken by path string only
4. For the final included file path set `S`, collect:
   * `tree_records`: tree.jsonl records for paths in `S`
   * `symbol_records`: symbols.jsonl records where `path` ∈ `S`
   * `dep_edges`: deps.jsonl edges where `from_path` ∈ `S` and (`to_path` ∈ `S` or `to_external` present)
5. Compute `snippets` (optional):
   * For each included file path, choose up to `K` symbol defs to excerpt:
     * select symbols in stable order by `symbol_id`
     * excerpt at most 1 symbol per file in v1 (to reduce churn); future versions may raise this
   * excerpting rule:
     * read file contents via `DiffSourceReader` for the selected `--diff-source`
     * extract lines `[line, min(end_line, line+snippet_lines-1)]`
     * if `end_line` missing, use `[line, line+snippet_lines-1]`
   * snippet text MUST be LF-only and MUST NOT include trailing spaces

### Output schema (`--format json`)

One JSON object (key order is significant):

```json
{
  "diff_source": "staged",
  "radius": 1,
  "budgets": { "max_bytes": 120000, "snippet_lines": 80 },
  "changed_paths": [
    { "path": "src/payments/core.rs", "change_kind": "modify" }
  ],
  "tree": [],
  "symbols": [],
  "deps": [],
  "snippets": []
}
```

Rules:

* `tree` records conform to `kb/gen/tree.jsonl` schema.
* `symbols` records conform to `kb/gen/symbols.jsonl` schema.
* `deps` records conform to `kb/gen/deps.jsonl` schema.
* `snippets` record schema (key order significant):
  ```json
  {
    "path": "src/lib.rs",
    "symbol_id": "sym:v1:...",
    "start_line": 1,
    "end_line": 10,
    "text": "..."
  }
  ```

Stable ordering:

* `tree` sorted by `path`
* `symbols` sorted by `symbol_id`
* `deps` sorted per `docs/SPECS.md`
* `snippets` sorted by `path`, then `symbol_id`

Budgeting:

* Apply `--max-bytes` to the serialized JSON output.
* If adding a section would exceed the budget:
  * drop `snippets` first (truncate to fit; stable order),
  * then drop `deps` edges,
  * then drop `symbols`,
  * then drop `tree`,
  * but NEVER drop `changed_paths`, `diff_source`, `radius`, or `budgets`.

### Text output (`--format text`)

Produce a deterministic report:

* changed paths
* included file set (paths)
* symbols summary (count + top N symbol IDs per file, stable)
* deps summary (count)
* snippets (if any, as fenced code blocks with `path:line` headers)

---

## `kb pack selectors` (selector-driven context pack)

### Purpose

Return a bounded context pack from explicit selectors only. No diff computation and no graph expansion.

### Selector semantics (v1)

* `--path <PATH>` includes:
  * matching `tree` record for the file (or directory record if path ends with `/`),
  * all `symbols` records with `path == <PATH>`,
  * all `deps` edges with `from_path == <PATH>`.
* `--symbol <SYMBOL_ID>` includes:
  * the symbol record,
  * a definition snippet for that symbol if file content is available under diff-source `worktree` (selectors pack uses worktree reads in v1).

If multiple selectors are provided, the pack is the union of included records, stable-sorted.

### Output schema

Same schema as `pack diff`, except:

* there is no `changed_paths` field; instead emit `selectors`:
  ```json
  { "selectors": { "paths": ["..."], "symbols": ["..."] } }
  ```

Budgeting and drop policy are the same as `pack diff`.
