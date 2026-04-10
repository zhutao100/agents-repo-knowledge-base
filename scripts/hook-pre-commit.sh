#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)"
cd "${repo_root}"

# Mechanical self-heal:
# If `kb/gen/*` is stale for the staged set, regenerate and auto-stage it so the
# commit gate remains hassle-free.
if ! kb index check --diff-source staged --format text >/dev/null 2>&1; then
  if ! command -v ctags >/dev/null 2>&1; then
    echo "error: ctags not found in PATH (required for kb index regen)" >&2
    exit 2
  fi

  kb index regen --scope all --diff-source staged --format text >/dev/null
  git add kb/gen
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
"${SCRIPT_DIR}/kb-gate.sh" staged
