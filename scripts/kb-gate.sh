#!/usr/bin/env bash
set -euo pipefail

DIFF_SOURCE="${1:-}"
if [[ -z "${DIFF_SOURCE}" ]]; then
  echo "usage: $(basename "$0") <diff-source>" >&2
  echo "  diff-source: staged | worktree | commit:<sha>" >&2
  exit 2
fi

kb index check --diff-source "${DIFF_SOURCE}" --format text >/dev/null
kb lint all --format text >/dev/null
kb obligations check --diff-source "${DIFF_SOURCE}" --format text >/dev/null
