# DP-0002 — Deterministic index generation (`kb/gen/*`) and `index check`

## Summary

Implement the baseline index pipeline:

* `kb index regen` generates deterministic, diff-friendly artifacts under `kb/gen/` in a target repo.
* `kb index check` fails (non-zero) if regeneration would change any committed `kb/gen/*` artifacts.

This plan implements `tree.jsonl`, `symbols.jsonl`, `deps.jsonl`, and `kb_meta.json` per `docs/SPECS.md`.

---

## Goals (must be true at end of DP-0002)

* Commands exist (typed, deterministic outputs):
  * `kb index regen --scope {all|changed} --diff-source {staged|worktree|commit:<sha>}`
  * `kb index check --diff-source {staged|worktree|commit:<sha>}`
* `kb/gen/kb_meta.json` is written without timestamps or environment fingerprints.
* `kb/gen/tree.jsonl` is generated from the Git-tracked tree and stable-sorted.
* `kb/gen/symbols.jsonl` is generated via Universal Ctags JSON and stable-sorted.
* `kb/gen/deps.jsonl` is generated via best-effort syntactic import parsing and stable-sorted.
* `index check` is deterministic and CI-safe: it must not depend on local filesystem ordering, locale, or timestamps.

---

## Non-goals

* No xrefs backend yet (`kb/gen/xrefs.jsonl` stays optional).
* No obligation evaluation yet (DP-0003/DP-0004).

---

## Decisions (locked)

### Artifact roots

* Target repo artifact root: `kb/` at repo root.
* Generated artifacts live under `kb/gen/` and are **committed**.
* Derived caches live under `kb/cache/` and are **gitignored**.

### `--scope changed` behavior (v1)

`--scope changed` MUST produce a correct full set of artifacts. In v1 it is allowed to internally fall back to full regeneration (same output as `--scope all`), but it MUST NOT produce partial/truncated artifact sets.

### Ctags baseline is required for symbols unless disabled

* If `symbols.jsonl` is required by config and Universal Ctags is missing or fails, `index regen` MUST fail non-zero.
* If symbol generation is disabled via config, `symbols.jsonl` is not required and must not be written.

---

## Implementation steps

### 1) Add `kb index` subcommands

Add clap subcommands:

* `kb index regen --scope {all|changed} --diff-source ...`
* `kb index check --diff-source ...`

Both commands MUST support `--format {json|text}`, but `index regen` primarily performs filesystem writes and may emit only a small JSON status object.

### 2) Implement “tracked path set” via Git

The generator MUST derive the path universe from Git, and it MUST treat `--diff-source` as the source-of-truth for both:

* the file set being indexed, and
* the file contents being read.

This is required for correctness under pre-commit gating (staged/index reads) and for reproducible “as-of commit” regeneration.

Path universe (file set) by diff-source:

* `staged` / `worktree`: `git ls-files -z`
* `commit:<sha>`: `git ls-tree -r --name-only -z <sha>`

Changed-path discovery (for `--scope changed` and any diff-scoped behavior):

* `staged`: `git diff --cached --name-status -z --find-renames`
* `worktree`: `git diff --name-status -z --find-renames`
* `commit:<sha>`: compare to first parent by default:
  * if `<sha>` has a parent: `git diff --name-status -z --find-renames <sha>^ <sha>`
  * if `<sha>` is a root commit: use the empty tree as base (`4b825dc642cb6eb9a060e54bf8d69288fbee4904`)

Indexing MUST exclude generated artifacts and caches to avoid self-referential churn:

* exclude any path under `kb/gen/`, `kb/cache/`, and `kb/.tmp/` from the indexed file set.

All paths must be normalized via the DP-0001 path module before being used.

### 3) Generate `kb/gen/kb_meta.json`

Write `kb/gen/kb_meta.json` exactly as specified in `docs/SPECS.md`:

* no timestamps
* stable `schemas[]` ordering
* only stable format/schema identifiers

