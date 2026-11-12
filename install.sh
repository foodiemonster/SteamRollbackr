#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_SRC="$DIR/steamrollbackr"
[ -f "$BIN_SRC" ] || BIN_SRC="$DIR/target/release/steamrollbackr"

ICON_SRC="$DIR/steamrollbackr.png"
[ -f "$ICON_SRC" ] || ICON_SRC="$DIR/packaging/steamrollbackr.png"

DESKTOP_SRC="$DIR/steamrollbackr.desktop"
[ -f "$DESKTOP_SRC" ] || DESKTOP_SRC="$DIR/packaging/steamrollbackr.desktop"

install -Dm755 "$BIN_SRC" "$HOME/.local/bin/steamrollbackr"
install -Dm644 "$ICON_SRC" "$HOME/.local/share/icons/hicolor/256x256/apps/steamrollbackr.png"
install -Dm644 "$DESKTOP_SRC" "$HOME/.local/share/applications/steamrollbackr.desktop"

# Use full path in Exec= so GUI launchers work even if ~/.local/bin isn't in desktop PATH
sed -i "s|^Exec=.*|Exec=$HOME/.local/bin/steamrollbackr|" "$HOME/.local/share/applications/steamrollbackr.desktop"

update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

echo "Installed SteamRollbackr! You can now launch it from your application menu."

