//! Fetch OSS release metadata and select assets from GitHub Releases.
//! Platform modules download the selected installer and reveal it in the file manager.

use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context as _, Result};
use lazy_static::lazy_static;
use serde::Deserialize;

const REPO_OWNER: &str = "zerx-lab";
const REPO_NAME: &str = "warp";

// GitHub requires a User-Agent. Pin the API version to avoid changes to its default.
const USER_AGENT: &str = "dWarp-Autoupdate";
const ACCEPT: &str = "application/vnd.github+json";
const API_VERSION: &str = "2022-11-28";

const FETCH_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub html_url: String,
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

impl GithubRelease {
    pub fn version(&self) -> &str {
        self.tag_name.trim_start_matches('v')
    }

    pub fn find_asset(&self, expected_name: &str) -> Option<&GithubAsset> {
        self.assets.iter().find(|a| a.name == expected_name)
    }
}

lazy_static! {
    /// Cache release metadata between the version check and the download.
    /// This avoids a second request selecting a different release.
    static ref LATEST_RELEASE: Mutex<Option<GithubRelease>> = Mutex::new(None);
}

pub fn cached_release() -> Option<GithubRelease> {
    LATEST_RELEASE.lock().ok().and_then(|g| g.clone())
}

fn store_cached(release: GithubRelease) {
    if let Ok(mut guard) = LATEST_RELEASE.lock() {
        *guard = Some(release);
    }
}

pub async fn fetch_latest_release(client: &http_client::Client) -> Result<GithubRelease> {
    let url = format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/latest");
    log::info!("Fetching latest release from {url}");
    let release: GithubRelease = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", ACCEPT)
        .header("X-GitHub-Api-Version", API_VERSION)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .context("GitHub Releases API request failed")?
        .error_for_status()
        .context("GitHub Releases API returned an unsuccessful status")?
        .json()
        .await
        .context("Could not parse the GitHub Releases JSON response")?;
    log::info!(
        "GitHub latest release: tag={} assets={}",
        release.tag_name,
        release.assets.len()
    );
    store_cached(release.clone());
    Ok(release)
}
