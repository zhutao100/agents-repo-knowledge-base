# kb-tool specs (v1)

This document defines the **normative, deterministic contracts** for kb-tool: artifact formats, ID rules, “diff-source” semantics, repo-bounded path rules, and budgeting behavior. It is written to be implementation-language-agnostic and intended to stand alone.

`docs/DESIGN.md` may provide additional rationale and higher-level intent, but if there is any conflict, **this spec wins**.

---

## 1) Scope

This spec defines:

* deterministic serialization rules for JSON/JSONL artifacts,
* repo-bounded path rules,
* the meaning of `--diff-source {staged|worktree|commit:<sha>}`,
* the required on-disk artifact set (in a target repo),
* v1 schemas for generated artifacts under `kb/gen/`,
* stable ID rules (notably `SYMBOL_ID`),
* baseline backend/toolchain requirements for symbol extraction,
* deterministic budgeting behavior for bounded outputs.

This spec assumes a `kb` **local CLI** that only accepts **typed selectors** (paths, IDs, enums, numeric budgets) and generates a deterministic repo map under `kb/gen/` in a target repo.

The baseline generators referenced by this spec are:

* tree: Git-tracked file tree
* symbols: Universal Ctags JSON → `kb/gen/symbols.jsonl`
* deps: best-effort syntactic import parsing → `kb/gen/deps.jsonl`

Alternate backends are allowed only if they produce artifacts that conform to the schemas and determinism rules in this document.

---

## 2) Normative language

The key words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are used as described in RFC 2119.

---

## 3) Determinism and portability rules (hard requirements)

### 3.1 Text encoding and newlines

* All persisted artifacts MUST be UTF-8.
* All persisted artifacts MUST use LF (`\n`) newlines.

### 3.2 Repo-bounded paths

All paths surfaced to the user (selectors) or persisted in artifacts MUST be:

* repo-relative (no absolute paths),
* normalized (no `.` segments),
* rejected if they would escape the repo root after normalization (no `..`),
* represented using forward slashes (`/`) regardless of OS.

### 3.3 Canonical JSON and JSONL

To minimize diff churn:

* JSON output MUST be **minified** (no pretty-print whitespace) and terminated by a single trailing `\n`.
* JSON object keys MUST be emitted in a **stable order**. To make this implementable across languages, the spec requires:
  * Each persisted record is a schema’d object with a fixed set of keys.
  * Keys are serialized in the exact order shown in the schema sections below.
* JSONL files MUST contain:
  * exactly one JSON object per line,
  * lines terminated by `\n`,
  * no trailing spaces.

### 3.4 Stable ordering of sets

Whenever an array represents a set (tags, IDs, edge lists), it MUST be sorted lexicographically by the array element’s string form unless a schema section below specifies a different comparator.

---

## 4) Diff-source semantics (staged vs worktree vs commit)

Many operations accept `--diff-source {staged|worktree|commit:<sha>}` and MUST interpret them as:

* `staged`: **Git index** is the source-of-truth for file contents. Changed paths are derived from the index vs HEAD. This is the correctness mode for pre-commit gating.
* `worktree`: **Working tree** is the source-of-truth for file contents. Changed paths are derived from worktree vs HEAD.
* `commit:<sha>`: the named commit is compared to its parent (first parent); file contents are read from that commit.

All three modes MUST operate over the Git-tracked path set (i.e., `git ls-files`) unless explicitly configured otherwise. This avoids filesystem nondeterminism and symlink/path-escape hazards.

---

## 5) On-disk artifact set (in a target repo)

### 5.1 Required directories

In a target repo, `kb/` is reserved for kb artifacts and configs:

```text
kb/
  gen/
  config/
  atlas/
  facts/
  sessions/
  cache/   (gitignored; derived only)
```

### 5.2 Required generated artifacts (`kb/gen/`)

`kb/gen/` MUST contain:

* `kb/gen/kb_meta.json`
* `kb/gen/tree.jsonl`
* `kb/gen/symbols.jsonl` (required unless disabled in config)
* `kb/gen/deps.jsonl` (required unless disabled in config)

`kb/gen/xrefs.jsonl` is OPTIONAL (allowed to be absent) until an xref backend is implemented.

### 5.3 `kb_meta.json` (generation metadata without churn)

`kb_meta.json` is a single JSON object. It MUST NOT include:

* absolute paths,
* timestamps,
* hostnames/usernames,
* commit SHAs,
* toolchain versions (ctags version, OS version, etc.).

It MAY include only stable configuration and format/schema versions.

Schema (key order is significant):

```json
{
  "kb_format_version": 1,
  "schemas": [
    { "name": "kb/gen/tree.jsonl", "version": 1, "required": true },
    { "name": "kb/gen/symbols.jsonl", "version": 1, "required": true },
    { "name": "kb/gen/deps.jsonl", "version": 1, "required": true },
    { "name": "kb/gen/xrefs.jsonl", "version": 1, "required": false }
  ]
}
```

The `schemas` array MUST be stable-sorted by `name`.

---

## 6) Artifact schemas (v1)

This section defines the **minimum** required fields for v1. Implementations MAY add new optional fields later, but MUST preserve existing fields and meanings.

### 6.1 `tree.jsonl` (v1)

Each line is one node record.

Directory record schema (key order is significant):

```json
{ "path": "src/", "kind": "dir" }
```

File record schema (key order is significant):

```json
{
  "path": "src/lib.rs",
  "kind": "file",
  "bytes": 1234,
  "lines": 56,
  "lang": "rust",
  "top_symbols": ["sym:v2:..."]
}
```

