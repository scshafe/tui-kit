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

metadata_path="${workdir}/c4tui-metadata.json"
cargo metadata --format-version 1 > "${metadata_path}"
python3 - "${metadata_path}" "${repo_root}" <<'PY'
import json
import pathlib
import sys

metadata = json.loads(pathlib.Path(sys.argv[1]).read_text())
expected = pathlib.Path(sys.argv[2]).resolve()
packages = [package for package in metadata["packages"] if package["name"] == "tui-kit"]
if len(packages) != 1:
    print(f"expected exactly one tui-kit package, found {len(packages)}", file=sys.stderr)
    sys.exit(1)

package = packages[0]
actual = pathlib.Path(package["manifest_path"]).parent.resolve()
if package.get("source") is not None or actual != expected:
    print(
        "c4tui is not using the local tui-kit checkout: "
        f"source={package.get('source')!r}, path={actual}, expected={expected}",
        file=sys.stderr,
    )
    sys.exit(1)
PY

cargo check --all-targets
cargo test
