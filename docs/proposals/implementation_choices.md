## What the repo says the implementation *must* optimize for

From `AGENTS.md` and `docs/MISSION.md` + `docs/DESIGN.md`, the non-negotiables driving implementation choices are:

* **Typed, deterministic CLI surface** (no free-text/NLP parameters; no fuzzy search; stable ordering; diff-friendly output).
* **Two churn reducers**

  * *Relevance churn*: avoid “dump a whole doc/file to find one fact.”
  * *IO churn*: avoid “10–50 tiny tool calls” to traverse directories/files.
* **Commit-gated freshness**: pre-commit + CI must fail if generated artifacts or “knowledge obligations” are stale/missing.
* **Local + repo-bounded**: no network; strict path normalization; no `..` escapes.

Those constraints eliminate a lot of otherwise-attractive “smart search” approaches and push you toward a small set of viable architectures.

---

## Decision: Option A is the baseline architecture

This repo is now explicitly basing implementation planning on:

* **Option A — Single-binary CLI + git/ctags baseline + JSONL artifacts**

Other options remain useful as **spikes** (e.g., short-lived prototype in Python, or premium tree-sitter backends), but they must preserve Option A’s **contract**.

**Where the hardened contract lives**

* `docs/DESIGN.md` is the high-level design intent + CLI shape.
* `docs/SPECS.md` is the decision-complete spec for determinism, formats, IDs, and “diff-source” semantics.
* `docs/dev_plans/INDEX.md` is the indexed execution plan.

---

## Decision axes and viable implementation choices (with Option A defaults)

### 1) Implementation language/runtime for the `kb` CLI

| Choice                 | What it looks like                                                                         | Pros (for your constraints)                                                                                              | Cons / risks                                                                                                                                                                            |
| ---------------------- | ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Rust single-binary** | `kb` is a static-ish executable; config + schemas in-repo; invokes `git`, optional `ctags` | Strong determinism controls; high performance for indexing/packing; easiest to vendor as a tool; good CLI typing (enums) | Higher implementation overhead; parsing + canonical serialization requires discipline; contributor friction if team is not Rust-native                                                  |
| **Go single-binary**   | Similar to Rust but with Go toolchain                                                      | Fast to ship; good cross-platform story; easy static builds; straightforward CLI typing                                  | Deterministic JSON/key ordering requires deliberate design; less ergonomic for complex schema generation/validation than Rust (still doable)                                            |
| **Python CLI package** | `kb` is a Python entrypoint; uses stdlib + a few pinned deps                               | Lowest time-to-MVP; easiest iteration on schemas and output formats; good for glue to existing tools                     | Runtime dependency (Python) in every target repo; performance can degrade on large repos; determinism can be accidentally violated by dict/order/version differences unless locked down |
| **Shell-script tool**  | A set of scripts that produce JSONL by orchestrating other CLIs                            | Minimal code; trivial bootstrap                                                                                          | Fails “typed interface” and “platform determinism” in practice (BSD/GNU drift, quoting, locale); hard to evolve without accumulating edge cases                                         |

**Option A default**

* Build `kb` as a **single-binary** program.
* Prefer **Rust** unless there is a strong organizational reason to standardize on Go.

If you do a Python prototype, treat it as a contract validator only: it must emit the same artifact formats and honor the same determinism rules described in `docs/SPECS.md`.

---

### 2) Source-of-truth for the file tree (determinism and staged-vs-worktree correctness)

| Choice                                      | Mechanism                                                             | Pros                                                                                                        | Cons / risks                                                                                                   |
| ------------------------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| **Git-tracked tree (recommended baseline)** | `git ls-files` (+ `git status --porcelain` for untracked when needed) | Deterministic ordering; respects ignore rules; aligns with “staged” diffs; avoids filesystem nondeterminism | Needs explicit handling for newly created but not-added files during worktree operations                       |
| Filesystem walk                             | `walkdir` / `filepath.Walk` / `os.walk`                               | Sees everything immediately                                                                                 | Ordering/ignore rules become a source of churn; harder to guarantee repo-bounded behavior with symlinks/mounts |

**Key design implication**
Given `--diff-source {staged|worktree|commit:<sha>}`, you want a **“read file as-of diff-source” abstraction**:

