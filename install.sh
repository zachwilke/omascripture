#!/bin/bash
# Install OmaScripture on an Omarchy system.
#
#   ./install.sh                     build + install binary, desktop entry, icon, menu entry
#   ./install.sh --translation kjv   also download a Bible translation
#   ./install.sh --study basic       also download the core study packs (~60 MB)
#   ./install.sh --study all         also download every study pack (~200 MB)
#   ./install.sh --bar               add a verse-of-the-day widget to the Omarchy bar
#   ./install.sh --bind "SUPER + SHIFT + ALT + B"   add a Hyprland keybinding
#   ./install.sh --no-menu           skip the Omarchy menu entry
#
# Everything goes under ~/.local and ~/.config; nothing in /usr/share/omarchy is touched.

set -euo pipefail

BIND=""
TRANSLATION=""
STUDY=""
ADD_MENU=1
ADD_BAR=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bind) BIND="$2"; shift 2 ;;
    --translation|-t) TRANSLATION="$2"; shift 2 ;;
    --study) STUDY="$2"; shift 2 ;;
    --bar) ADD_BAR=1; shift ;;
    --no-menu) ADD_MENU=0; shift ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$HOME/.local/bin"
BIN="$BIN_DIR/omascripture"
APP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
MENU_FILE="$HOME/.config/omarchy/extensions/omarchy-menu.jsonc"
BINDINGS_FILE="$HOME/.config/hypr/bindings.lua"
SHELL_JSON="$HOME/.config/omarchy/shell.json"
SHELL_DEFAULT="${OMARCHY_PATH:-/usr/share/omarchy}/config/omarchy/shell.json"
APP_ID="io.github.zachwilke.OmaScripture"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required. Install a Rust toolchain first, e.g.:  omarchy install dev-env rust" >&2
  exit 1
fi

echo "==> Building release binary"
(cd "$HERE" && cargo build --release --quiet)

echo "==> Installing binary to $BIN"
mkdir -p "$BIN_DIR"
install -m 755 "$HERE/target/release/omascripture" "$BIN.next"
mv -f "$BIN.next" "$BIN"

echo "==> Installing icon"
mkdir -p "$ICON_DIR"
install -m 644 "$HERE/assets/omascripture.svg" "$ICON_DIR/omascripture.svg"

echo "==> Installing desktop entry"
mkdir -p "$APP_DIR"
rm -f "$APP_DIR/OmaScripture.desktop"
cat > "$APP_DIR/$APP_ID.desktop" <<EOF
[Desktop Entry]
Version=1.0
Name=OmaScripture
Comment=Read and study the Bible: translations, interlinear, commentaries, dictionaries
Exec=$BIN --gui
Terminal=false
Type=Application
Icon=omascripture
Categories=Education;Literature;
Keywords=Bible;Scripture;Verse;Study;Greek;Hebrew;Commentary;
StartupNotify=true
StartupWMClass=$APP_ID
EOF
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APP_DIR" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

if [[ $ADD_MENU -eq 1 ]]; then
  echo "==> Adding Omarchy menu entry (Learn → Bible)"
  mkdir -p "$(dirname "$MENU_FILE")"
  if [[ ! -f "$MENU_FILE" ]]; then
    printf '{\n}\n' > "$MENU_FILE"
  fi
  if grep -q '"learn.bible"' "$MENU_FILE"; then
    echo "    updating existing launcher to the desktop app"
    sed -i 's/omarchy-launch-or-focus-tui omascripture/omarchy-launch-or-focus io.github.zachwilke.OmaScripture omascripture/g; s/org.omarchy.omascripture/io.github.zachwilke.OmaScripture/g' "$MENU_FILE"
  else
    ENTRY='  "learn.bible": {"icon":"","label":"Bible","description":"OmaScripture: read and study the Bible","aliases":["bible","scripture","omascripture"],"action":"omarchy-launch-or-focus io.github.zachwilke.OmaScripture omascripture"},'
    LAST=$(grep -n '^}' "$MENU_FILE" | tail -1 | cut -d: -f1)
    if [[ -n "$LAST" ]]; then
      sed -i "${LAST}i\\
