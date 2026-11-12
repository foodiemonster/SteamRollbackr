# SteamRollbackr

A Linux desktop tool to roll back the Steam client to an earlier version and lock or unlock automatic updates.

Works on most Linux distributions (Bazzite, SteamOS, Mint, Ubuntu, Fedora, Arch) and supports both native and Flatpak Steam installations.

![SteamRollbackr](docs/screenshot.png)

## What It Does

- Detects your Steam installation and shows the currently installed client version.
- Shows whether automatic updates are currently locked or unlocked.
- Lists previous Steam client releases (newest first) from [SteamTracking](https://github.com/SteamTracking/SteamTracking).
- Rolls back Steam to the selected release and locks updates using `Steam.cfg` / `steam.cfg` and read-only permissions on `package/`.
- Unlocks updates when you want Steam to update normally again.

## Installation

1. Download `steamrollbackr-linux-x86_64.tar.gz` from the [Releases](https://github.com/foodiemonster/SteamRollbackr/releases) page.
2. Extract and run the installer:

```bash
tar -xzf steamrollbackr-linux-x86_64.tar.gz
./install.sh
```

This installs the binary to `~/.local/bin/steamrollbackr`, the icon to `~/.local/share/icons/hicolor/256x256/apps/steamrollbackr.png`, and the desktop entry to `~/.local/share/applications/steamrollbackr.desktop`.

### Building from Source

```bash
cargo build --release
./install.sh
```

## Usage

1. Open **SteamRollbackr** from your application menu (or run `~/.local/bin/steamrollbackr`).
2. Select a channel:
   - `steamrt64` (recommended for modern 64-bit Linux systems)
   - `ubuntu12` (legacy pre-November 2026 builds)
3. Pick a release from the list and click **Rollback & Lock Updates**.
4. To allow Steam to update again later, click **Unlock Updates**.
5. To lock your current version without rolling back, click **Lock Current Version**.

## Uninstall

```bash
rm -f ~/.local/bin/steamrollbackr \
      ~/.local/share/applications/steamrollbackr.desktop \
      ~/.local/share/icons/hicolor/256x256/apps/steamrollbackr.png
```

If Steam updates are still locked and you want to re-enable them, click **Unlock Updates** before uninstalling (or run `chmod u+w` on your Steam `package/` folder and remove `Steam.cfg` and `steam.cfg`).

## License

MIT
