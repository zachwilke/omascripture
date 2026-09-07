#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_dir="${1:-$repo_dir/dist}"
mkdir -p "$output_dir"
output_dir="$(cd "$output_dir" && pwd)"
package_dir="$(mktemp -d)"
trap 'rm -rf "$package_dir"' EXIT
mkdir -p "$package_dir/omascripture/assets"
install -m 755 "$repo_dir/target/release/omascripture" "$package_dir/omascripture/omascripture"
install -m 755 "$repo_dir/install.sh" "$repo_dir/uninstall.sh" "$package_dir/omascripture/"
install -m 644 "$repo_dir/assets/omascripture.svg" "$package_dir/omascripture/assets/"
install -m 644 "$repo_dir/LICENSE" "$repo_dir/README.md" "$package_dir/omascripture/"
archive='omascripture-linux-x86_64.tar.gz'
tar -czf "$output_dir/$archive" -C "$package_dir" omascripture
(cd "$output_dir" && sha256sum "$archive" > "$archive.sha256")
echo "Created $output_dir/$archive"
