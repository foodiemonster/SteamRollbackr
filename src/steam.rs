use chrono::{DateTime, Local, Utc};
use serde::Deserialize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::server::ManifestServer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestChannel {
    Ubuntu12,
    SteamRt64,
}

impl ManifestChannel {
    pub fn suffix(self) -> &'static str {
        match self {
            ManifestChannel::Ubuntu12 => "ubuntu12",
            ManifestChannel::SteamRt64 => "steamrt64",
        }
    }

    pub fn filename(self) -> &'static str {
        match self {
            ManifestChannel::Ubuntu12 => "steam_client_ubuntu12",
            ManifestChannel::SteamRt64 => "steam_client_steamrt64",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientVersion {
    pub version_id: u64,
    pub formatted_date: String,
    pub channel: ManifestChannel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockState {
    Locked,
    PartiallyLocked(String),
    Unlocked,
}

impl LockState {
    pub fn is_locked(&self) -> bool {
        !matches!(self, LockState::Unlocked)
    }
}

#[derive(Debug, Clone, Default)]
pub struct LumenVersions {
    pub slsteam_moon: Option<String>,
    pub lumen: Option<String>,
    pub luatools: Option<String>,
    pub slssteam_installed: bool,
    pub lumen_installed: bool,
    pub luatools_installed: bool,
}

#[derive(Deserialize)]
struct LumenVersionsJson {
    slsteam_moon: Option<LumenComponent>,
    lumen: Option<LumenComponent>,
    plugin: Option<LumenComponent>,
}

#[derive(Deserialize)]
struct LumenComponent {
    tag: Option<String>,
}

fn read_lumen_versions() -> LumenVersions {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return LumenVersions::default();
    };
    let slssteam_installed = home.join(".local/share/SLSsteam/SLSsteam.so").is_file();
    let lumen_installed = home.join(".local/share/Lumen/lumen").is_file();
    let luatools_installed =
        home.join(".local/share/LuaTools").is_dir() || home.join(".local/share/Lumen/luatools").is_dir();

    let mut out = LumenVersions {
        slssteam_installed,
        lumen_installed,
        luatools_installed,
        ..Default::default()
    };

    let versions_path = home.join(".local/share/Lumen/versions.json");
    if let Ok(content) = fs::read_to_string(versions_path) {
        if let Ok(parsed) = serde_json::from_str::<LumenVersionsJson>(&content) {
            out.slsteam_moon = parsed.slsteam_moon.and_then(|c| c.tag);
            out.lumen = parsed.lumen.and_then(|c| c.tag);
            out.luatools = parsed.plugin.and_then(|c| c.tag);
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct DistroInfo {
    pub id: String,
    pub name: String,
    pub is_arch_based: bool,
    pub in_steamos_gaming_mode: bool,
}

#[derive(Debug, Clone)]
pub struct SteamStatus {
    pub steam_dir: Option<PathBuf>,
    pub is_flatpak: bool,
    pub distro: DistroInfo,
    pub current_version: Option<ClientVersion>,
    pub lock_state: LockState,
    pub lumen_versions: LumenVersions,
}

pub fn parse_os_release(content: &str) -> (String, String, bool) {
    let mut id = String::new();
    let mut id_like = String::new();
    let mut pretty = String::new();
    let mut name = String::new();

    for line in content.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("ID=") {
            id = val.trim_matches('"').trim_matches('\'').to_lowercase();
        } else if let Some(val) = line.strip_prefix("ID_LIKE=") {
            id_like = val.trim_matches('"').trim_matches('\'').to_lowercase();
        } else if let Some(val) = line.strip_prefix("PRETTY_NAME=") {
            pretty = val.trim_matches('"').trim_matches('\'').to_string();
        } else if let Some(val) = line.strip_prefix("NAME=") {
            name = val.trim_matches('"').trim_matches('\'').to_string();
        }
    }

    let is_arch_based = id != "steamos"
        && (id == "arch" || id_like.split_whitespace().any(|tok| tok == "arch"));

    let display = if !pretty.is_empty() {
        pretty
    } else if !name.is_empty() {
        name
    } else if !id.is_empty() {
        id.clone()
    } else {
        "Linux".to_string()
    };
    (id, display, is_arch_based)
}

pub fn detect_distro() -> DistroInfo {
    let (id, name, is_arch_based) = fs::read_to_string("/etc/os-release")
        .map(|s| parse_os_release(&s))
        .unwrap_or_else(|_| (String::new(), "Linux".to_string(), false));

    let in_steamos_gaming_mode = id == "steamos"
        && Command::new("pgrep")
            .args(["-x", "gamescope-session"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

    DistroInfo {
        id,
        name,
        is_arch_based,
        in_steamos_gaming_mode,
    }
}

/// Searches standard and Flatpak Steam locations in a distro-agnostic order.
pub fn find_steam_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let candidates = [
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".steam/root"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ];

    for candidate in &candidates {
        if candidate.join("package").is_dir() {
            if let Ok(resolved) = fs::canonicalize(candidate) {
                if resolved.join("package").is_dir() {
                    return Some(resolved);
                }
            }
            return Some(candidate.clone());
        }
    }
    None
}

/// Parses the `"version" "<digits>"` field from a Valve VDF client manifest.
pub fn parse_manifest_version(content: &str) -> Option<u64> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("\"version\"") {
            let digits = rest.trim().trim_matches('"');
            if let Ok(val) = digits.parse::<u64>() {
                return Some(val);
            }
        }
    }
    None
}

pub fn format_unix_timestamp(ts: u64) -> String {
    DateTime::<Utc>::from_timestamp(ts as i64, 0)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "Unknown date".to_string())
}

fn read_installed_version(steam_dir: &Path) -> Option<ClientVersion> {
    let pkg = steam_dir.join("package");
    let checks = [
        (ManifestChannel::SteamRt64, "steam_client_steamrt64.manifest"),
        (ManifestChannel::Ubuntu12, "steam_client_ubuntu12.manifest"),
    ];

    let mut found: Vec<ClientVersion> = Vec::new();
    for (channel, file) in checks {
        let path = pkg.join(file);
        if let Ok(content) = fs::read_to_string(&path) {
            if let Some(ver) = parse_manifest_version(&content) {
                found.push(ClientVersion {
                    version_id: ver,
                    formatted_date: format_unix_timestamp(ver),
                    channel,
                });
            }
        }
    }

    // Prefer the newer manifest if both exist on disk.
    found.into_iter().max_by_key(|v| v.version_id)
}

pub fn check_lock_state(steam_dir: &Path) -> LockState {
    let has_cfg = ["Steam.cfg", "steam.cfg"].iter().any(|name| {
        let p = steam_dir.join(name);
        fs::read_to_string(p)
            .map(|c| c.contains("BootStrapperInhibitAll=enable"))
            .unwrap_or(false)
    });

    let pkg_readonly = fs::metadata(steam_dir.join("package"))
        .map(|m| (m.permissions().mode() & 0o222) == 0)
        .unwrap_or(false);

    match (has_cfg, pkg_readonly) {
        (true, true) => LockState::Locked,
        (true, false) => LockState::PartiallyLocked("cfg present, package/ writable".into()),
        (false, true) => LockState::PartiallyLocked("package/ read-only, cfg missing".into()),
        (false, false) => LockState::Unlocked,
    }
}

pub fn inspect_steam() -> SteamStatus {
    inspect_steam_with_override(None)
}

pub fn inspect_steam_with_override(override_dir: Option<&Path>) -> SteamStatus {
    let steam_dir = match override_dir {
        Some(p) if p.is_dir() => Some(p.to_path_buf()),
        Some(_) => None,
        None => find_steam_dir(),
    };
    let lumen_versions = read_lumen_versions();
    let distro = detect_distro();
    match steam_dir {
        Some(dir) => {
            let is_flatpak = is_flatpak_steam_dir(&dir);
            let current_version = read_installed_version(&dir);
            let lock_state = check_lock_state(&dir);
            SteamStatus {
                steam_dir: Some(dir),
                is_flatpak,
                distro,
                current_version,
                lock_state,
                lumen_versions,
            }
        }
        None => SteamStatus {
            steam_dir: None,
            is_flatpak: false,
            distro,
            current_version: None,
            lock_state: LockState::Unlocked,
            lumen_versions,
        },
    }
}

fn is_flatpak_steam_dir(steam_dir: &Path) -> bool {
    steam_dir
        .to_string_lossy()
        .contains(".var/app/com.valvesoftware.Steam")
}

fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111 != 0))
        .unwrap_or(false)
}