* **staged**: read blobs from the Git index (not the worktree) for correctness under pre-commit gating.
* **commit:<sha>**: read blobs from that commit.
* **worktree**: read filesystem (but still anchored to the Git-tracked path set, unless explicitly configured).

This pushes you toward a real implementation language (Rust/Go/Python), not shell.

---

### 3) Symbol extraction backend (`symbols.jsonl`) and stable `SYMBOL_ID`s

| Choice                                                 | Pros                                                                                                | Cons / risks                                                                              | When it’s viable                                                   |
| ------------------------------------------------------ | --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| **Universal Ctags JSON output**                        | Mature; multi-language; fast; externalizes parser complexity; easy to turn into stable IDs          | External dependency; occasional language inaccuracies; symbol “uses” are not first-class  | Best MVP for breadth and speed                                     |
| **Tree-sitter embedded**                               | High-quality structure; enables better incremental parsing; can derive symbols + some deps reliably | Engineering-heavy (grammars, bindings, version pinning); cross-platform build complexity  | Best if you have a narrow language set or very high accuracy needs |
| Language-specific compilers/LS (Swift, TS, Rust, etc.) | Potentially highest fidelity                                                                        | Often slow; can pull toolchains; may trigger downloads/build steps; operationally brittle | Only for targeted “premium extractors” after baseline is working   |

**Option A default**

* **MVP:** ctags JSON → stable `SYMBOL_ID`, plus deterministic sorting.
* **Later:** optional tree-sitter “backend” for specific languages where ctags is weak, but keep the *schema and IDs stable* (or introduce explicit versioning/migration).

**Hardening note**
To avoid “works on my machine” churn, you must explicitly define:

* supported ctags variants / minimum version,
* the exact invocation flags,
* and the behavior when a backend is missing (fail vs “disabled artifact”), in `docs/SPECS.md`.

---

### 4) Dependency graph extraction (`deps.jsonl`)

There are two viable philosophies:

#### A) **Best-effort, lightweight deps** (Option A default)

* Parse import/include statements with small, language-scoped parsers (avoid regex-only where practical).
* Keep `deps` semantics modest: “file/module A references module B,” not “full build graph.”
* Prefer **syntactic imports only** early (no TS path mapping, no SwiftPM target graph) to stay deterministic and local.

**Pros:** deterministic, local, fast, doesn’t trigger builds/downloads.
**Cons:** incomplete graphs; some ecosystems (TS path mapping, SwiftPM targets) are non-trivial.

#### B) **Build-tool-derived deps**

* Use `cargo metadata`, `go list`, `swift package describe`, etc.

**Pros:** more “true” build graph.
**Cons:** can be slow; can cause toolchain side effects; some commands may download deps (conflicts with “no network calls” operationally).

Most compatible with the repo’s constraints is **A**, with targeted opt-in enrichers later.

---

### 5) Storage format for artifacts (diffability vs query performance)

| Canonical store                                       | Pros                                                                 | Cons                                                            | Fit                                                                     |
| ----------------------------------------------------- | -------------------------------------------------------------------- | --------------------------------------------------------------- | ----------------------------------------------------------------------- |
| **JSONL (recommended for `tree/symbols/deps/xrefs`)** | Line-addressable; stable sorting; minimal diff churn; easy streaming | Requires careful schema discipline; joins require indexing step | Excellent for commit-tracked artifacts                                  |
| Pretty JSON                                           | Human-readable                                                       | Larger diffs; whole-file rewrites likely                        | Acceptable for small “meta” objects only                                |
| SQLite                                                | Fast queries; good for xrefs                                         | Binary diffs; hard to review; migration pain                    | Best as **derived cache** under `kb/cache/` (gitignored), not canonical |

This aligns with `docs/DESIGN.md`: **JSONL committed**, SQLite optional local acceleration.

---

### 6) “Pack” implementation (single-call retrieval without violating “no fuzzy search”)

You have two viable pack planners:

#### Option 1: **Selector-driven pack only** (strict)

* `kb pack selectors --path ... --module ... --symbol ... --max-bytes ...`
* Retrieval is purely by exact IDs + paths.

**Pros:** perfectly aligned with “no fuzzy search.”
**Cons:** less ergonomic until indexes are mature; agents must `list` then `pack`.

#### Option 2: **Diff-driven pack** (still typed; Option A default)

