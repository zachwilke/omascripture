#!/bin/bash
# Remove OmaScripture from an Omarchy system. Keeps your downloaded
# translations, study packs, bookmarks and notes unless you pass --purge.

set -euo pipefail

INSTALL_ROOT="${OMASCRIPTURE_INSTALL_ROOT:-$HOME}"
PURGE=0
[[ "${1:-}" == "--purge" ]] && PURGE=1

MENU_FILE="$INSTALL_ROOT/.config/omarchy/extensions/omarchy-menu.jsonc"
BINDINGS_FILE="$INSTALL_ROOT/.config/hypr/bindings.lua"
SHELL_JSON="$INSTALL_ROOT/.config/omarchy/shell.json"

rm -f "$INSTALL_ROOT/.local/bin/omascripture"
rm -f "$INSTALL_ROOT/.local/share/omascripture/uninstall.sh"
rm -f "$INSTALL_ROOT/.local/share/applications/OmaScripture.desktop"
rm -f "$INSTALL_ROOT/.local/share/applications/io.github.zachwilke.OmaScripture.desktop"
rm -f "$INSTALL_ROOT/.local/share/icons/hicolor/scalable/apps/omascripture.svg"

if [[ -f "$MENU_FILE" ]]; then
  sed -i '/"learn\.bible"/d' "$MENU_FILE"
fi
if [[ -f "$BINDINGS_FILE" ]]; then
  sed -i '/-- OmaScripture (added by install.sh)/d; /tui = "omascripture"/d; /launch = "omascripture"/d' "$BINDINGS_FILE"
  command -v hyprctl >/dev/null 2>&1 && hyprctl reload >/dev/null 2>&1 || true
fi
if [[ -f "$SHELL_JSON" ]] && command -v python3 >/dev/null 2>&1 && grep -q omascripture.votd "$SHELL_JSON"; then
  python3 - "$SHELL_JSON" <<'PY'
import json, sys
path = sys.argv[1]
with open(path) as f:
    data = json.load(f)
layout = data.get("bar", {}).get("layout", {})
for section in layout.values():
    if isinstance(section, list):
        section[:] = [m for m in section if m.get("id") != "omascripture.votd"]
with open(path, "w") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
PY
fi

if [[ $PURGE -eq 1 ]]; then
  rm -rf "${XDG_DATA_HOME:-$INSTALL_ROOT/.local/share}/omascripture"
  echo "Removed OmaScripture and all its data."
else
  echo "Removed OmaScripture. Your data is still in ${XDG_DATA_HOME:-$INSTALL_ROOT/.local/share}/omascripture (use --purge to delete it)."
fi