fn find_in_path(bin_name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(bin_name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Returns true if Millennium Steam patcher/loader is detected on the system.
fn has_millennium(steam_dir: &Path) -> bool {
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        if home.join(".millennium").exists() {
            return true;
        }
    }
    if Path::new("/usr/lib/millennium").exists()
        || Path::new("/usr/share/millennium").exists()
        || find_in_path("millennium").is_some()
    {
        return true;
    }
    let steam_sh = steam_dir.join("steam.sh");
    if let Ok(content) = fs::read_to_string(&steam_sh) {
        if content.to_lowercase().contains("millennium") {
            return true;
        }
    }
    false
}

/// Returns ordered (program, prefix_args, description) launcher candidates for rollback.
/// When `prefer_direct_binary` is true (e.g. Arch-based distros or Millennium setups),
/// `ubuntu12_32/steam` and `linux32/steam` are tried before `steam.sh` / system `steam`
/// so wrapper/LD_PRELOAD hooks do not interfere with rollback flags.
fn build_steam_launcher_candidates(
    steam_dir: &Path,
    prefer_direct_binary: bool,
) -> Vec<(PathBuf, Vec<String>, String)> {
    let mut out = Vec::new();
    if is_flatpak_steam_dir(steam_dir) {
        if let Some(flatpak) = find_in_path("flatpak") {
            out.push((
                flatpak,
                vec!["run".to_string(), "com.valvesoftware.Steam".to_string()],
                "flatpak run com.valvesoftware.Steam".to_string(),
            ));
            return out;
        }
    }

    let ordered: [PathBuf; 4] = if prefer_direct_binary {
        [
            steam_dir.join("ubuntu12_32/steam"),
            steam_dir.join("linux32/steam"),
            steam_dir.join("steam.sh"),
            find_in_path("steam").unwrap_or_default(),
        ]
    } else {
        [
            steam_dir.join("steam.sh"),
            find_in_path("steam").unwrap_or_default(),
            steam_dir.join("ubuntu12_32/steam"),
            steam_dir.join("linux32/steam"),
        ]
    };

    for bin in ordered {
        if !bin.as_os_str().is_empty() && is_executable_file(&bin) {
            let desc = bin.display().to_string();
            if !out.iter().any(|(p, _, _): &(PathBuf, Vec<String>, String)| p == &bin) {
                out.push((bin, Vec::new(), desc));
            }
        }
    }
    out
}

/// Builds a distro- and sandbox-aware Steam launch command.
fn build_steam_command(steam_dir: &Path, extra_args: &[&str]) -> Result<(Command, String), String> {
    if is_flatpak_steam_dir(steam_dir) {
        if let Some(flatpak) = find_in_path("flatpak") {
            let mut cmd = Command::new(&flatpak);
            cmd.args(["run", "com.valvesoftware.Steam"]);
            cmd.args(extra_args);
            return Ok((cmd, "flatpak run com.valvesoftware.Steam".to_string()));
        }
    }

    // Prefer steam.sh or system steam wrapper so Steam Runtime LD_LIBRARY_PATH is configured.
    let candidates = [
        steam_dir.join("steam.sh"),
        find_in_path("steam").unwrap_or_default(),
        steam_dir.join("ubuntu12_32/steam"),
        steam_dir.join("linux32/steam"),
    ];
    for bin in candidates {
        if !bin.as_os_str().is_empty() && is_executable_file(&bin) {
            let mut cmd = Command::new(&bin);
            cmd.args(extra_args);
            return Ok((cmd, bin.display().to_string()));
        }
    }

    Err("Could not find executable steam launcher (steam.sh, steam, ubuntu12_32/steam, or linux32/steam)".to_string())
}

fn set_package_writable(steam_dir: &Path, writable: bool) -> Result<(), String> {
    let pkg = steam_dir.join("package");
    let meta = fs::metadata(&pkg).map_err(|e| format!("Failed to stat package/: {e}"))?;
    let mut perms = meta.permissions();
    let mode = perms.mode();
    let new_mode = if writable {
        mode | 0o700
    } else {
        mode & !0o222
    };
    perms.set_mode(new_mode);
    fs::set_permissions(&pkg, perms).map_err(|e| format!("Failed to chmod package/: {e}"))
}

fn write_lock_cfg(steam_dir: &Path) -> Result<(), String> {
    let cfg = "BootStrapperInhibitAll=enable\nBootStrapperInhibitUpdateOnLaunch=enable\nBootStrapperForceSelfUpdate=false\n";
    for name in ["Steam.cfg", "steam.cfg"] {
        let p = steam_dir.join(name);
        // Remove symlink first if one exists so we write clean files.
        if let Ok(meta) = fs::symlink_metadata(&p) {
            if meta.file_type().is_symlink() {
                let _ = fs::remove_file(&p);
            }
        }
        fs::write(&p, cfg).map_err(|e| format!("Failed to write {name}: {e}"))?;
    }
    Ok(())
}

fn kill_steam(log: &mut dyn FnMut(&str)) {
    log("Stopping any running Steam processes...");
    for proc_name in ["steam", "steamwebhelper"] {
        let _ = Command::new("pkill")
            .args(["-9", "-x", proc_name])
            .status();
    }
    thread::sleep(Duration::from_secs(2));
}

fn cleanup_conflicting_files(steam_dir: &Path, target_channel: ManifestChannel, log: &mut dyn FnMut(&str)) {
    let pkg = steam_dir.join("package");
    let prefix_to_remove = match target_channel {
        ManifestChannel::Ubuntu12 => "steam_client_steamrt64",
        ManifestChannel::SteamRt64 => "steam_client_ubuntu12",
    };

    if let Ok(entries) = fs::read_dir(&pkg) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with(prefix_to_remove) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let slsteam_dir = home.join(".local/state/slsteam-moon");
        if let Ok(entries) = fs::read_dir(&slsteam_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("safe_mode") {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
    log(&format!("Cleaned conflicting {prefix_to_remove}* and safe_mode files."));
}

fn reset_slsteam_boot_count(log: &mut dyn FnMut(&str)) {
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let slsteam_dir = home.join(".local/state/slsteam-moon");
        if slsteam_dir.is_dir() {
            let boot_fail = slsteam_dir.join("boot_fail_count");
            if fs::write(&boot_fail, "0\n").is_ok() {
                log("Reset slsteam-moon boot_fail_count to 0.");
            }
        }
    }
}

pub fn lock_only(steam_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<(), String> {
    log("Writing Steam.cfg / steam.cfg to inhibit updates...");
    write_lock_cfg(steam_dir)?;
    log("Setting package/ directory to read-only (chmod a-w)...");
    set_package_writable(steam_dir, false)?;
    reset_slsteam_boot_count(log);
    log("SUCCESS! Steam updates are now locked.");
    Ok(())
}

pub fn unlock_updates(steam_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<(), String> {
    log("Restoring write permissions on package/...");
    set_package_writable(steam_dir, true)?;

    for name in ["Steam.cfg", "steam.cfg"] {
        let p = steam_dir.join(name);
        if p.exists() || fs::symlink_metadata(&p).is_ok() {
            fs::remove_file(&p).map_err(|e| format!("Failed to remove {name}: {e}"))?;
            log(&format!("Removed {name}"));
        }
    }
    log("SUCCESS! Steam is unlocked and can now receive updates.");
    Ok(())
}

/// Unlocks Steam updates and runs the Steam updater to restore the latest official client.
pub fn unlock_and_update_steam(steam_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<(), String> {
    kill_steam(log);
    unlock_updates(steam_dir, log)?;
    cleanup_conflicting_files(steam_dir, ManifestChannel::SteamRt64, log);

    let (mut cmd, launcher_desc) = build_steam_command(
        steam_dir,
        &["-forcemanifestsuffix", "steamrt64", "-forcesteamupdate", "-exitsteam"],
    )?;
    log(&format!("Updating Steam to the latest official release via {launcher_desc}..."));
    let status = cmd.status();

    match status {
        Ok(s) => log(&format!("Steam updater finished with status: {s}")),
        Err(e) => return Err(format!("Failed to run Steam updater: {e}")),
    }

    reset_slsteam_boot_count(log);
    log("SUCCESS! Steam has been unlocked and updated to the latest release.");
    Ok(())
}

/// Restarts the Steam client cleanly.
pub fn restart_steam(steam_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<(), String> {
    kill_steam(log);
    let (mut cmd, launcher_desc) = build_steam_command(steam_dir, &[])?;
    log(&format!("Starting Steam via {launcher_desc}..."));
    cmd.spawn()
        .map_err(|e| format!("Failed to start Steam: {e}"))?;
    log("Steam launched.");
    Ok(())
}

/// Checks whether a file begins with a 32-bit ELF header (`EI_CLASS == 1`).
fn is_elf32(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = fs::File::open(path) else {
        return false;
    };
    let mut hdr = [0u8; 5];
    f.read_exact(&mut hdr).is_ok() && hdr == [0x7F, b'E', b'L', b'F', 1]
}

fn find_i386_libcrypto(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_i386_libcrypto(&path) {
                return Some(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some("libcrypto.so.3")
            && path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                == Some("i386-linux-gnu")
            && is_elf32(&path)
        {
            return Some(path);
        }
    }
    None
}

/// Parses library folder paths from Steam's `libraryfolders.vdf`.
fn parse_library_folders(content: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for line in content.lines() {
        let mut tokens = line.split('"');
        let _before = tokens.next();
        let Some(key) = tokens.next() else { continue };
        let _between = tokens.next();
        let Some(val) = tokens.next() else { continue };
        if key == "path" && !val.trim().is_empty() {
            let p = PathBuf::from(val.trim());
            if !dirs.contains(&p) {
                dirs.push(p);
            }
        }
    }
    dirs
}

/// Collects `<library>/steamapps/common` directories from the primary Steam dir and `libraryfolders.vdf`.
fn collect_steam_common_dirs(steam_dir: &Path) -> Vec<PathBuf> {
    let mut common_dirs = vec![steam_dir.join("steamapps/common")];
    for vdf_rel in ["steamapps/libraryfolders.vdf", "config/libraryfolders.vdf"] {
        let vdf_path = steam_dir.join(vdf_rel);
        if let Ok(content) = fs::read_to_string(&vdf_path) {
            for lib in parse_library_folders(&content) {
                let common = lib.join("steamapps/common");
                if !common_dirs.contains(&common) {
                    common_dirs.push(common);
                }
            }
        }
    }
    common_dirs
}

/// Copies 32-bit libcrypto.so.3 from SteamLinuxRuntime or host 32-bit OpenSSL into ubuntu12_32/ and sets +x.
pub fn install_libcrypto3(steam_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<(), String> {
    let common_dirs = collect_steam_common_dirs(steam_dir);
    let mut found = None;
    for common_dir in &common_dirs {
        if !common_dir.is_dir() {
            continue;
        }
        log(&format!(
            "Searching {} for i386-linux-gnu/libcrypto.so.3...",
            common_dir.display()
        ));
        if let Some(src) = find_i386_libcrypto(common_dir) {
            found = Some(src);
            break;
        }
    }

    if found.is_none() {
        let host_candidates = [
            "/usr/lib32/libcrypto.so.3",
            "/usr/lib/i386-linux-gnu/libcrypto.so.3",
            "/usr/lib/libcrypto.so.3",
        ];
        for cand in host_candidates {
            let p = Path::new(cand);
            if p.is_file() && is_elf32(p) {
                log(&format!("Found 32-bit system libcrypto.so.3 at {cand}"));
                found = Some(p.to_path_buf());
                break;
            }
        }
    }

    let src = found.ok_or_else(|| {
        "Could not find 32-bit libcrypto.so.3 in any Steam library folder or host 32-bit library directory. Install \"Steam Linux Runtime\" in Steam or 32-bit OpenSSL (e.g. lib32-openssl / libssl3:i386 / openssl-libs.i686).".to_string()
    })?;
    log(&format!("Found: {}", src.display()));

    let dest_dir = steam_dir.join("ubuntu12_32");
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Failed to create {}: {e}", dest_dir.display()))?;
    let dest = dest_dir.join("libcrypto.so.3");

    fs::copy(&src, &dest)
        .map_err(|e| format!("Failed to copy to {}: {e}", dest.display()))?;
    log(&format!("Copied to {}", dest.display()));

    let mut perms = fs::metadata(&dest)
        .map_err(|e| format!("Failed to read permissions on {}: {e}", dest.display()))?
        .permissions();
    perms.set_mode(perms.mode() | 0o111);
    fs::set_permissions(&dest, perms)
        .map_err(|e| format!("Failed to chmod +x {}: {e}", dest.display()))?;

    log("SUCCESS! 32-bit libcrypto.so.3 installed and marked executable.");
    Ok(())
}

/// Removes the copied 32-bit libcrypto.so.3 from ubuntu12_32/.
pub fn uninstall_libcrypto3(steam_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<(), String> {
    let dest = steam_dir.join("ubuntu12_32/libcrypto.so.3");
    if !dest.exists() {
        log("32-bit libcrypto.so.3 is not present in ubuntu12_32/.");
        return Ok(());
    }
    fs::remove_file(&dest)
        .map_err(|e| format!("Failed to remove {}: {e}", dest.display()))?;
    log(&format!("Removed {}", dest.display()));
    Ok(())
}

/// Checks if mrpinkguyjp's luatools-linux is installed under ~/.local/bin/luatools.
pub fn is_luatools_linux_installed() -> bool {
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".local/bin/luatools").is_file())
        .unwrap_or(false)
}

/// Runs a shell command via `sh -c` and streams combined output lines to `log`.
pub fn run_shell_command(cmd: &str, log: &mut dyn FnMut(&str)) -> Result<(), String> {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;

    log(&format!("Running: {cmd}"));
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(format!("{cmd} 2>&1"))
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to launch shell command: {e}"))?;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                log(trimmed);
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed waiting for command: {e}"))?;
    if status.success() {
        log("Command completed successfully.");
        Ok(())
    } else {
        Err(format!("Command exited with status: {status}"))
    }
}

/// Rolls back Steam to a specific manifest (served via local HTTP server) and locks updates.
pub fn rollback_to_manifest(
    steam_dir: &Path,
    channel: ManifestChannel,
    manifest_content: &str,
    log: &mut dyn FnMut(&str),
) -> Result<(), String> {
    let distro = detect_distro();
    if distro.in_steamos_gaming_mode {
        log("WARNING: SteamOS Gaming Mode detected. Please run SteamRollbackr from Desktop Mode so Gamescope does not restart Steam during rollback.");
    }

    let millennium_detected = has_millennium(steam_dir);
    let prefer_direct_binary = distro.is_arch_based || millennium_detected;
    if prefer_direct_binary {
        log(&format!(
            "Using direct Steam binary priority (Arch-based: {}, Millennium detected: {}).",
            distro.is_arch_based, millennium_detected
        ));
    }

    kill_steam(log);

    log("Clearing any existing update locks before rollback...");
    unlock_updates(steam_dir, log)?;
    cleanup_conflicting_files(steam_dir, channel, log);

    let candidates = build_steam_launcher_candidates(steam_dir, prefer_direct_binary);
    if candidates.is_empty() {
        return Err("Could not find executable steam launcher (steam.sh, steam, ubuntu12_32/steam, or linux32/steam)".to_string());
    }

    log("Starting local manifest server for -overridepackageurl...");
    let server = ManifestServer::start(channel.filename(), manifest_content.to_string())?;
    let url = format!("http://127.0.0.1:{}", server.port());
    log(&format!("Local manifest server listening on {url}"));

    let rollback_args = [
        "-forcemanifestsuffix",
        channel.suffix(),
        "-forcesteamupdate",
        "-forcepackagedownload",
        "-overridepackageurl",
        &url,
        "-exitsteam",
    ];

    let mut total_hits = 0usize;
    let total_candidates = candidates.len();
    for (idx, (bin, prefix_args, launcher_desc)) in candidates.into_iter().enumerate() {
        log(&format!(
            "Running Steam rollback ({launcher_desc}) — this may take a minute while packages download..."
        ));

        let mut cmd = Command::new(&bin);
        cmd.args(&prefix_args);
        cmd.args(rollback_args);

        let hits_before = server.manifest_hits();
        match cmd.status() {
            Ok(s) => {
                let hits_now = server.manifest_hits();
                let delta = hits_now.saturating_sub(hits_before);
                total_hits = hits_now;
                log(&format!(
                    "Steam updater exited with status: {s} (manifest requests served: {delta})"
                ));
                if delta > 0 {
                    break;
                }
            }
            Err(e) => {
                log(&format!("Failed to run Steam updater ({launcher_desc}): {e}"));
                if idx + 1 == total_candidates {
                    drop(server);
                    return Err(format!("Failed to run Steam updater ({launcher_desc}): {e}"));
                }
            }
        }

        if idx + 1 < total_candidates {
            log("No manifest request received from that launcher; trying next fallback launcher...");
        }
    }

    drop(server);

    if total_hits == 0 {
        return Err("Steam exited without requesting the rollback manifest from the local server. Rollback did not run (leaving package/ unlocked). Check Logs for details.".to_string());
    }

    lock_only(steam_dir, log)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manifest_version() {
        let sample = r#"
"ubuntu12"
{
	"version"		"1792618285"
	"tenfoot_images_all"
	{
		"file"		"tenfoot_images_all.zip.86419c"
	}
}
"#;
        assert_eq!(parse_manifest_version(sample), Some(1792618285));
    }

    #[test]
    fn test_lock_state_detection() {
        let tmp = std::env::temp_dir().join(format!("steamrollbackr_test_{}", std::process::id()));
        let pkg = tmp.join("package");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&pkg).unwrap();

        assert_eq!(check_lock_state(&tmp), LockState::Unlocked);

        write_lock_cfg(&tmp).unwrap();
        assert!(matches!(check_lock_state(&tmp), LockState::PartiallyLocked(_)));

        set_package_writable(&tmp, false).unwrap();
        assert_eq!(check_lock_state(&tmp), LockState::Locked);

        set_package_writable(&tmp, true).unwrap();
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_parse_library_folders() {
        let vdf = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"/home/user/.local/share/Steam"
		"label"		""
	}
	"1"
	{
		"path"		"/run/media/mmcblk0p1/SteamLibrary"
		"label"		"SDCard"
	}
}
"#;
        let libs = parse_library_folders(vdf);
        assert_eq!(
            libs,
            vec![
                PathBuf::from("/home/user/.local/share/Steam"),
                PathBuf::from("/run/media/mmcblk0p1/SteamLibrary"),
            ]
        );
    }

    #[test]
    fn test_parse_os_release() {
        let sample = "NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\nID=arch\n";
        assert_eq!(
            parse_os_release(sample),
            ("arch".to_string(), "Arch Linux".to_string(), true)
        );

        let cachy = "NAME=\"CachyOS\"\nPRETTY_NAME=\"CachyOS\"\nID=cachyos\nID_LIKE=arch\n";
        assert_eq!(
            parse_os_release(cachy),
            ("cachyos".to_string(), "CachyOS".to_string(), true)
        );

        let steamos = "NAME=\"SteamOS\"\nPRETTY_NAME=\"SteamOS 3.6\"\nID=\"steamos\"\nID_LIKE=arch\n";
        assert_eq!(
            parse_os_release(steamos),
            ("steamos".to_string(), "SteamOS 3.6".to_string(), false)
        );
    }

    #[test]
    fn test_launcher_candidate_priority() {
        let tmp = std::env::temp_dir().join(format!("steamrollbackr_launch_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("ubuntu12_32")).unwrap();
        fs::create_dir_all(tmp.join("linux32")).unwrap();
        for rel in ["steam.sh", "ubuntu12_32/steam", "linux32/steam"] {
            let p = tmp.join(rel);
            fs::write(&p, "#!/bin/sh\n").unwrap();
            let mut perms = fs::metadata(&p).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&p, perms).unwrap();
        }

        let arch_order = build_steam_launcher_candidates(&tmp, true);
        assert_eq!(arch_order[0].0, tmp.join("ubuntu12_32/steam"));
        assert_eq!(arch_order[1].0, tmp.join("linux32/steam"));
        assert_eq!(arch_order[2].0, tmp.join("steam.sh"));

        let default_order = build_steam_launcher_candidates(&tmp, false);
        assert_eq!(default_order[0].0, tmp.join("steam.sh"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_is_elf32() {
        let tmp = std::env::temp_dir().join(format!("steamrollbackr_elf_{}", std::process::id()));
        fs::write(&tmp, [0x7F, b'E', b'L', b'F', 1, 1, 1, 0]).unwrap();
        assert!(is_elf32(&tmp));
        fs::write(&tmp, [0x7F, b'E', b'L', b'F', 2, 1, 1, 0]).unwrap();
        assert!(!is_elf32(&tmp));
        let _ = fs::remove_file(&tmp);
    }
}

