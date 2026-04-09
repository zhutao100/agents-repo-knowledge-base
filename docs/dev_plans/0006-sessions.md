# DP-0006 — Session capsules (`kb session *`) and templates

## Summary

Implement session capsules as structured, repo-bounded records that capture “session-only truth” (pitfalls, ruled-out approaches, verification performed) in a way that can be enforced by `kb obligations check` when required.

This plan adds:

* `kb session init --id <SESSION_ID> [--tag <TAG>]...`
* `kb session finalize --id <SESSION_ID> --diff-source {staged|worktree|commit:<sha>} [--verification {tests|bench|repro|lint}]...`
* `kb session check --id <SESSION_ID>`
* a session template at `kb/templates/session.json` (optional; fallback to built-in)

---

## Goals (must be true at end of DP-0006)

* Session capsules can be created and validated deterministically with typed selectors.
* A session capsule can be required by obligations (DP-0004) and satisfied by adding/updating a `kb/sessions/**.json` file in the diff.
* Session capsules do not leak environment-specific data (absolute paths, usernames, hostnames).

---

## Non-goals

* No automatic population from free-text prompts.
* No network calls.

---

## Decisions (locked)

### 1) Session file location and naming (v1)

`kb session init` MUST create a capsule file at:

* `kb/sessions/<YYYY>/<MM>/<SESSION_ID>.json`

Where:

* `<YYYY>` and `<MM>` are derived from the local machine date at init time.
* `<SESSION_ID>` is validated and must match the filename stem exactly.

Valid `SESSION_ID` (v1):

* regex: `^[A-Za-z0-9][A-Za-z0-9_.-]*$`

### 2) Capsule schema (v1)

A session capsule is a single JSON object with these required fields (key order is significant):

```json
{
  "session_id": "PR-1234",
  "tags": [],
  "summary": "",
  "decisions": [],
  "pitfalls": [],
  "verification": [],
  "refs": []
}
```

Field rules:

* `tags`: stable-sorted unique array of strings.
* `summary`: may be multi-line; MUST use LF newlines when stored.
* `decisions`: array of concise strings (no enforced format in v1).
* `pitfalls`: array of concise strings (no enforced format in v1).
* `verification`: stable-sorted unique array of strings. Allowed values in v1: `tests`, `bench`, `repro`, `lint`.
* `refs`: stable-sorted unique array of repo-relative paths or IDs (no absolute paths).

No timestamps are required in v1.

### 3) Template selection

If `kb/templates/session.json` exists in the target repo, `kb session init` SHOULD use it as the skeleton (after validating it matches the v1 schema keys). Otherwise it MUST use a built-in template matching the schema above.

---

## Command behavior

### `kb session init`

Inputs:

* `--id <SESSION_ID>`
* optional `--tag <TAG>` (repeatable)

Behavior:

1. Validate `<SESSION_ID>`.
2. Determine output path `kb/sessions/<YYYY>/<MM>/<SESSION_ID>.json`.
3. Create parent directories if needed.
4. Write the template JSON object with:
   * `session_id` populated,
   * `tags` populated (stable-sorted unique),
   * other fields empty.
5. Fail with `INVALID_ARGUMENT` if the file already exists (no silent overwrite).

### `kb session finalize`

Inputs:

* `--id <SESSION_ID>`
* `--diff-source ...`
* optional `--verification ...` (repeatable from allowed set)

Behavior:

1. Locate the capsule file for `<SESSION_ID>` by searching under `kb/sessions/` for `<SESSION_ID>.json`.
   * If multiple matches exist, fail non-zero with an error listing candidate paths (stable-sorted).
2. Parse and validate the capsule JSON schema (required keys present).
3. Update:
   * merge `verification` entries (stable-sorted unique),
   * optionally append refs derived from the diff (e.g., top-level changed paths) without exceeding reasonable size (v1: at most 100 refs).
4. Write the updated JSON file deterministically (minified JSON, stable key order).

### `kb session check`

Inputs:

* `--id <SESSION_ID>`

Behavior:

* Locate, parse, and validate the capsule file.
* Exit 0 if valid, else non-zero JSON error.

---

## Acceptance tests (required)

* `session init` creates the expected file path and schema.
* `session init` fails when the file already exists.
* `session finalize` merges verification entries deterministically.
* `session check` rejects missing required keys.
* Session JSON never contains absolute paths (basic scan for `^/`-like patterns on macOS/Linux and drive prefixes on Windows).
