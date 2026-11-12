# SteamRollbackr

A lightweight, distro-agnostic Linux GUI utility to inspect your Steam client version, browse historical Steam client releases, roll back to a specific version, and lock or unlock automatic Steam updates.

Works across **Bazzite**, **SteamOS**, **Linux Mint**, **Ubuntu**, **Fedora**, **Arch**, and **Flatpak Steam** installations.

## Features

- **Distro-Agnostic Detection**: Automatically locates Steam across `~/.local/share/Steam`, `~/.steam/steam`, `~/.steam/root`, and Flatpak (`~/.var/app/com.valvesoftware.Steam/.local/share/Steam`).
- **Version & Lock Status**: Displays the currently installed Steam client build ID & timestamp, plus whether updates are **Locked**, **Partially Locked**, or **Unlocked**.
- **Release History Browser**: Fetches historical `steam_client_steamrt64` and `steam_client_ubuntu12` manifests from [SteamTracking](https://github.com/SteamTracking/SteamTracking) sorted newest-first, with search filtering.
- **One-Click Rollback & Lock**:
  - Serves the chosen manifest locally via `-overridepackageurl` while redirecting package downloads to Valve's CDN.
  - Cleans conflicting manifests and `slsteam-moon` safe-mode state.
  - Locks updates via `Steam.cfg` / `steam.cfg` (`BootStrapperInhibitAll=enable`, `BootStrapperInhibitUpdateOnLaunch=enable`, `BootStrapperForceSelfUpdate=false`) and `chmod a-w package/`.
- **One-Click Unlock**: Restores write permissions on `package/` and removes `Steam.cfg` / `steam.cfg` so Steam can update normally.

## Building from Source

```bash
cargo build --release
./target/release/steamrollbackr
```

## License

MIT

