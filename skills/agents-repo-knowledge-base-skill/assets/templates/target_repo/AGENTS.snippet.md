## Repo navigation (use kb first)

This repo is **kb-enabled** (it has a committed `kb/` root). Prefer `kb` commands over wide filesystem scans to reduce IO churn and keep updates commit-gated.

- Install (latest release): `bash kb/tooling/install_kb.sh` (optional pin: `KB_TOOL_TAG=vX.Y.Z bash kb/tooling/install_kb.sh`)
- Recipe: `kb/AGENTS_kb.md`
- Quick start:
  - `kb list modules --format text`
  - `kb describe module --id <MODULE_ID> --format json`
  - `kb pack selectors --module <MODULE_ID> --format json`
  - `kb list facts --format text`
  - `kb describe fact --id <FACT_ID> --format json`

Only after `kb` output indicates likely files/symbols should you open code files directly.
