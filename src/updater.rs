use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASES_ATOM_URL: &str = "https://github.com/foodiemonster/SteamRollbackr/releases.atom";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallKind {
    AppImage(PathBuf),
    UserLocal,
    SystemPackage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub latest_tag: String,
    pub latest_version: String,
    pub release_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOutcome {
    Relaunched,
    OpenedReleasePage,
}

pub fn detect_install_kind() -> InstallKind {
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        let p = PathBuf::from(appimage);
        if p.is_file() {
            return InstallKind::AppImage(p);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if exe.starts_with("/usr/") {
            return InstallKind::SystemPackage;
        }
    }
    InstallKind::UserLocal
}

pub fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let trimmed = v.trim().trim_start_matches(['v', 'V']);
    let core = trimmed.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let patch = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    Some((major, minor, patch))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_semver(latest), parse_semver(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

pub fn parse_latest_release_from_atom(xml: &str) -> Option<(String, String)> {
    let entry_start = xml.find("<entry>")?;
    let entry_end = xml[entry_start..].find("</entry>")?;
    let entry = &xml[entry_start..entry_start + entry_end];

    if let Some(pos) = entry.find("/releases/tag/") {
        let after = &entry[pos + "/releases/tag/".len()..];
        let tag_end = after
            .find(['"', '<', '\'', ' ', '\n', '\r'])
            .unwrap_or(after.len());
        let tag = after[..tag_end].trim().to_string();
        if !tag.is_empty() {
            let ver = tag.trim_start_matches(['v', 'V']).to_string();
            return Some((tag, ver));
        }
    }

    // Fallback to <title>
    let t_start = entry.find("<title>")? + "<title>".len();
    let t_end = entry[t_start..].find("</title>")?;
    let title = entry[t_start..t_start + t_end].trim();
    let token = title
        .split_whitespace()
        .find(|w| parse_semver(w).is_some())?;
    let ver = token.trim_start_matches(['v', 'V']).to_string();
    Some((format!("v{ver}"), ver))
}

pub fn check_for_update() -> Result<Option<UpdateInfo>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();
    let resp = agent
        .get(RELEASES_ATOM_URL)
        .set("User-Agent", "SteamRollbackr")
        .call()
        .map_err(|e| format!("Failed to check for updates: {e}"))?;

    let xml = resp
        .into_string()
        .map_err(|e| format!("Failed to read update feed: {e}"))?;

    let Some((latest_tag, latest_version)) = parse_latest_release_from_atom(&xml) else {
        return Ok(None);
    };

    if is_newer(&latest_version, CURRENT_VERSION) {
        let release_url = format!(
            "https://github.com/foodiemonster/SteamRollbackr/releases/tag/{latest_tag}"
        );
        Ok(Some(UpdateInfo {
            latest_tag,
            latest_version,
            release_url,
        }))
    } else {
        Ok(None)
    }
}

fn download_to_file(url: &str, dest: &PathBuf) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(60))
        .build();
    let resp = agent
        .get(url)
        .set("User-Agent", "SteamRollbackr")
        .call()
        .map_err(|e| format!("Download failed ({url}): {e}"))?;

    let mut bytes = Vec::new();
    resp.into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed reading download stream: {e}"))?;

    fs::write(dest, &bytes)
        .map_err(|e| format!("Failed writing {}: {e}", dest.display()))
}

pub fn perform_update(
    info: &UpdateInfo,
    log: &mut dyn FnMut(&str),
) -> Result<UpdateOutcome, String> {
    match detect_install_kind() {
        InstallKind::AppImage(appimage_path) => {
            let url = format!(
                "https://github.com/foodiemonster/SteamRollbackr/releases/download/{}/SteamRollbackr-{}-x86_64.AppImage",
                info.latest_tag, info.latest_version
            );
            log(&format!("Downloading AppImage update from {url}..."));

            let tmp_path = appimage_path.with_extension("AppImage.new");
            download_to_file(&url, &tmp_path)?;

            let mut perms = fs::metadata(&tmp_path)
                .map_err(|e| format!("Failed to stat downloaded AppImage: {e}"))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&tmp_path, perms)
                .map_err(|e| format!("Failed to chmod +x AppImage: {e}"))?;

            fs::rename(&tmp_path, &appimage_path)
                .map_err(|e| format!("Failed to replace AppImage: {e}"))?;
            log("AppImage updated! Relaunching SteamRollbackr...");

            Command::new(&appimage_path)
                .spawn()
                .map_err(|e| format!("Updated, but failed to relaunch AppImage: {e}"))?;
            Ok(UpdateOutcome::Relaunched)
        }
        InstallKind::UserLocal => {
            let url = format!(
                "https://github.com/foodiemonster/SteamRollbackr/releases/download/{}/steamrollbackr-linux-x86_64.tar.gz",
                info.latest_tag
            );
            let tmp_dir = std::env::temp_dir().join(format!(
                "steamrollbackr_update_{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&tmp_dir);
            fs::create_dir_all(&tmp_dir)
                .map_err(|e| format!("Failed to create temp update dir: {e}"))?;

            let tarball = tmp_dir.join("update.tar.gz");
            log(&format!("Downloading update archive from {url}..."));
            download_to_file(&url, &tarball)?;

            log("Extracting update archive...");
            let extract_status = Command::new("tar")
                .args(["-xzf"])
                .arg(&tarball)
                .arg("-C")
                .arg(&tmp_dir)
                .status()
                .map_err(|e| format!("Failed to run tar: {e}"))?;
            if !extract_status.success() {
                return Err(format!("tar extraction failed with status: {extract_status}"));
            }

            let install_script = tmp_dir.join("install.sh");
            log("Installing updated binary, icon, and desktop entry...");
            let install_status = Command::new("bash")
                .arg(&install_script)
                .status()
                .map_err(|e| format!("Failed to run install.sh: {e}"))?;
            if !install_status.success() {
                return Err(format!("install.sh failed with status: {install_status}"));
            }

            let _ = fs::remove_dir_all(&tmp_dir);

            let target_bin = std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".local/bin/steamrollbackr"))
                .filter(|p| p.is_file())
                .or_else(|| std::env::current_exe().ok())
                .ok_or_else(|| "Could not locate updated binary to relaunch".to_string())?;

            log("Update installed! Relaunching SteamRollbackr...");
            Command::new(&target_bin)
                .spawn()
                .map_err(|e| format!("Updated, but failed to relaunch: {e}"))?;
            Ok(UpdateOutcome::Relaunched)
        }
        InstallKind::SystemPackage => {
            log("System package installation detected (.deb / .rpm). Opening GitHub Releases page...");
            Command::new("xdg-open")
                .arg(&info.release_url)
                .spawn()
                .map_err(|e| format!("Failed to open browser: {e}"))?;
            Ok(UpdateOutcome::OpenedReleasePage)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_comparison() {
        assert!(is_newer("0.5.3", "0.5.2"));
        assert!(is_newer("v0.6.0", "0.5.2"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.5.2", "0.5.2"));
        assert!(!is_newer("0.5.1", "0.5.2"));
    }

    #[test]
    fn test_parse_latest_release_from_atom() {
        let xml = r#"
<feed>
  <entry>
    <title>SteamRollbackr v0.5.3</title>
    <link rel="alternate" type="text/html" href="https://github.com/foodiemonster/SteamRollbackr/releases/tag/v0.5.3"/>
  </entry>
</feed>
"#;
        assert_eq!(
            parse_latest_release_from_atom(xml),
            Some(("v0.5.3".to_string(), "0.5.3".to_string()))
        );
    }
}
