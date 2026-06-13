use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::header::ACCEPT;
use serde::{Deserialize, Serialize};

use crate::socket::github_release_cache_path;

const GITHUB_LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/vestin-io/so-context/releases/latest";
const GITHUB_LATEST_RELEASE_URL: &str = "https://github.com/vestin-io/so-context/releases/latest";
const GITHUB_RELEASE_CACHE_TTL: Duration = Duration::from_secs(60 * 60 * 24);
const GITHUB_RELEASE_TIMEOUT: Duration = Duration::from_secs(3);

pub struct LatestReleaseMismatch {
    pub current_version: String,
    pub latest_version: String,
    pub current_binary: PathBuf,
    pub release_url: String,
}

#[derive(Clone)]
pub struct LatestReleaseInfo {
    pub tag_name: String,
    pub html_url: String,
    pub assets: Vec<ReleaseAssetInfo>,
}

#[derive(Clone)]
pub struct ReleaseAssetInfo {
    pub name: String,
    pub download_url: String,
}

#[derive(Clone, Deserialize, Serialize)]
struct LatestReleaseCache {
    checked_at_epoch_secs: u64,
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<ReleaseAssetCache>,
}

#[derive(Clone, Deserialize, Serialize)]
struct ReleaseAssetCache {
    name: String,
    download_url: String,
}

#[derive(Deserialize)]
struct GithubLatestReleaseResponse {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<GithubLatestReleaseAsset>,
}

#[derive(Deserialize)]
struct GithubLatestReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SemanticVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

impl LatestReleaseCache {
    fn new(tag_name: String, html_url: String, assets: Vec<ReleaseAssetCache>) -> Self {
        Self {
            checked_at_epoch_secs: current_epoch_secs(),
            tag_name,
            html_url,
            assets,
        }
    }

    fn is_fresh(&self) -> bool {
        current_epoch_secs().saturating_sub(self.checked_at_epoch_secs)
            <= GITHUB_RELEASE_CACHE_TTL.as_secs()
    }
}

impl From<LatestReleaseCache> for LatestReleaseInfo {
    fn from(value: LatestReleaseCache) -> Self {
        Self {
            tag_name: value.tag_name,
            html_url: value.html_url,
            assets: value
                .assets
                .into_iter()
                .map(|asset| ReleaseAssetInfo {
                    name: asset.name,
                    download_url: asset.download_url,
                })
                .collect(),
        }
    }
}

