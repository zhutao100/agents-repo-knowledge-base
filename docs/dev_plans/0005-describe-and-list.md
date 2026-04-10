# DP-0005 — `describe` and `list` (typed discovery + deterministic lookups)

## Summary

Implement the typed discovery and lookup commands defined in `docs/DESIGN.md`:

* `kb describe path|module|symbol`
* `kb list modules|facts|symbols|tags`

These commands are the primary “no-fuzzy-search” navigation surface: agents discover via `list`, then select via exact IDs/paths and optionally request a bounded `pack`.

---

## Goals (must be true at end of DP-0005)

Commands exist and are deterministic:

* `kb describe path --path <PATH> --depth <N> --include {dirs,files,top_symbols,entrypoints} --format {json|text}`
* `kb describe module --id <MODULE_ID> --include {all,card,entrypoints,edit_points,related_facts} --format {json|text}`
* `kb describe symbol --id <SYMBOL_ID> --include {def,signature,uses,deps} --format {json|text}`
* `kb describe fact --id <FACT_ID> --format {json|text}`
* `kb list modules [--tag <TAG>] [--owner <OWNER>] --format {json|text}`
* `kb list facts [--type <FACT_TYPE>] [--tag <TAG>] --format {json|text}`
* `kb list symbols --path <PATH> [--kind <SYMBOL_KIND>] --format {json|text}`
* `kb list tags --format {json|text}`

All commands MUST:

* accept only typed selectors (no free text),
* be repo-bounded and path-normalizing,
* use stable ordering for all lists,
* return one JSON object to stdout for `--format json` success, or one JSON error object for failure.

---

## Non-goals

* No fuzzy discovery by substring search.
* No semantic ranking.
* No requirement to implement an xrefs backend; `uses` MAY be empty until `kb/gen/xrefs.jsonl` exists.

---

## Inputs and prerequisites

Target repo inputs:

* Generated index (required):
  * `kb/gen/tree.jsonl`
  * `kb/gen/symbols.jsonl` (unless disabled)
  * `kb/gen/deps.jsonl` (unless disabled)
* Overlays (optional but supported):
  * `kb/atlas/modules/*.toml`
  * `kb/facts/facts.jsonl`
* Config (optional but supported):
  * `kb/config/tags.toml` (validated tag vocabulary)

---

## Decisions (locked)

### 1) Data sources are exact and deterministic

* `list symbols` and `describe symbol` are backed by `kb/gen/symbols.jsonl`.
* `describe path` is backed by `kb/gen/tree.jsonl` (and `symbols.jsonl` for top-symbols derivation).
* `list modules` and `describe module` are backed by filenames + TOML parsing under `kb/atlas/modules/`.
* `list facts` is backed by JSONL parsing of `kb/facts/facts.jsonl`.
* `list tags` is backed by `kb/config/tags.toml` when present.

No command may fall back to filesystem walking as the primary mechanism.

### 2) Stable ordering (global)

All record lists MUST be stable-sorted:

* IDs lexicographically for ID lists.
* Paths lexicographically for path lists.

### 3) “Top symbols” derivation (no extra stored fields required)

Even if `kb/gen/tree.jsonl` does not populate `top_symbols`, `kb describe path --include top_symbols` MUST work by deriving top symbols from `kb/gen/symbols.jsonl`:

* For each file in the described subtree, compute `top_symbols` as the first `TOP_SYMBOLS_PER_FILE` symbol IDs for that file, where symbol IDs are stable-sorted.
* v1 `TOP_SYMBOLS_PER_FILE` default: 5.
* v1 MAY allow overriding via `kb/config/kb.toml` later; not required in this plan.

### 4) Tags are validated only when filtering by tag

If a command accepts `--tag <TAG>`, it MUST:

* fail with `INVALID_ARGUMENT` if `kb/config/tags.toml` is present and `<TAG>` is not in it,
* otherwise (no tags.toml), treat the filter as a strict string match without validation.

This avoids blocking early adoption in repos that haven’t introduced tag vocab yet, while still allowing strong validation when configured.

---

## Command details

### `kb list symbols`

Inputs:

* `--path <PATH>` must normalize to a repo-relative file path.

Algorithm:

1. Read `kb/gen/symbols.jsonl` (worktree) and select records with `path == <PATH>`.
2. If `--kind` is provided, select records with `kind == <SYMBOL_KIND>` (exact match).
3. Stable-sort selected records by `symbol_id`.

