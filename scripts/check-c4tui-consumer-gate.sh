#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
c4tui_source="${C4TUI_REPO:-${repo_root}/../c4tui}"

if [[ ! -f "${c4tui_source}/Cargo.toml" ]]; then
  echo "c4tui checkout not found at ${c4tui_source}" >&2
  echo "Set C4TUI_REPO=/path/to/c4tui and retry." >&2
  exit 2
fi

workdir="$(mktemp -d "${TMPDIR:-/tmp}/tui-kit-c4tui-gate.XXXXXX")"
cleanup() {
  rm -rf "${workdir}"
}
trap cleanup EXIT

rsync -a --delete \
  --exclude '.git' \
  --exclude 'target' \
  "${c4tui_source}/" "${workdir}/c4tui/"

cat >> "${workdir}/c4tui/Cargo.toml" <<EOF_PATCH

# Added by tui-kit/scripts/check-c4tui-consumer-gate.sh so c4tui's git
# dependency is resolved to this local checkout instead of the locked remote SHA.
[patch."https://github.com/scshafe/tui-kit"]
tui-kit = { path = "${repo_root}" }
EOF_PATCH

cd "${workdir}/c4tui"
cargo update -p tui-kit
cargo check --all-targets
cargo test