* `kb pack diff --diff-source ... --radius ...`
* “Relevance” is computed deterministically from the diff + dependency edges + obligations, not from free text.

**Pros:** major IO churn reduction with no NLP; maps directly to your enforcement model.
**Cons:** requires a minimally useful deps/xref signal to avoid under/over-inclusion.

Given the repo’s constraints, **diff-driven pack** is the best “single-call” workhorse. The output schema and determinism rules are specified in `docs/SPECS.md`; the implementation steps live in `docs/dev_plans/`.

---

### 7) Enforcement integration: pre-commit/prek hook vs bespoke hook

Two viable enforcement integrations:

| Approach                                                                                                 | Pros                                                               | Cons                                                                             |
| -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | -------------------------------------------------------------------------------- |
| **Pre-commit config adds local `kb` hooks** (`repo: local`, `language: system`, `pass_filenames: false`) | Declarative, standard, CI-friendly; easy for target repos to adopt | Requires ensuring `kb` is discoverable in PATH (or use repo-relative entry)      |
| **Bespoke hook script runs `kb index check`, `kb lint`, `kb obligations check`**                         | Fully controlled; no pre-commit dependency assumptions             | More one-off glue; less portable across teams already standardized on pre-commit |

**Option A default**
Prefer the **pre-commit-compatible** definition of checks (so CI and local converge), with a thin runner script for convenience. If this repo doesn’t yet contain the hook/config scaffolding, it is added as part of the dev plans rather than assumed to already exist.

---

## Composite implementation options (end-to-end)

### Implementation Option A — **Single-binary CLI + git/ctags baseline + JSONL artifacts** (selected)

* Language: **Rust** (single-binary).
* Tree: `git ls-files` (+ staged/worktree/commit diff-source reader).
* Symbols: ctags JSON → `symbols.jsonl`.
* Deps: lightweight import parsers (best-effort; syntactic imports only initially).
* Packs: diff-driven packs + selector packs.
* Enforcement: pre-commit/CI run `kb index check` and `kb obligations check`.

**Pros:** best determinism + adoption story; low runtime dependencies; scales to large repos.
**Cons:** higher up-front build cost; requires careful schema/versioning planning.

---

### Implementation Option B — **Python MVP, then harden** (allowed only as a spike)

* Python CLI implements the contract; uses `git` subprocess; uses `ctags` if available.
* Later, re-implement in Rust/Go **without changing** the on-disk format and CLI JSON outputs.

**Pros:** fastest iteration on the CLI contract, schemas, and deterministic formatting rules.
**Cons:** “prototype gravity” risk—teams may get stuck with Python runtime + performance limits; re-implementation requires strict compatibility discipline.

---

### Implementation Option C — **Tree-sitter-centric indexer from day one** (defer)

* Embed parsers and extract both symbols and deps structurally.

**Pros:** best structural fidelity long-term; supports richer xrefs.
**Cons:** heaviest engineering and maintenance; grammar/version pinning becomes part of your product surface.

---

## Hardening checklist: contracts to lock early (to prevent churn)

Before writing substantial code, lock these down in `docs/SPECS.md`:

1. **Canonical formats**: UTF-8, LF, JSON/JSONL canonicalization rules, stable list ordering.
2. **Artifact schemas + versioning**: minimal required fields, optional fields, schema evolution rules.
3. **Stable IDs**: `SYMBOL_ID` format + disambiguation rules (overloads, anonymous symbols).
4. **Toolchain policy**: ctags invocation and “missing backend” behavior; no environment-specific fields in committed artifacts.
5. **Diff-source semantics**: exact meaning of `staged`, `worktree`, `commit:<sha>`, and how blobs are read.
6. **Pack budgeting**: deterministic include order + drop policy when `--max-bytes` is hit.

These items are the difference between “a good idea” and “a reproducible tool that doesn’t churn diffs or drift across machines.”

---

## Concise reasoning narrative

Start from the explicit non-negotiables (typed ops, determinism, no network, commit-gated freshness, and IO+relevance churn targets). Those constraints eliminate shell and NLP-heavy approaches, and converge on: (1) a real CLI program with a staged/worktree/commit file reader, (2) deterministic, diff-friendly committed artifacts (JSONL), and (3) an indexing backend that ships early (git + ctags) while leaving room for targeted accuracy upgrades (tree-sitter) without changing the surface contract.