JSON output schema (key order is significant):

```json
{
  "path": "src/lib.rs",
  "kind": "function",
  "symbols": [
    { "symbol_id": "sym:v3:...", "name": "parse_thing", "qualified_name": "crate::parse_thing" }
  ]
}
```

### `kb describe symbol`

Algorithm:

1. Locate the symbol record in `kb/gen/symbols.jsonl` by exact `symbol_id`.
2. If `--include deps`, include outgoing deps for the symbol’s `path` from `kb/gen/deps.jsonl` (stable-sorted per `docs/SPECS.md`).
3. If `--include uses`:
   * if `kb/gen/xrefs.jsonl` exists, include matching xref edges where `to_symbol_id == <SYMBOL_ID>` (stable-sorted),
   * else return an empty `uses` list.

JSON output schema (key order is significant):

```json
{
  "symbol_id": "sym:v3:...",
  "def": { "path": "src/lib.rs", "line": 42, "end_line": 57, "kind": "function", "name": "parse_thing", "qualified_name": "crate::parse_thing" },
  "signature": "(...) -> ...",
  "uses": [],
  "deps": []
}
```

### `kb describe path`

Inputs:

* `--path` may point to a directory (`path/` or `.`) or a file.
* `--depth` controls subtree expansion (0 = only the node; 1 = immediate children; etc.).

Algorithm:

1. Load `kb/gen/tree.jsonl` and build an in-memory index of:
   * directories (`kind=dir`)
   * files (`kind=file`)
2. Resolve the target node by normalized path:
   * if `--path` is `.`, treat as root directory
   * if `--path` ends with `/`, treat as directory
   * else treat as file path if present, otherwise error `NOT_FOUND`
3. Expand descendants up to `--depth` and emit:
   * `dirs`: directory records (paths only)
   * `files`: file records (path/bytes/lines/lang)
4. If `--include top_symbols`:
   * for each emitted file, compute top symbols as defined in Decisions §3.
5. If `--include entrypoints`:
   * include files from the emitted subtree that appear in any module card’s `entrypoints` list (if module cards exist).

All output lists are stable-sorted by `path`.

### `kb list modules` / `kb describe module`

Module cards live under:

* `kb/atlas/modules/<MODULE_ID>.toml`

v1 required fields (TOML):

```toml
id = "payments.core"
title = "Payments core"
```

v1 optional fields:

```toml
owners = ["team:payments"]
tags = ["payments"]
entrypoints = ["src/payments/mod.rs"]
edit_points = ["src/payments/handlers.rs"]
related_facts = ["fact:v1:payments.api.endpoints"]
```

Rules:

* `<MODULE_ID>` MUST equal `id` in the TOML.
* `tags` MUST be stable-sorted in-file (linted later).

`kb list modules`:

* enumerates module cards by filename,
* applies exact-match filters:
  * `--tag` matches any tag in the module card,
  * `--owner` matches any owner in the module card,
* stable-sorts by module id.

`kb describe module`:

* loads and returns the card,
* returns requested includes as separate fields (`entrypoints`, `edit_points`, `related_facts`).

### `kb list facts`

Facts live at:

* `kb/facts/facts.jsonl`

Each JSONL record MUST be a JSON object with at least:

* `fact_id` (string, stable ID)
* `type` (string, exact-match fact type)

Optional fields:

* `tags`: array of strings
* `paths`: array of repo-relative paths
* `data`: JSON object (type-specific payload)

`kb list facts` filters by:

* `--type` (optional; exact match)
* `--tag` (optional; exact match)

Stable-sort results by `fact_id`.

`kb list facts` is for discovery and SHOULD return summaries. Use `kb describe fact` for the full record.

### `kb describe fact`

`kb describe fact` returns the full fact record by exact id:

* `kb describe fact --id <FACT_ID>`

### `kb list tags`

If `kb/config/tags.toml` exists, it defines the valid tag vocabulary.

v1 format:

```toml
[[tag]]
id = "payments"
description = "Payments domain"
```

Rules:

* `id` is required and unique.
* `kb list tags` outputs stable-sorted tag IDs.

---

## Acceptance tests (required)

Add unit/integration tests that:

* verify stable ordering of list outputs,
* verify tag validation behavior when tags.toml is present vs absent,
* verify `describe path` depth expansion is deterministic,
* verify `describe symbol` returns empty `uses` when xrefs missing,
* verify module card filename/id mismatch is rejected.
