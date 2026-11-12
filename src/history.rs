use chrono::{DateTime, Local};
use serde::Deserialize;

use crate::steam::{parse_manifest_version, ManifestChannel};

#[derive(Debug, Clone)]
pub struct ReleaseEntry {
    pub sha: String,
    pub short_sha: String,
    pub date_display: String,
    pub message: String,
    pub channel: ManifestChannel,
}

#[derive(Deserialize)]
struct GitHubCommit {
    sha: String,
    commit: CommitInner,
}

#[derive(Deserialize)]
struct CommitInner {
    author: CommitAuthor,
    message: String,
}

#[derive(Deserialize)]
struct CommitAuthor {
    date: String,
}

pub fn fetch_releases(channel: ManifestChannel, page: u32) -> Result<Vec<ReleaseEntry>, String> {
    let url = format!(
        "https://api.github.com/repos/SteamTracking/SteamTracking/commits?path=ClientManifest/{}&per_page=50&page={page}",
        channel.filename()
    );

    let resp = ureq::get(&url)
        .set("User-Agent", "SteamRollbackr/0.1")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("GitHub API request failed: {e}"))?;

    let commits: Vec<GitHubCommit> = resp
        .into_json()
        .map_err(|e| format!("Failed to parse GitHub commits JSON: {e}"))?;

    let mut releases = Vec::new();
    for c in commits {
        let first_line = c.commit.message.lines().next().unwrap_or("").trim().to_string();
        if first_line.starts_with("Delete ") {
            continue;
        }

        let date_display = DateTime::parse_from_rfc3339(&c.commit.author.date)
            .map(|dt| {
                dt.with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|_| c.commit.author.date.clone());

        let short_sha = c.sha.chars().take(8).collect();
        releases.push(ReleaseEntry {
            sha: c.sha,
            short_sha,
            date_display,
            message: first_line,
            channel,
        });
    }

    Ok(releases)
}

pub fn fetch_manifest_content(
    sha: &str,
    channel: ManifestChannel,
) -> Result<(String, Option<u64>), String> {
    let url = format!(
        "https://raw.githubusercontent.com/SteamTracking/SteamTracking/{sha}/ClientManifest/{}",
        channel.filename()
    );

    let resp = ureq::get(&url)
        .set("User-Agent", "SteamRollbackr/0.1")
        .call()
        .map_err(|e| format!("Failed to download manifest ({sha}): {e}"))?;

    let body = resp
        .into_string()
        .map_err(|e| format!("Failed to read manifest body: {e}"))?;

    let ver = parse_manifest_version(&body);
    Ok((body, ver))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_commit_json() {
        let json = r#"
        [
          {
            "sha": "abcdef1234567890",
            "commit": {
              "author": { "date": "2026-11-10T12:00:00Z" },
              "message": "Update steam_client_steamrt64"
            }
          }
        ]
        "#;
        let parsed: Vec<GitHubCommit> = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].sha, "abcdef1234567890");
    }
}