Rules:

* `path` MUST be the normalized repo-relative path. Directory paths MUST end with `/`. File paths MUST NOT end with `/`.
* `bytes` and `lines` MUST be non-negative integers.
* `lang` SHOULD be a stable, lowercase identifier (e.g., `rust`, `go`, `python`, `unknown`).
* `top_symbols` is OPTIONAL; when present, it MUST be stable-sorted.

`tree.jsonl` MUST be stable-sorted by:

1. `path` (lexicographic)
2. `kind` (`dir` before `file` for identical path prefixes)

### 6.2 `symbols.jsonl` (v1)

Each line is one symbol definition record.

Schema (key order is significant):

```json
{
  "symbol_id": "sym:v2:0123456789abcdef01234567",
  "lang": "rust",
  "path": "src/lib.rs",
  "kind": "function",
  "name": "parse_thing",
  "qualified_name": "crate::parser::parse_thing",
  "line": 42,
  "end_line": 57,
  "signature": "(...) -> ...",
  "scope": "crate::parser"
}
```

Rules:

* `symbol_id` is a stable opaque ID defined in §7.
* `line` and `end_line` are 1-based line numbers; `end_line` MAY be omitted when not available.
* `signature` and `scope` MAY be omitted.

`symbols.jsonl` MUST be stable-sorted by `symbol_id`.

### 6.3 `deps.jsonl` (v1)

Each line is one dependency edge record.

Schema (key order is significant):

```json
{
  "from_path": "src/lib.rs",
  "kind": "import",
  "to_path": "src/parser.rs",
  "to_external": "serde_json",
  "raw": "use serde_json::Value;"
}
```

Rules:

* Exactly one of `to_path` or `to_external` MUST be present.
* `raw` is OPTIONAL; if present, it MUST be a single-line string (newlines replaced with `\\n`).
* `kind` MUST be a stable lowercase enum string. v1 allowed set: `import`, `include`, `require`, `dynamic`, `unknown`.

`deps.jsonl` MUST be stable-sorted by:

1. `from_path`
2. `kind`
3. `to_path` (if present) else `to_external`
4. `raw` (if present)

### 6.4 `xrefs.jsonl` (v1, optional)

This file is OPTIONAL in v1, but the schema is reserved:

Schema (key order is significant):

```json
{
  "from_symbol_id": "sym:v2:...",
  "kind": "ref",
  "to_symbol_id": "sym:v2:...",
  "path": "src/lib.rs",
  "line": 123
}
```

Rules:

* `kind` MUST be a stable lowercase enum string. v1 allowed set: `call`, `read`, `write`, `ref`, `unknown`.
* `path` and `line` are OPTIONAL but recommended when available.

`xrefs.jsonl` MUST be stable-sorted by:

1. `from_symbol_id`
2. `kind`
3. `to_symbol_id`
4. `path` (if present)
5. `line` (if present)

---

## 7) Stable ID rules

IDs are typed selectors exposed to users and used as join keys across artifacts. The ID format MUST be stable and validated strictly.

### 7.1 `SYMBOL_ID`

`SYMBOL_ID` MUST have the format:

* `sym:v2:<HEX_SHA256_96>`

Where `<HEX_SHA256_96>` is the first 96 bits (24 lowercase hex chars) of the SHA-256 of the **canonical symbol key**.

Canonical symbol key (string):

```text
lang=<lang>\n
path=<path>\n
kind=<kind>\n
qualified_name=<qualified_name>\n
disambiguator=<disambiguator>\n
```

Where:

* `lang`, `path`, `kind`, `qualified_name` come from the symbol record.
* `disambiguator` MUST be derived deterministically from the backend output and SHOULD be stable across line shifts. v1 rule:
  * If a backend provides a stable pattern for the def site (e.g., ctags `pattern`), use `pat_sha256=<HEX_SHA256(pattern)>`.
  * Else if a backend provides a signature, use `sig_sha256=<HEX_SHA256(signature)>`.
  * Else fall back to `line=<line>`.

This produces:

* deterministic IDs (same inputs → same IDs),
* disambiguation for overloads/duplicates,
* and avoids embedding raw, backend-specific strings into the ID.

---

## 8) Backend/toolchain policy (ctags baseline)

### 8.1 Ctags invocation

When using Universal Ctags as a backend, `kb` MUST:

* execute it in a locale-stable way (`LC_ALL=C`),
* use JSON output (stream or file) as the only input,
* treat non-zero exit as a hard failure for symbol generation.

The exact invocation flags and supported versions are part of the implementation, but the determinism requirements above are non-negotiable: a backend that yields nondeterministic output is not conforming.

### 8.2 Missing backends

If a required backend (symbols or deps) is missing and the corresponding artifact is required by config, `kb index regen` MUST fail non-zero with a structured error (see `docs/DESIGN.md` JSON error contract). Silent partial generation is not allowed for required artifacts.

---

## 9) Budgeting rules (packs, lists, and excerpts)

Any command that accepts `--max-bytes` or `--snippet-lines` MUST:

* apply budgets deterministically,
* have a deterministic include order,
* and apply a deterministic drop policy when budgets are exceeded.

The preferred global drop policy is:

1. include required “plan” metadata (changed paths, triggered obligations),
2. include module cards / facts,
3. include generated index entries,
4. include code excerpts last, dropping excerpts first when budgets are hit.

The exact include ordering per command is defined in the dev plans and must be documented as part of the implementation.
