# DP-0001 — CLI skeleton, canonical IO, diff-source reader

## Summary

Create the Rust single-binary `kb` CLI skeleton and the foundational IO layer that guarantees the determinism rules in `docs/SPECS.md` (UTF-8, LF, stable ordering, minified JSON, JSONL rules). Add the repo-bounded path normalization and the “read file as-of diff-source” abstraction required for staged/worktree/commit correctness.

This plan does **not** implement indexing or pack logic; it only creates the safe, deterministic substrate.

---

## Goals (must be true at end of DP-0001)

* A `kb` binary exists and runs on macOS/Linux/Windows.
* CLI supports a minimal command set with typed arguments and stable output:
  * `kb version --format {json|text}`
  * `kb help` (standard)
  * `kb debug diff-source --diff-source {staged|worktree|commit:<sha>}` (temporary, internal-only command; can be removed later)
* A canonical IO module exists:
  * minified JSON writer
  * JSON error writer (one object to stdout; logs to stderr)
  * JSONL writer with stable sorting helpers
* A repo root is discovered deterministically and path traversal is rejected (no `..` escapes).
* A `DiffSourceReader` exists that can read:
  * staged blobs from the Git index,
  * commit blobs from a specific commit,
  * worktree files from disk,
  all while remaining repo-bounded.

---

## Non-goals

* No symbol/deps/tree generation yet.
* No “pack” or “plan” commands yet.
* No pre-commit/CI integration yet (handled in DP-0004).

---

## Decisions (locked)

### Language and packaging

* Implement `kb` as a Rust single-binary CLI.
* Prefer minimal dependencies; avoid “framework” CLIs that hide IO or sorting behavior.

### Output contract

* `--format json` MUST emit exactly one JSON object to stdout for success (or JSONL where explicitly documented later).
* For failures, `--format json` MUST emit exactly one JSON error object to stdout and exit non-zero; stderr may contain logs.
* JSON is minified. JSON object key order is stable and matches schema order (see `docs/SPECS.md`).

### Repo-boundedness

* All user-provided paths are treated as repo-relative and normalized.
* Any normalized path that would escape the repo root is rejected.

---

## Implementation steps

### 1) Bootstrap the Rust workspace

Create:

* `Cargo.toml` for a single binary crate `kb`.
* `src/main.rs` with clap-driven CLI parsing.
* `src/lib.rs` (optional) if you prefer a library + binary split.

Dependencies (locked set unless a later plan amends it):

* CLI: `clap` (derive)
* JSON: `serde`, `serde_json`
* Errors: `thiserror`, `anyhow` (or `miette`; pick one and standardize)
* Hashing: `sha2` (needed for `SYMBOL_ID` spec)

### 2) Define the CLI command graph (minimal, typed)

Implement:

* `kb version --format {json|text}`
  * json output schema (key order significant):
    ```json
    { "name": "kb", "version": "0.0.0" }
    ```
* `kb debug diff-source --diff-source {staged|worktree|commit:<sha>}`
  * outputs the resolved diff-source in JSON, to validate parsing and wiring
  * this command is intentionally namespaced as `debug` so it cannot become an accidental public interface; remove after DP-0002.

Global flags:

* `--format {json|text}` (default: `json` for machine stability; `text` as convenience view)

### 3) Canonical JSON writer

Create `src/io/json.rs` (or similar) that:

* serializes structs (not arbitrary maps) to preserve key order,
* outputs minified JSON without trailing whitespace,
* writes to stdout only, and
* is used by all `--format json` commands.

Also define a stable JSON error object schema (key order significant):

```json
{
  "error": {
    "code": "INVALID_ARGUMENT",
    "message": "human readable, stable enough for logs",
    "details": []
  }
}
```

Rules:

* `code` is an enum-like string; define a small set now: `INVALID_ARGUMENT`, `NOT_FOUND`, `BACKEND_MISSING`, `BACKEND_FAILED`, `INTERNAL`.
* `details` is an array of `{ "key": "...", "value": "..." }` pairs (stable-sorted by `key`) to avoid unordered maps.

### 4) JSONL writer + stable sorting helpers

Create `src/io/jsonl.rs` that:

* writes one record per line,
* ensures trailing `\n`,
* provides helpers:
  * `stable_sort_by_key(Vec<T>, key_fn)` with lexicographic ordering,
  * `write_jsonl(path, records)` which guarantees stable ordering before write (the caller provides the comparator).

### 5) Repo root discovery and path normalization

Create `src/repo/root.rs` + `src/repo/path.rs`:

* Determine repo root by invoking `git rev-parse --show-toplevel`.
* Normalize repo-relative paths:
  * reject absolute paths,
  * normalize separators to `/`,
  * collapse `.` segments,
  * reject `..` segments after normalization.

### 6) Diff-source parsing and file reading abstraction

Create `src/repo/diff_source.rs` + `src/repo/reader.rs`:

* Parse `--diff-source` into:
  * `Staged`
  * `Worktree`
  * `Commit(String)`
* Implement `read_to_string(path)` and `read_bytes(path)` for each mode:
  * `Worktree`: read from filesystem under repo root.
  * `Staged`: read from Git index (plumbing), e.g. `git show :<path>` (ensure correct quoting and NUL-safe path passing).
  * `Commit`: read from `git show <sha>:<path>`.

Hard requirements:

* All reads must be repo-bounded by normalized repo-relative paths.
* All Git invocations must be locale-stable (`LC_ALL=C`) and must not depend on user config for output formatting.

---

## Acceptance tests (required)

Add unit tests for:

* path normalization rejects `../x`, `/abs/path`, and accepts `src/lib.rs`.
* JSON writer produces minified output and stable key order.
* JSONL writer always ends with a final newline.

Manual smoke checks:

* `kb version --format json` prints one JSON object and exits 0.
* `kb version --format text` prints a human-readable string and exits 0.
* `kb debug diff-source --diff-source staged --format json` prints the parsed mode.
