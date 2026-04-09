#!/usr/bin/env bash
set -euo pipefail

# CI checkouts typically have the head commit in the working tree.
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
"${SCRIPT_DIR}/kb-gate.sh" worktree
