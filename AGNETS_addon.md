# kb — agent recipe (repo inspection + in-session updates)

This repo is a **kb-enabled target repo** (it has a committed `kb/` root). Prefer `kb` commands over wide filesystem scans to minimize IO churn and keep updates commit-gated.

## 0) Preflight (fast)

- Verify generated artifacts are current:
  - `kb index check --diff-source worktree`
- If you are about to commit:
  - `kb plan diff --diff-source staged --policy default --format json`
  - `scripts/kb-gate.sh staged`

## 1) Discovery (typed; no fuzzy search)

- List what exists:
  - `kb list modules --format text`
  - `kb list tags --format text`
- Read a module card:
  - `kb describe module --id <MODULE_ID> --format json`
- Facts (discovery → exact lookup):
  - `kb list facts --format text`
  - `kb describe fact --id <FACT_ID> --format json`

## 2) “Single-call context” for code review / debugging

- For a change-set (preferred):
  - `kb pack diff --diff-source {staged|worktree} --format json`
- For exact selectors:
  - `kb pack selectors --module <MODULE_ID> --format json`
  - `kb pack selectors --path <PATH> --format json`

Notes (v1):

- `pack selectors --module <MODULE_ID>` expands the module card’s `entrypoints`/`edit_points` and includes `related_facts` automatically.
- `pack selectors --path <DIR/>` includes a bounded subtree under the directory prefix.

Then open only the specific files/line ranges you still need (use `sed-x` slices rather than dumping files).

## 3) In-session updates (what to edit when gates fail)

- Module cards: `kb/atlas/modules/<MODULE_ID>.toml`
- Facts: `kb/facts/facts.jsonl`
- Sessions:
  - `kb session init --id <SESSION_ID> [--tag <TAG>]...`
  - Edit the capsule to record decisions/pitfalls/verification (no absolute paths).
  - `kb session finalize --id <SESSION_ID> --diff-source staged --verification tests --verification lint`

## 4) Working in this repo (kb-tool)

If `kb` isn’t installed globally, use the local build:

- `cargo build -q`
- `PATH="$PWD/target/debug:$PATH" kb <command...>`
