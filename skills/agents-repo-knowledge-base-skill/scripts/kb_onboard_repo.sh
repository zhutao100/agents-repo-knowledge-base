#!/usr/bin/env bash
set -euo pipefail

# Bootstrap kb-tool into a target repo.
#
# This script is designed to be run by agents: typed flags only, idempotent writes.

usage() {
  cat >&2 <<'USAGE'
usage: kb_onboard_repo.sh --repo <PATH> [--kb-bin <PATH>] [--kb-tag <TAG>] [--install-ci true|false]

required:
  --repo <PATH>        Path to a git repository (any subdir).

optional:
  --kb-bin <PATH>      Path to an existing kb binary.
  --kb-tag <TAG>       kb-tool release tag like v0.2.1 (defaults to latest release).
  --install-ci <bool>  Install GitHub Actions workflow (.github/workflows/kb-ci.yml). Default: false.

behavior:
  - Installs a local kb binary at <repo>/.kb-tool/bin/kb if needed (downloads latest GitHub release).
  - Creates kb/ skeleton and generates kb/gen/*.
  - Installs kb/tooling/* (kb gate + pre-commit + CI helpers).
  - Installs kb/AGENTS_kb.md and inserts/updates a marker-bounded snippet in AGENTS.md.
USAGE
}

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
TEMPLATES_DIR="${SKILL_DIR}/assets/templates/target_repo"

repo_path=""
kb_bin_arg=""
kb_tag_arg=""
install_ci="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)
      repo_path="${2:-}"; shift 2 ;;
    --kb-bin)
      kb_bin_arg="${2:-}"; shift 2 ;;
    --kb-tag)
      kb_tag_arg="${2:-}"; shift 2 ;;
    --install-ci)
      install_ci="${2:-}"; shift 2 ;;
    -h|--help)
      usage; exit 0 ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if [[ -z "${repo_path}" ]]; then
  echo "error: --repo is required" >&2
  usage
  exit 2
fi

if [[ -n "${kb_tag_arg}" ]] && [[ ! "${kb_tag_arg}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: --kb-tag must look like vX.Y.Z (got: ${kb_tag_arg})" >&2
  exit 2
fi

if [[ "${install_ci}" != "true" && "${install_ci}" != "false" ]]; then
  echo "error: --install-ci must be true or false" >&2
  exit 2
fi

need_cmd() {
  local c="$1"
  if ! command -v "${c}" >/dev/null 2>&1; then
    echo "error: required command not found in PATH: ${c}" >&2
    exit 2
  fi
}

need_cmd git
need_cmd ctags
need_cmd python3
if [[ -z "${kb_bin_arg}" ]]; then
  need_cmd curl
  need_cmd tar
fi

repo_root="$(cd "${repo_path}" && git rev-parse --show-toplevel 2>/dev/null)"
if [[ -z "${repo_root}" ]]; then
  echo "error: not a git repository: ${repo_path}" >&2
  exit 2
fi

resolve_kb_bin() {
  local repo_root="$1"

  if [[ -x "${repo_root}/.kb-tool/bin/kb" ]]; then
    echo "${repo_root}/.kb-tool/bin/kb"
    return 0
  fi

  mkdir -p "${repo_root}/.kb-tool/bin"

  if [[ -n "${kb_bin_arg}" ]]; then
    if [[ ! -x "${kb_bin_arg}" ]]; then
      echo "error: --kb-bin is not executable: ${kb_bin_arg}" >&2
      exit 2
    fi
    cp "${kb_bin_arg}" "${repo_root}/.kb-tool/bin/kb"
    chmod +x "${repo_root}/.kb-tool/bin/kb"
    echo "${repo_root}/.kb-tool/bin/kb"
    return 0
  fi

  pushd "${repo_root}" >/dev/null
  if [[ -n "${kb_tag_arg}" ]]; then
    KB_TOOL_TAG="${kb_tag_arg}" bash kb/tooling/install_kb.sh
  else
    bash kb/tooling/install_kb.sh
  fi
  popd >/dev/null

  if [[ ! -x "${repo_root}/.kb-tool/bin/kb" ]]; then
    echo "error: kb install did not produce an executable at ${repo_root}/.kb-tool/bin/kb" >&2
    echo "hint: rerun with --kb-bin /path/to/kb if network access is restricted" >&2
    exit 2
  fi

  echo "${repo_root}/.kb-tool/bin/kb"
}

install_kb_skeleton() {
  local repo_root="$1"

  mkdir -p "${repo_root}/kb/config" \
           "${repo_root}/kb/gen" \
           "${repo_root}/kb/templates" \
           "${repo_root}/kb/atlas/modules" \
           "${repo_root}/kb/facts" \
           "${repo_root}/kb/sessions" \
           "${repo_root}/kb/cache" \
           "${repo_root}/kb/.tmp" \
           "${repo_root}/kb/tooling"

  if [[ ! -f "${repo_root}/kb/config/obligations.toml" ]]; then
    cp "${TEMPLATES_DIR}/kb/config/obligations.toml" "${repo_root}/kb/config/obligations.toml"
  fi

  if [[ ! -f "${repo_root}/kb/config/tags.toml" ]]; then
    cp "${TEMPLATES_DIR}/kb/config/tags.toml" "${repo_root}/kb/config/tags.toml"
  fi

  if [[ ! -f "${repo_root}/kb/templates/session.json" ]]; then
    cp "${TEMPLATES_DIR}/kb/templates/session.json" "${repo_root}/kb/templates/session.json"
  fi
}

install_tooling_files() {
  local repo_root="$1"
  mkdir -p "${repo_root}/kb/tooling"

  for f in install_kb.sh kb-gate.sh kb-pre-commit.sh kb-ci-check.sh; do
    if [[ ! -f "${repo_root}/kb/tooling/${f}" ]]; then
      cp "${TEMPLATES_DIR}/kb/tooling/${f}" "${repo_root}/kb/tooling/${f}"
      chmod +x "${repo_root}/kb/tooling/${f}"
    fi
  done
}

install_agent_recipe() {
  local repo_root="$1"

  if [[ ! -f "${repo_root}/kb/AGENTS_kb.md" ]]; then
    cp "${TEMPLATES_DIR}/kb/AGENTS_kb.md" "${repo_root}/kb/AGENTS_kb.md"
  fi
}

install_ci_workflow() {
  local repo_root="$1"
  if [[ "${install_ci}" != "true" ]]; then
    return 0
  fi

  mkdir -p "${repo_root}/.github/workflows"
  if [[ ! -f "${repo_root}/.github/workflows/kb-ci.yml" ]]; then
    cp "${TEMPLATES_DIR}/.github/workflows/kb-ci.yml" "${repo_root}/.github/workflows/kb-ci.yml"
  fi
}

ensure_gitignore() {
  local repo_root="$1"
  local path="${repo_root}/.gitignore"
  if [[ ! -f "${path}" ]]; then
    touch "${path}"
  fi

  add_line() {
    local line="$1"
    if ! grep -Fq "${line}" "${path}" 2>/dev/null; then
      printf '\n%s\n' "${line}" >>"${path}"
    fi
  }

  add_line "# kb-tool (local-only)"
  add_line ".kb-tool/"
  add_line "kb/cache/"
  add_line "kb/.tmp/"
}

patch_agents_md() {
  local repo_root="$1"
  local agents_path="${repo_root}/AGENTS.md"
  local snippet_path="${TEMPLATES_DIR}/AGENTS.snippet.md"

  if [[ ! -f "${agents_path}" ]]; then
    cat >"${agents_path}" <<'HDR'
# Agent notes

This repository is kb-enabled.
HDR
  fi

  python3 - "${agents_path}" "${snippet_path}" <<'PY'
import pathlib
import sys

agents_path = pathlib.Path(sys.argv[1])
snippet_path = pathlib.Path(sys.argv[2])

begin = "<!-- kb-tool:begin -->\n"
end = "<!-- kb-tool:end -->\n"

snippet = snippet_path.read_text(encoding="utf-8")
block = begin + snippet.rstrip() + "\n" + end

text = agents_path.read_text(encoding="utf-8")

if begin in text and end in text:
    pre, rest = text.split(begin, 1)
    _, post = rest.split(end, 1)
    new_text = pre + block + post
else:
    new_text = text.rstrip() + "\n\n" + block

agents_path.write_text(new_text, encoding="utf-8")
PY
}

install_pre_commit_hook() {
  local repo_root="$1"

  if [[ -f "${repo_root}/.pre-commit-config.yaml" ]]; then
    # Prefer wiring kb gate as a pre-commit hook when the repo already uses pre-commit/prek.
    python3 - "${repo_root}/.pre-commit-config.yaml" <<'PY'
import pathlib
import re
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text(encoding="utf-8")

if re.search(r"(?m)^\\s*-\\s*id:\\s*kb-gate\\s*$", text):
    sys.exit(0)

hook = (
    "      - id: kb-gate\n"
    "        name: kb gate (staged)\n"
    "        language: system\n"
    "        entry: bash kb/tooling/kb-pre-commit.sh\n"
    "        pass_filenames: false\n"
    "        always_run: true\n"
    "        require_serial: true\n"
)

# Always append a new local repo block at the end so the kb gate runs last.
block = "\n\n  - repo: local\n    hooks:\n" + hook
path.write_text(text.rstrip() + block, encoding="utf-8")
PY
    return 0
  fi

  local hooks_path
  hooks_path="$(git -C "${repo_root}" config --get core.hooksPath || true)"

  local hook_dir
  if [[ -z "${hooks_path}" ]]; then
    hooks_path=".githooks"
    git -C "${repo_root}" config core.hooksPath "${hooks_path}"
    hook_dir="${repo_root}/${hooks_path}"
  else
    if [[ "${hooks_path}" = /* ]]; then
      hook_dir="${hooks_path}"
    else
      hook_dir="${repo_root}/${hooks_path}"
    fi
  fi

  mkdir -p "${hook_dir}"

  local hook_file="${hook_dir}/pre-commit"
  local prev_hook=""

  if [[ -f "${hook_file}" ]] && ! grep -Fq "kb-tool hook wrapper" "${hook_file}" 2>/dev/null; then
    prev_hook="${hook_dir}/pre-commit.prev"
    local n=1
    while [[ -e "${prev_hook}" ]]; do
      n=$((n + 1))
      prev_hook="${hook_dir}/pre-commit.prev${n}"
    done
    mv "${hook_file}" "${prev_hook}"
    chmod +x "${prev_hook}" 2>/dev/null || true
  fi

  cat >"${hook_file}" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail

# kb-tool hook wrapper
# - runs any previous pre-commit hook if it was present
# - then runs the kb pre-commit entrypoint (staged)

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)"
hook_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# Run prior hook(s), if present.
for prev in "${hook_dir}"/pre-commit.prev*; do
  if [[ -x "${prev}" ]]; then
    "${prev}"
  fi
done

"${repo_root}/kb/tooling/kb-pre-commit.sh"
HOOK

  chmod +x "${hook_file}"
}

run_initial_index() {
  local repo_root="$1"
  local kb_bin="$2"

  pushd "${repo_root}" >/dev/null
  "${kb_bin}" index regen --scope all --diff-source worktree --format text >/dev/null
  "${kb_bin}" index check --diff-source worktree --format text >/dev/null
  "${kb_bin}" lint all --format text >/dev/null
  popd >/dev/null
}

# Do work
install_kb_skeleton "${repo_root}"
install_tooling_files "${repo_root}"
install_agent_recipe "${repo_root}"
install_ci_workflow "${repo_root}"
ensure_gitignore "${repo_root}"
patch_agents_md "${repo_root}"
install_pre_commit_hook "${repo_root}"

kb_bin="$(resolve_kb_bin "${repo_root}")"
run_initial_index "${repo_root}" "${kb_bin}"

cat >&2 <<EOF_SUMMARY
ok: kb onboarded

repo: ${repo_root}
kb:   ${kb_bin}

next steps (target repo):
  - review the new/updated files
  - git add kb/ AGENTS.md .gitignore
  - git commit -m "chore: enable kb tool"

notes:
  - obligations.toml is installed empty by default; add narrow rules as you map modules.
  - derived paths are ignored: .kb-tool/, kb/cache/, kb/.tmp/
EOF_SUMMARY
