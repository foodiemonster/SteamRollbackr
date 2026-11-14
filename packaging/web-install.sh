#!/usr/bin/env bash
set -euo pipefail

REPO="foodiemonster/SteamRollbackr"
TARBALL_URL="https://github.com/$REPO/releases/latest/download/steamrollbackr-linux-x86_64.tar.gz"

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$HOME/.local/bin/steamrollbackr" \
          "$HOME/.local/share/applications/steamrollbackr.desktop" \
          "$HOME/.local/share/icons/hicolor/256x256/apps/steamrollbackr.png"
    update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
    echo "Uninstalled SteamRollbackr."
    exit 0
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

echo "Downloading latest SteamRollbackr release..."
curl -fsSL "$TARBALL_URL" -o "$TMP_DIR/steamrollbackr.tar.gz"

echo "Extracting and installing..."
tar -xzf "$TMP_DIR/steamrollbackr.tar.gz" -C "$TMP_DIR"
bash "$TMP_DIR/install.sh"

