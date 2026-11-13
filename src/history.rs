use chrono::{DateTime, Local};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::steam::ManifestChannel;

#[derive(Debug, Clone)]
pub struct ReleaseEntry {
    pub sha: String,
    pub short_sha: String,
    pub date_display: String,
    pub timestamp_utc: i64,
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

fn cache_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("steamrollbackr"));
        }
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache/steamrollbackr"))
}

pub fn read_cache(key: &str, max_age: Duration) -> Option<String> {
    let path = cache_dir()?.join(key);
    let meta = fs::metadata(&path).ok()?;
    let modified = meta.modified().ok()?;
    let age = SystemTime::now().duration_since(modified).ok()?;
    if age <= max_age {
        fs::read_to_string(path).ok()
    } else {
        None
    }
}

pub fn read_stale_cache(key: &str) -> Option<String> {
    let path = cache_dir()?.join(key);
    fs::read_to_string(path).ok()
}

pub fn write_cache(key: &str, content: &str) {
    if let Some(dir) = cache_dir() {
        if fs::create_dir_all(&dir).is_ok() {
            let _ = fs::write(dir.join(key), content);
        }
    }
}

fn extract_tag(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&close)? + start;
    Some(block[start..end].trim().to_string())
}

fn decode_xml_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

/// Parses GitHub `.atom` commit feed entries into `(sha, rfc3339_date, title)`.
pub fn parse_atom_entries(xml: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<entry>") {
        let after_start = &rest[start + 7..];
        let Some(end) = after_start.find("</entry>") else {
            break;
        };
        let entry = &after_start[..end];
        rest = &after_start[end + 8..];

        let sha = extract_tag(entry, "id")
            .and_then(|id| id.rsplit('/').next().map(|s| s.to_string()));
        let updated = extract_tag(entry, "updated");
        let title = extract_tag(entry, "title").map(|t| {
            let first = t.lines().next().unwrap_or("").trim();
            decode_xml_entities(first)
        });

        if let (Some(sha), Some(updated), Some(title)) = (sha, updated, title) {
            if !sha.is_empty() {
                out.push((sha, updated, title));
            }
        }
    }
    out
}

fn parse_rfc3339_ts(date_str: &str) -> i64 {
    DateTime::parse_from_rfc3339(date_str)
        .map(|dt| dt.timestamp())
        .unwrap_or(0)
}

fn format_rfc3339(date_str: &str) -> String {
    DateTime::parse_from_rfc3339(date_str)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| date_str.to_string())
}

fn fetch_releases_atom(channel: ManifestChannel) -> Result<Vec<ReleaseEntry>, String> {
    let cache_key = format!("releases_{}_atom.xml", channel.suffix());
    let ttl = Duration::from_secs(600);

    let xml = if let Some(cached) = read_cache(&cache_key, ttl) {
        cached
    } else {
        let url = format!(
            "https://github.com/SteamTracking/SteamTracking/commits/HEAD/ClientManifest/{}.atom",
            channel.filename()
        );
        match ureq::get(&url)
            .set("User-Agent", "SteamRollbackr/0.3")
            .call()
            .and_then(|r| r.into_string().map_err(|e| ureq::Error::from(e)))
        {
            Ok(body) => {
                write_cache(&cache_key, &body);
                body
            }
            Err(e) => {
                if let Some(stale) = read_stale_cache(&cache_key) {
                    stale
                } else {
                    return Err(format!("GitHub Atom feed request failed: {e}"));
                }
            }
        }
    };

    let mut releases = Vec::new();
    for (sha, updated, title) in parse_atom_entries(&xml) {
        if title.starts_with("Delete ") {
            continue;
        }
        let short_sha = sha.chars().take(8).collect();
        releases.push(ReleaseEntry {
            sha,
            short_sha,
            date_display: format_rfc3339(&updated),
            timestamp_utc: parse_rfc3339_ts(&updated),
            message: title,
            channel,
        });
    }
    Ok(releases)
}

pub fn fetch_releases(channel: ManifestChannel, page: u32) -> Result<Vec<ReleaseEntry>, String> {
    if page == 1 {
        return fetch_releases_atom(channel);
    }

    let cache_key = format!("releases_{}_p{page}.json", channel.suffix());
    let ttl = Duration::from_secs(1800);

    let body = if let Some(cached) = read_cache(&cache_key, ttl) {
        cached
    } else {
        let url = format!(
            "https://api.github.com/repos/SteamTracking/SteamTracking/commits?path=ClientManifest/{}&per_page=50&page={page}",
            channel.filename()
        );

        match ureq::get(&url)
            .set("User-Agent", "SteamRollbackr/0.3")
            .set("Accept", "application/vnd.github+json")
            .call()
        {
            Ok(resp) => {
                let s = resp
                    .into_string()
                    .map_err(|e| format!("Failed to read GitHub commits response: {e}"))?;
                write_cache(&cache_key, &s);
                s
            }
            Err(ureq::Error::Status(403, _)) => {
                if let Some(stale) = read_stale_cache(&cache_key) {
                    stale
                } else {
                    return Err("GitHub API rate limit reached for older pages (try again later).".to_string());
                }
            }
            Err(e) => {
                if let Some(stale) = read_stale_cache(&cache_key) {
                    stale
                } else {
                    return Err(format!("GitHub API request failed: {e}"));
                }
            }
        }
    };

    let commits: Vec<GitHubCommit> = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse GitHub commits JSON: {e}"))?;

    let mut releases = Vec::new();
    for c in commits {
        let first_line = c.commit.message.lines().next().unwrap_or("").trim().to_string();
        if first_line.starts_with("Delete ") {
            continue;
        }
        let short_sha = c.sha.chars().take(8).collect();
        releases.push(ReleaseEntry {
            sha: c.sha,
            short_sha,
            date_display: format_rfc3339(&c.commit.author.date),
            timestamp_utc: parse_rfc3339_ts(&c.commit.author.date),
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
        .set("User-Agent", "SteamRollbackr/0.3")
        .call()
        .map_err(|e| format!("Failed to download manifest ({sha}): {e}"))?;

    let body = resp
        .into_string()
        .map_err(|e| format!("Failed to read manifest body: {e}"))?;

    let ver = crate::steam::parse_manifest_version(&body);
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

    #[test]
    fn test_parse_atom_entries() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <updated>2026-11-12T15:42:17Z</updated>
  <entry>
    <id>tag:github.com,2008:Grit::Commit/64129d04a32dc2652e8534898fb7d52da2b8db3b</id>
    <title>
        Update steam_client_steamrt64
    </title>
    <updated>2026-11-12T15:42:17Z</updated>
  </entry>
</feed>"#;
        let entries = parse_atom_entries(xml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "64129d04a32dc2652e8534898fb7d52da2b8db3b");
        assert_eq!(entries[0].1, "2026-11-12T15:42:17Z");
        assert_eq!(entries[0].2, "Update steam_client_steamrt64");
    }
}
