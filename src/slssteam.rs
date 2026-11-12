use chrono::{DateTime, Local, Utc};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct SlsMoonStatus {
    pub monitor_display: String,
    pub moon_display: String,
    pub latest_steam_display: Option<String>,
    pub updated_for_latest_steam: Option<bool>,
    pub local_pattern_supported: Option<bool>,
    pub local_steamclient_sha: Option<String>,
}

#[derive(Deserialize)]
struct GitHubCommit {
    commit: CommitInner,
}

#[derive(Deserialize)]
struct CommitInner {
    author: CommitAuthor,
}

#[derive(Deserialize)]
struct CommitAuthor {
    date: String,
}

fn format_dt(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

fn fetch_latest_commit_date(repo: &str, path: Option<&str>) -> Result<DateTime<Utc>, String> {
    let mut url = format!("https://api.github.com/repos/{repo}/commits?per_page=1");
    if let Some(p) = path {
        url.push_str(&format!("&path={p}"));
    }

    let commits: Vec<GitHubCommit> = ureq::get(&url)
        .set("User-Agent", "SteamRollbackr/0.1")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Failed to query {repo}: {e}"))?
        .into_json()
        .map_err(|e| format!("Failed to parse commits for {repo}: {e}"))?;

    let first = commits
        .into_iter()
        .next()
        .ok_or_else(|| format!("No commits found for {repo}"))?;

    DateTime::parse_from_rfc3339(&first.commit.author.date)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| format!("Invalid date from {repo}: {e}"))
}

fn compute_sha256(path: &Path) -> Option<String> {
    let out = Command::new("sha256sum").arg(path).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout.split_whitespace().next().map(|s| s.to_string())
}

fn check_pattern_exists(sha256: &str) -> bool {
    let url = format!(
        "https://raw.githubusercontent.com/swwayps/steam-monitor/main/linux32/steamclient/{sha256}.toml"
    );
    ureq::head(&url)
        .set("User-Agent", "SteamRollbackr/0.1")
        .call()
        .map(|r| r.status() == 200)
        .unwrap_or(false)
}

pub fn check_slsteam_moon(steam_dir: Option<&Path>) -> Result<SlsMoonStatus, String> {
    let monitor_date = fetch_latest_commit_date("swwayps/steam-monitor", None)?;
    let moon_date = fetch_latest_commit_date("swwayps/slsteam-moon", None)?;
    let latest_steam_date =
        fetch_latest_commit_date("SteamTracking/SteamTracking", Some("ClientManifest/steam_client_steamrt64")).ok();

    let newest_pattern_update = monitor_date.max(moon_date);
    let updated_for_latest_steam = latest_steam_date.map(|st| newest_pattern_update >= st);

    let (local_steamclient_sha, local_pattern_supported) = match steam_dir {
        Some(dir) => {
            let so = dir.join("ubuntu12_32/steamclient.so");
            if so.is_file() {
                if let Some(sha) = compute_sha256(&so) {
                    let supported = check_pattern_exists(&sha);
                    (Some(sha), Some(supported))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        }
        None => (None, None),
    };

    Ok(SlsMoonStatus {
        monitor_display: format_dt(monitor_date),
        moon_display: format_dt(moon_date),
        latest_steam_display: latest_steam_date.map(format_dt),
        updated_for_latest_steam,
        local_pattern_supported,
        local_steamclient_sha,
    })
}
