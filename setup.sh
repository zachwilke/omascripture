#!/usr/bin/env bash
# Quick install: download the latest tested Linux release, install, and open it.
set -euo pipefail

if [[ "${1:-}" == "--help" ]]; then
  echo 'OmaScripture quick installer. Use --no-launch to install without opening the app.'
  echo 'Other options are passed to install.sh. No sudo or Rust toolchain is needed.'
  exit 0
fi
if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  echo 'Prebuilt releases support x86_64 Linux. See README.md for source installation.' >&2
  exit 1
fi
for tool in curl tar sha256sum mktemp; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Please install $tool, then run this installer again." >&2
    exit 1
  fi
done

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT
asset='omascripture-linux-x86_64.tar.gz'
base='https://github.com/zachwilke/omascripture/releases/latest/download'
echo 'Downloading OmaScripture…'
if ! curl --fail --location --silent --show-error --retry 2 "$base/$asset" -o "$work_dir/$asset"; then
  echo 'Could not download the release. Check your connection and try again.' >&2
  exit 1
fi
curl --fail --location --silent --show-error --retry 2 "$base/$asset.sha256" -o "$work_dir/$asset.sha256"
# Accept one checksum for precisely this archive; do not process arbitrary paths.
read -r digest checksum_name extra < "$work_dir/$asset.sha256"
if [[ ! "$digest" =~ ^[[:xdigit:]]{64}$ || "$checksum_name" != "$asset" || -n "${extra:-}" ]]; then
  echo 'The release checksum file is invalid. Nothing has been installed.' >&2
  exit 1
fi
if ! (cd "$work_dir" && printf '%s  %s\n' "$digest" "$asset" | sha256sum --check --status); then
  echo 'Download verification failed. Nothing has been installed; please try again.' >&2
  exit 1
fi
tar -xzf "$work_dir/$asset" -C "$work_dir"
# The desktop launcher is sufficient for a quick install. Advanced Omarchy
# integration remains available through the source installer's optional flags.
bash "$work_dir/omascripture/install.sh" --binary "$work_dir/omascripture/omascripture" --no-menu --launch "$@"
