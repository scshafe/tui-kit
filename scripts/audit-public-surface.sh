#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/audit-public-surface.sh [--c4tui PATH]

Audits tui-kit's public type surface. A public type-like item passes when it
has a real reference in c4tui/src, tui-kit/tests, tui-kit/examples, or a
documented retention reason in docs/public-surface-retention.txt.
USAGE
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
c4tui_dir="${repo_root}/../c4tui"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --c4tui)
      if [ "$#" -lt 2 ]; then
        echo "error: --c4tui requires a path" >&2
        exit 2
      fi
      c4tui_dir="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if ! command -v rg >/dev/null 2>&1; then
  echo "error: ripgrep (rg) is required" >&2
  exit 2
fi

if [ ! -d "${c4tui_dir}/src" ]; then
  echo "error: c4tui src directory not found: ${c4tui_dir}/src" >&2
  exit 2
fi

policy_file="${repo_root}/docs/public-surface-retention.txt"
if [ ! -f "${policy_file}" ]; then
  echo "error: missing retention policy file: ${policy_file}" >&2
  exit 2
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

{
  rg --no-heading --glob '*.rs' '^[[:space:]]*pub (struct|enum|trait|type) [A-Za-z_][A-Za-z0-9_]*' "${repo_root}/src" \
    | sed -E 's/.*pub (struct|enum|trait|type) ([A-Za-z_][A-Za-z0-9_]*).*/\2/'
} | sort -u > "${tmp_dir}/public-items"

reference_roots=("${c4tui_dir}/src")
[ -d "${repo_root}/tests" ] && reference_roots+=("${repo_root}/tests")
[ -d "${repo_root}/examples" ] && reference_roots+=("${repo_root}/examples")

missing=()
while IFS= read -r item; do
  [ -n "${item}" ] || continue

  if rg --quiet --fixed-strings --glob '*.rs' "${item}" "${reference_roots[@]}"; then
    continue
  fi

  if rg --quiet "^${item}[[:space:]]+-[[:space:]]+" "${policy_file}"; then
    continue
  fi

  missing+=("${item}")
done < "${tmp_dir}/public-items"

if [ "${#missing[@]}" -gt 0 ]; then
  echo "Public tui-kit items without a c4tui/test/example reference or documented retention reason:" >&2
  printf '  %s\n' "${missing[@]}" >&2
  echo >&2
  echo "Add a real consumer, a test/example reference, or a reason to ${policy_file}." >&2
  exit 1
fi

echo "public surface audit passed ($(wc -l < "${tmp_dir}/public-items" | tr -d ' ') items checked)"