pub fn binary_version(binary: &Path) -> Result<String> {
    let output = std::process::Command::new(binary)
        .arg("--version")
        .output()
        .with_context(|| format!("inspect so-context version via {}", binary.display()))?;
    if !output.status.success() {
        bail!(
            "failed to inspect so-context version via {} (status: {})",
            binary.display(),
            output.status
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_version_output(&stdout).ok_or_else(|| {
        anyhow!(
            "could not parse so-context version from {} output: {}",
            binary.display(),
            stdout.trim()
        )
    })
}

pub async fn latest_release_mismatch(binary: &str) -> Result<Option<LatestReleaseMismatch>> {
    let current_version = binary_version(Path::new(binary))?;
    if parse_semantic_version(&current_version).is_none() {
        return Ok(None);
    }

    let Some(latest_release) = latest_release_info().await? else {
        return Ok(None);
    };
    if version_order(&current_version, &latest_release.tag_name) != Some(Ordering::Less) {
        return Ok(None);
    }

    Ok(Some(LatestReleaseMismatch {
        current_version,
        latest_version: latest_release.tag_name,
        current_binary: PathBuf::from(binary),
        release_url: latest_release.html_url,
    }))
}

pub async fn latest_release_info() -> Result<Option<LatestReleaseInfo>> {
    Ok(cached_or_fetched_latest_release().await?.map(Into::into))
}

pub async fn latest_release_info_for_update() -> Result<LatestReleaseInfo> {
    let latest = fetch_latest_release().await?;
    write_latest_release_cache(&latest)?;
    Ok(latest.into())
}

pub fn current_release_target() -> Result<String> {
    release_target_from_parts(std::env::consts::OS, std::env::consts::ARCH)
}

pub fn release_target_from_parts(os: &str, arch: &str) -> Result<String> {
    let normalized_arch = match arch {
        "x86_64" | "amd64" => "x86_64",
        "aarch64" | "arm64" => "aarch64",
        other => bail!("unsupported architecture for release update: {other}"),
    };

    let target = match os {
        "macos" => format!("{normalized_arch}-apple-darwin"),
        "linux" => format!("{normalized_arch}-unknown-linux-gnu"),
        other => bail!("unsupported operating system for release update: {other}"),
    };
    Ok(target)
}

pub fn release_asset_name(target: &str) -> String {
    format!("so-context-{target}.tar.gz")
}

pub fn cached_latest_release_mismatch(binary: &str) -> Result<Option<LatestReleaseMismatch>> {
    let current_version = binary_version(Path::new(binary))?;
    if parse_semantic_version(&current_version).is_none() {
        return Ok(None);
    }

    let Some(latest_release) = read_latest_release_cache()?.filter(|cache| cache.is_fresh()) else {
        return Ok(None);
    };
    if version_order(&current_version, &latest_release.tag_name) != Some(Ordering::Less) {
        return Ok(None);
    }

    Ok(Some(LatestReleaseMismatch {
        current_version,
        latest_version: latest_release.tag_name,
        current_binary: PathBuf::from(binary),
        release_url: latest_release.html_url,
    }))
}

fn parse_version_output(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .find(|token| token.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
        .map(|token| token.trim().to_string())
}

fn parse_semantic_version(raw: &str) -> Option<SemanticVersion> {
    let normalized = raw.trim().trim_start_matches('v');
    let core = normalized.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(SemanticVersion {
        major,
        minor,
        patch,
    })
}

fn version_order(current: &str, latest: &str) -> Option<Ordering> {
    Some(parse_semantic_version(current)?.cmp(&parse_semantic_version(latest)?))
}

async fn cached_or_fetched_latest_release() -> Result<Option<LatestReleaseCache>> {
    let cached = read_latest_release_cache().ok().flatten();
    if let Some(cache) = cached.as_ref()
        && cache.is_fresh()
    {
        return Ok(Some(cache.clone()));
    }

    match fetch_latest_release().await {
        Ok(latest) => {
            write_latest_release_cache(&latest)?;
            Ok(Some(latest))
        }
        Err(_) => Ok(cached),
    }
}

async fn fetch_latest_release() -> Result<LatestReleaseCache> {
    let client = reqwest::Client::builder()
        .user_agent(format!("so-context/{}", env!("CARGO_PKG_VERSION")))
        .timeout(GITHUB_RELEASE_TIMEOUT)
        .connect_timeout(GITHUB_RELEASE_TIMEOUT)
        .build()
        .context("build release lookup client")?;

    let payload = client
        .get(GITHUB_LATEST_RELEASE_API)
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .context("fetch latest so-context release metadata")?
        .error_for_status()
        .context("latest release metadata endpoint returned an error")?
        .json::<GithubLatestReleaseResponse>()
        .await
        .context("decode latest so-context release metadata")?;

    Ok(LatestReleaseCache::new(
        payload.tag_name,
        if payload.html_url.is_empty() {
            GITHUB_LATEST_RELEASE_URL.to_string()
        } else {
            payload.html_url
        },
        payload
            .assets
            .into_iter()
            .map(|asset| ReleaseAssetCache {
                name: asset.name,
                download_url: asset.browser_download_url,
            })
            .collect(),
    ))
}

fn read_latest_release_cache() -> Result<Option<LatestReleaseCache>> {
    let path = github_release_cache_path();
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read release cache {}", path.display()))?;
    let cache = serde_json::from_str(&raw)
        .with_context(|| format!("parse release cache {}", path.display()))?;
    Ok(Some(cache))
}

fn write_latest_release_cache(cache: &LatestReleaseCache) -> Result<()> {
    let path = github_release_cache_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create version cache dir {}", parent.display()))?;
    }

    let raw = serde_json::to_string(cache).context("encode release cache")?;
    fs::write(&path, raw).with_context(|| format!("write release cache {}", path.display()))
}

fn current_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "version_tests.rs"]
mod version_tests;
