# DP-0004 — Enforcement: pre-commit/CI gates and target-repo integration

## Summary

Implement the “commit-gated freshness” loop that makes kb-tool operationally useful:

* A target repo can install/invoke `kb`.
* Pre-commit and CI run deterministic checks that fail on stale/missing knowledge artifacts.
* The checks are defined once and reused across local and CI to prevent bypass.

This plan adds:

* `kb lint all`
* `kb obligations check --diff-source {staged|worktree|commit:<sha>}`
* hook/CI runner scripts
* a minimal “install” path for target repos

---

## Goals (must be true at end of DP-0004)

* `kb lint all` exists and validates:
  * that required configs exist and are parseable (`kb/config/*`),
  * that required generated artifacts exist and conform to v1 schemas (`kb/gen/*`),
  * that no persisted artifacts contain forbidden nondeterministic fields (timestamps, absolute paths).
* `kb obligations check` exists and fails commit/CI when obligations are unmet for the selected `--diff-source`.
* A pre-commit runner exists that runs the canonical gate sequence in the correct order:
  1. `kb index check --diff-source staged`
  2. `kb lint all`
  3. `kb obligations check --diff-source staged`
* A CI runner exists that executes the same sequence for the head commit under test.
* A target repo integration recipe exists (scripts + minimal documentation snippet).

---

## Non-goals

* No network calls and no remote backends.
* No automatic “doc gardening” agent in this phase.

---

## Decisions (locked)

### 1) Canonical gate sequence is single-source-of-truth

The exact gate sequence and flags MUST live in one place (a script or a single command) so local and CI cannot drift.

### 2) Obligations are deterministic, prefix-based rules

`kb obligations check` MUST evaluate obligations based only on:

* `kb/config/obligations.toml` rules (prefix match),
* the diff (changed paths) under the selected `--diff-source`,
* the presence/validity of required knowledge artifacts in the target repo,
* and whether required knowledge artifacts were updated in the same diff when mandated by policy.

No heuristics, no free text, no fuzzy matching.

### 3) “Satisfied” means “present, valid, and updated in the diff” in v1

In v1, an obligation is satisfied only when the required knowledge artifact:

* exists as-of the selected `--diff-source`,
* is valid (parseable, schema-valid where applicable),
* and is updated in the same diff when the obligation requires an in-session update.

Concrete rules (v1):

* `require_module_card = "<MODULE_ID>"` is satisfied iff:
  * `kb/atlas/modules/<MODULE_ID>.toml` exists, and
  * the diff contains `kb/atlas/modules/<MODULE_ID>.toml` as added or modified (rename counts as modified).
* `require_fact_types = ["..."]` is satisfied iff:
  * `kb/facts/facts.jsonl` exists and is updated in the diff (added or modified), and
  * as-of `--diff-source`, it contains at least one fact record of each required type.

This is intentionally strict: it forces in-session capture rather than allowing off-session doc gardening to “maybe” fix things later.

### 4) Session capsule obligation is always “must be added/updated in the diff”

If any triggered rule requires a session capsule, the diff MUST include at least one added/modified file under `kb/sessions/`.

This is the mechanism that enforces in-session capture for decision-heavy changes.

---

## `kb lint all`

### Inputs

Target repo root containing `kb/`.

### Validations (locked)

1. Parseability:
   * `kb/config/obligations.toml` must parse as TOML.
   * if present, `kb/config/tags.toml` and `kb/config/kb.toml` must parse as TOML.
2. Repo-boundedness:
   * any paths inside configs that represent repo paths must be normalized and must not contain `..` after normalization.
3. Generated artifacts exist:
   * `kb/gen/kb_meta.json` and `kb/gen/tree.jsonl` must exist.
   * `kb/gen/symbols.jsonl` and `kb/gen/deps.jsonl` must exist unless explicitly disabled via config (future).
4. Schema conformance:
   * each JSON/JSONL file must parse and conform to v1 schemas in `docs/SPECS.md`.
5. Forbidden fields:
   * reject absolute paths in any persisted artifacts,
   * reject timestamps/epoch fields if found (best-effort scanning for known keys like `epoch`, `timestamp`, `mtime`).
6. Optional overlays (validate if present):
   * parse all `kb/atlas/modules/*.toml` as TOML (if directory exists),
   * parse `kb/facts/facts.jsonl` as JSONL objects with at least `fact_id` and `type` fields (if file exists),
   * parse `kb/sessions/**/*.json` as JSON objects (if any exist).

Output:

* `--format json` emits one status object:
  ```json
  { "ok": true }
  ```
* failures emit the standard JSON error object.

---

## `kb obligations check`

### Purpose

Fail the commit/CI run if the diff triggers requirements that are missing or invalid in the kb artifacts.

### Algorithm (locked)

1. Compute `plan = kb plan diff --diff-source <...> --policy default --format json`.
2. For each required item in `plan.required`:
   * `module_cards[]`:
     * `kb/atlas/modules/<module_id>.toml` must exist, and
     * the diff must include `kb/atlas/modules/<module_id>.toml` as added or modified.
   * `fact_types[]`:
     * `kb/facts/facts.jsonl` must exist and be added or modified in the diff, and
     * it must contain at least one fact record of each required type as-of `--diff-source`.
   * `session_capsule == true`: the diff must include at least one path under `kb/sessions/` as added or modified.
3. If any requirement is unmet, emit a JSON error with details and exit non-zero.

### Output (`--format json`)

On success:

```json
{ "ok": true }
```

On failure, include detail keys:

* `missing_module_cards`: array of module ids (missing file)
* `missing_module_card_updates`: array of module ids (file exists but was not updated in the diff)
* `missing_fact_types`: array of types (no record present as-of diff-source)
* `missing_fact_updates`: array of types (facts file exists but was not updated in the diff)
* `missing_session_capsule`: boolean

---

## Hooks and CI runners

### Hook runner script

Provide `scripts/hook-pre-commit.sh` that runs the canonical sequence:

1. `kb index check --diff-source staged`
2. `kb lint all`
3. `kb obligations check --diff-source staged`

The script MUST:

* use `set -euo pipefail`,
* print minimal output on success,
* print actionable errors on failure (stderr).

### CI runner script

Provide `scripts/ci-check.sh` that runs the same sequence against the CI checkout state (typically `--diff-source worktree` or `commit:<sha>` depending on CI model). The flags must be deterministic and documented in the script header.

---

## Target repo integration recipe (locked)

Minimal integration steps for a target repo:

1. Install `kb` into PATH (vendored binary or `kb-tool/kb/bin/kb` wrapper).
2. Add `kb/` directory with required configs:
   * `kb/config/obligations.toml`
3. Run `kb index regen --scope all --diff-source worktree` once and commit `kb/gen/*`.
4. Install the pre-commit hook runner (or pre-commit config) so the canonical gate runs on every commit.

Documentation to add to a target repo’s `AGENTS.md`:

* `kb plan diff` and `kb pack diff` are the first-line navigation commands to reduce IO churn.