### 4) Generate `kb/gen/tree.jsonl`

Inputs:

* tracked file paths from Git
* file contents via `DiffSourceReader` to compute `bytes` and `lines`

Algorithm (locked):

1. For every tracked file path, emit a file record.
2. Emit directory records for every parent directory of every tracked file.
3. Canonicalize:
   * directory paths end with `/`
   * file paths do not end with `/`
4. Populate:
   * `bytes`: byte length of the file content
   * `lines`: count of `\n` plus 1 if non-empty and not ending with `\n` (define precisely and test it)
   * `lang`: derived from extension via a small fixed mapping; default `unknown`
5. Stable-sort as specified in `docs/SPECS.md` and write JSONL.

### 5) Generate `kb/gen/symbols.jsonl` via Universal Ctags

#### 5.1 Ctags command and environment (locked)

Execute ctags:

* working directory: repo root
* environment: `LC_ALL=C`
* input: newline-separated tracked file paths (repo-relative), provided via `-L -`
* required flags:
  * `--output-format=json`
  * `--sort=no` (kb sorts itself)
  * `--quiet=yes`
  * `-f -` (write JSON tags to stdout)
  * `-L -` (read file list from stdin)
  * `--fields=+n+e+S+l+z` (enable line/end/signature/language/long kind)
  * `--fields=-T` (disable epoch/timestamp fields)

If the ctags invocation emits file-mtime-like fields, they MUST be ignored (or prevented via flags) so no timestamps enter committed artifacts.

#### 5.2 Symbol record extraction (locked)

For each ctags tag record:

* map to the v1 `symbols.jsonl` schema in `docs/SPECS.md`
* compute `symbol_id` exactly per §7.1 (`sym:v2:<sha256_96>`)
* sanitize:
  * normalize paths to repo-relative `/` form
  * ensure `raw` fields do not contain embedded newlines if they are persisted

Stable-sort the final symbol records by `symbol_id` and write JSONL.

### 6) Generate `kb/gen/deps.jsonl` (best-effort syntactic imports)

Implement a deterministic deps extractor with two layers:

1. **Generic edge capture** (always on)
   * For a limited set of languages, parse syntactic import/include statements and emit edges with `to_external` populated.
   * Emit `kind` as one of the v1 enums in `docs/SPECS.md`.

2. **Safe path resolution** (optional, deterministic)
   * For JavaScript/TypeScript:
     * resolve relative imports (`./` and `../`) to a repo-relative `to_path` when the target file can be resolved deterministically by extension probing (e.g., `.ts`, `.tsx`, `.js`, `.jsx`, `/index.ts`, etc.).
   * If resolution is ambiguous, fall back to `to_external` rather than guessing.

Stable-sort edges per `docs/SPECS.md` and write JSONL.

### 7) Implement `kb index check`

`index check` MUST:

1. regenerate artifacts into a temporary directory under the target repo (e.g., `kb/.tmp/regen/`),
2. compare the generated content byte-for-byte to the existing `kb/gen/*` artifacts **as-of the selected `--diff-source`**:
   * `staged`: read `kb/gen/*` from the Git index (not the worktree)
   * `worktree`: read `kb/gen/*` from the filesystem
   * `commit:<sha>`: read `kb/gen/*` from that commit
3. exit 0 if identical, else exit non-zero with a JSON error describing which artifact(s) differ,
4. remove the temporary directory on both success and failure.

Comparison must be platform-stable (no reliance on filesystem metadata).

---

## Acceptance tests (required)

Unit/integration tests (run inside the kb-tool repo):

* Tree generation:
  * directory paths end with `/`
  * stable ordering is invariant across runs
* Symbols generation:
  * symbol IDs stable across repeated runs with identical inputs
  * no timestamp-like fields appear in output
* Index check:
  * fails when an artifact is modified
  * succeeds after regen

Manual smoke checks (in a small fixture repo or this repo):

* `kb index regen --scope all --diff-source worktree`
* `kb index check --diff-source worktree`
