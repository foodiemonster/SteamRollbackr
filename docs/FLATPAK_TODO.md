# Flatpak Distribution TODO

Notes and implementation plan for packaging **SteamRollbackr** as a `.flatpak` bundle (and optional custom `.flatpakrepo` for Bazaar / Discover / GNOME Software).

## Why Self-Hosted `.flatpak` / `.flatpakrepo` (vs. Flathub)
Flathub's human review policy typically rejects apps that:
- Use `--talk-name=org.freedesktop.Flatpak` (`flatpak-spawn --host`) to run host binaries.
- Modify another app's files or inhibit its updates.
- Download and execute external shell scripts (`curl ... | bash` in the Tools tab).

Shipping a self-hosted `.flatpak` bundle on GitHub Releases (and optionally a `.flatpakrepo` on GitHub Pages) avoids Flathub restrictions while letting Bazzite/SteamOS/desktop users double-click to install in **Bazaar**, **Discover**, or **GNOME Software**.

---

## Implementation Checklist

### 1. Host Command Wrapper in `src/steam.rs`
- Detect Flatpak sandbox via `Path::new("/.flatpak-info").exists()`.
- When inside Flatpak, wrap host commands (`pkill`, `pgrep`, `steam.sh`, `/usr/bin/steam`, `flatpak run com.valvesoftware.Steam`, `sh -c <tool_cmd>`) with `flatpak-spawn --host`.
- Note: `/etc/os-release` inside a Flatpak reflects the Freedesktop runtime, not the host distro; read `/run/host/etc/os-release` when running inside Flatpak (`--filesystem=host-etc` or `flatpak-spawn --host cat /etc/os-release`).

### 2. Reverse-DNS App ID & AppStream Metadata
- Choose App ID: `io.github.foodiemonster.SteamRollbackr`
- Add `packaging/io.github.foodiemonster.SteamRollbackr.metainfo.xml` (name, summary, description, MIT license, screenshot URL, release tags).
- Provide/rename `.desktop` and 256x256 `.png` icon using the reverse-DNS ID inside the Flatpak manifest.

### 3. Flatpak Manifest (`packaging/io.github.foodiemonster.SteamRollbackr.yml`)
- Runtime: `org.freedesktop.Platform` / `org.freedesktop.Sdk` (e.g. `24.08`) + `org.freedesktop.Sdk.Extension.rust-stable`.
- Finish args:
  - `--share=network`
  - `--socket=wayland`
  - `--socket=fallback-x11`
  - `--device=dri`
  - `--filesystem=home`
  - `--filesystem=/run/media`
  - `--talk-name=org.freedesktop.Flatpak`
- Offline Rust build sources:
  - Generate `packaging/cargo-sources.json` from `Cargo.lock` using `flatpak-cargo-generator.py`.

### 4. CI & Hosting (`.github/workflows/release.yml`)
- Add a `build-flatpak` job using `flatpak-builder` + `flatpak build-bundle` to produce `SteamRollbackr-<version>-x86_64.flatpak` and attach it to GitHub Releases.
- *(Optional)* Publish an OSTree Flatpak repository to GitHub Pages with a `.flatpakrepo` file so users can add it to **Bazaar** / **Discover** for automatic updates.

