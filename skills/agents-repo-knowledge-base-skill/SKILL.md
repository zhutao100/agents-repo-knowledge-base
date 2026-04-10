---
name: agents-repo-knowledge-base-skill
version: 0.3.0
description: Bootstrap the kb (repo knowledge base) workflow into a target git repository.
inputs:
  - name: repo
    type: path
    required: true
    description: Path to a git repository (any subdir is fine; it will resolve to the repo root).
  - name: kb_bin
    type: path
    required: false
    description: Optional path to an existing kb binary. If omitted, the skill installs kb from the latest GitHub release.
  - name: kb_tag
    type: string
    required: false
    description: Optional kb-tool release tag like v0.2.1. If omitted, installs the latest release.
  - name: install_ci
    type: enum
    required: false
    default: "false"
    values: ["true", "false"]
    description: Whether to install a GitHub Actions workflow that runs the kb gate.
---

# agents-repo-knowledge-base-skill

## What this skill does

This skill performs a **fresh onboarding** of the `kb` tool into a target repository, producing a **committed, deterministic repo knowledge base** under `kb/` and enabling **commit-gated freshness**.

The goal is to reduce:
- **IO churn** (many small file reads/tool calls) by using single-call `kb pack`/`kb describe` operations.
- **Relevance churn** (large free-text scans) by keeping repo knowledge as structured, typed artifacts.

## Requirements

On the machine running the onboarding:

- `git`
- `ctags` (Universal Ctags recommended)
- `curl` + `tar` (for downloading the release binary)
- `python3`

## One-command onboarding

```bash
agents-repo-knowledge-base-skill/scripts/kb_onboard_repo.sh --repo <TARGET_REPO_PATH>
```

Optional:

```bash
agents-repo-knowledge-base-skill/scripts/kb_onboard_repo.sh \
  --repo <TARGET_REPO_PATH> \
  --kb-bin </path/to/kb> \
  --kb-tag vX.Y.Z \
  --install-ci true
```

## What to commit after onboarding

In the target repo, you should commit:

- `kb/config/obligations.toml`
- `kb/templates/session.json` (if you keep the provided template)
- `kb/gen/*` (generated deterministically)
- `kb/tooling/install_kb.sh`, `kb/tooling/kb-gate.sh`, `kb/tooling/kb-pre-commit.sh`, `kb/tooling/kb-ci-check.sh`
- `AGENTS.md` (kb navigation snippet)
- `kb/AGENTS_kb.md` (typed kb recipe)

If you ran with `--install-ci true`, also commit:

- `.github/workflows/kb-ci.yml`

You should *not* commit:

- `kb/cache/`
- `kb/.tmp/`
- `.kb-tool/` (local kb binary cache)

## First agent workflow in a kb-enabled repo

- Discovery:
  - `kb list modules --format text`
  - `kb describe module --id <MODULE_ID> --format json`
  - `kb list facts --format text`
- Single-call context for current work:
  - `kb plan diff --diff-source worktree --format text`
  - `kb pack diff --diff-source worktree --radius 1 --max-bytes 120000 --snippet-lines 80 --format text`

## References

- `references/onboarding_playbook.md` — detailed steps and expected diffs.
- `references/obligations_and_modules.md` — how to design `obligations.toml` + module cards without heuristics.
- `references/troubleshooting.md` — common failure modes (ctags/network/path).
