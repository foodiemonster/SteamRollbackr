use chrono::{DateTime, Local, Utc};
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

    pub fn label(self) -> &'static str {
        match self {
            ManifestChannel::Ubuntu12 => "ubuntu12 (Legacy / Pre-Nov 2026)",
            ManifestChannel::SteamRt64 => "steamrt64 (64-bit / Recommended)",
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

#[derive(Debug, Clone)]
pub struct SteamStatus {
    pub steam_dir: Option<PathBuf>,
    pub is_flatpak: bool,
    pub current_version: Option<ClientVersion>,
    pub lock_state: LockState,
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
    let steam_dir = find_steam_dir();
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
            }
        }
        None => SteamStatus {
            steam_dir: None,
            is_flatpak: false,
            current_version: None,
            lock_state: LockState::Unlocked,
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

