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
pub struct SteamStatus {
    pub steam_dir: Option<PathBuf>,
    pub is_flatpak: bool,
    pub current_version: Option<ClientVersion>,
    pub lock_state: LockState,
    pub lumen_versions: LumenVersions,
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
    match steam_dir {
        Some(dir) => {
            let is_flatpak = dir
                .to_string_lossy()
                .contains(".var/app/com.valvesoftware.Steam");
            let current_version = read_installed_version(&dir);
            let lock_state = check_lock_state(&dir);
            SteamStatus {
                steam_dir: Some(dir),
                is_flatpak,
                current_version,
                lock_state,
                lumen_versions,
            }
        }
        None => SteamStatus {
            steam_dir: None,
            is_flatpak: false,
            current_version: None,
            lock_state: LockState::Unlocked,
            lumen_versions,
        },
    }
}

fn find_steam_bin(steam_dir: &Path) -> Option<PathBuf> {
    let ubuntu_bin = steam_dir.join("ubuntu12_32/steam");
    if ubuntu_bin.is_file() {
        return Some(ubuntu_bin);
    }
    let script_bin = steam_dir.join("steam.sh");
    if script_bin.is_file() {
        return Some(script_bin);
    }
    None
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
    let bin = find_steam_bin(steam_dir)
        .ok_or_else(|| "Could not find steam binary (ubuntu12_32/steam or steam.sh)".to_string())?;

    kill_steam(log);
    unlock_updates(steam_dir, log)?;
    cleanup_conflicting_files(steam_dir, ManifestChannel::SteamRt64, log);

    log("Updating Steam to the latest official release...");
    let status = Command::new(&bin)
        .args(["-forcemanifestsuffix", "steamrt64", "-forcesteamupdate", "-exitsteam"])
        .status();

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
    let bin = find_steam_bin(steam_dir)
        .ok_or_else(|| "Could not find steam binary (ubuntu12_32/steam or steam.sh)".to_string())?;

    kill_steam(log);
    log("Starting Steam...");
    Command::new(&bin)
        .spawn()
        .map_err(|e| format!("Failed to start Steam: {e}"))?;
    log("Steam launched.");
    Ok(())
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
        {
            return Some(path);
        }
    }
    None
}

/// Copies 32-bit libcrypto.so.3 from SteamLinuxRuntime in steamapps/common into ubuntu12_32/ and sets +x.
pub fn install_libcrypto3(steam_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<(), String> {
    let common_dir = steam_dir.join("steamapps/common");
    if !common_dir.is_dir() {
        return Err(format!(
            "steamapps/common not found at {}",
            common_dir.display()
        ));
    }

    log(&format!(
        "Searching {} for i386-linux-gnu/libcrypto.so.3...",
        common_dir.display()
    ));
    let src = find_i386_libcrypto(&common_dir).ok_or_else(|| {
        "Could not find i386-linux-gnu/libcrypto.so.3 in steamapps/common. Make sure Steam Linux Runtime is installed.".to_string()
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
    let bin = find_steam_bin(steam_dir)
        .ok_or_else(|| "Could not find steam binary (ubuntu12_32/steam or steam.sh)".to_string())?;

    kill_steam(log);

    log("Ensuring package/ is writable...");
    set_package_writable(steam_dir, true)?;
    cleanup_conflicting_files(steam_dir, channel, log);

    log("Starting local manifest server for -overridepackageurl...");
    let server = ManifestServer::start(channel.filename(), manifest_content.to_string())?;
    let url = format!("http://127.0.0.1:{}", server.port());
    log(&format!("Local manifest server listening on {url}"));

    log(&format!(
        "Running Steam rollback ({}) — this may take a minute while packages download...",
        bin.display()
    ));

    let status = Command::new(&bin)
        .args([
            "-forcemanifestsuffix",
            channel.suffix(),
            "-forcesteamupdate",
            "-forcepackagedownload",
            "-overridepackageurl",
            &url,
            "-exitsteam",
        ])
        .status();

    drop(server);

    match status {
        Ok(s) => log(&format!("Steam updater exited with status: {s}")),
        Err(e) => log(&format!("Steam updater warning: {e} (continuing to lock)")),
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
}