$ENTRY" "$MENU_FILE"
    else
      printf '%s\n' "$ENTRY" >> "$MENU_FILE"
    fi
  fi
fi

if [[ -n "$BIND" ]]; then
  echo "==> Adding Hyprland binding: $BIND"
  if grep -q 'launch = "omascripture"' "$BINDINGS_FILE" 2>/dev/null; then
    echo "    a binding already exists in $BINDINGS_FILE, skipping"
  else
    printf '\n-- OmaScripture (added by install.sh)\no.bind("%s", "Bible", { launch = "omascripture", focus = "io.github.zachwilke.OmaScripture" })\n' "$BIND" >> "$BINDINGS_FILE"
    if command -v hyprctl >/dev/null 2>&1; then
      hyprctl reload >/dev/null 2>&1 || true
      ERRORS=$(hyprctl configerrors 2>/dev/null || true)
      if [[ -n "$ERRORS" && "$ERRORS" != "no errors" ]]; then
        echo "    Hyprland reported config errors:"; echo "$ERRORS"
      fi
    fi
  fi
fi

if [[ $ADD_BAR -eq 1 ]]; then
  echo "==> Adding verse-of-the-day widget to the Omarchy bar"
  if ! command -v python3 >/dev/null 2>&1; then
    echo "    python3 is needed to edit shell.json; skipping" >&2
  else
    mkdir -p "$(dirname "$SHELL_JSON")"
    if [[ ! -f "$SHELL_JSON" && -f "$SHELL_DEFAULT" ]]; then
      cp "$SHELL_DEFAULT" "$SHELL_JSON"
    fi
    python3 - "$SHELL_JSON" "$BIN" <<'PY'
import json, sys
path, binary = sys.argv[1], sys.argv[2]
try:
    with open(path) as f:
        data = json.load(f)
except (OSError, ValueError):
    data = {"version": 1}
bar = data.setdefault("bar", {})
layout = bar.setdefault("layout", {})
right = layout.setdefault("right", [])
if any(m.get("id") == "omascripture.votd" for m in right):
    print("    already present, skipping")
else:
    right.insert(0, {
        "id": "omascripture.votd",
        "type": "command",
        "exec": f"{binary} --votd-bar",
        "interval": 3600,
        "tooltip": "Verse of the day",
        "onClick": "omarchy-launch-or-focus io.github.zachwilke.OmaScripture omascripture",
    })
    with open(path, "w") as f:
        json.dump(data, f, indent=2)
        f.write("\n")
    print("    added; the bar reloads shell.json automatically")
PY
  fi
fi

if [[ -n "$TRANSLATION" ]]; then
  echo "==> Downloading translation: $TRANSLATION"
  "$BIN" --download "$TRANSLATION"
fi

case "$STUDY" in
  "") ;;
  all)
    echo "==> Downloading every study pack (this is about 200 MB)"
    "$BIN" --install all
    ;;
  basic)
    echo "==> Downloading the core study packs"
    for id in crossrefs interlinear-nt lexicon-greek mhc easton; do
      "$BIN" --install "$id"
    done
    ;;
  *)
    echo "==> Downloading study packs: $STUDY"
    for id in ${STUDY//,/ }; do
      "$BIN" --install "$id"
    done
    ;;
esac

cat <<EOF

Done. Launch OmaScripture with any of:
  omascripture                       (desktop app)
  omascripture "John 3:16"           (open at a reference)
  omascripture --tui                 (optional terminal interface)
  Super+Space → Learn → Bible        (Omarchy menu)
  App launcher → OmaScripture

Use the book navigator, Study sidebar, and Settings to make it your own.
EOF
if ! echo "$PATH" | tr ':' '\n' | grep -qx "$BIN_DIR"; then
  echo
  echo "Note: $BIN_DIR is not on your PATH; the desktop entry and menu still work, but add it for terminal use."
fi
