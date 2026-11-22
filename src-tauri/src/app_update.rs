//! Looking for a newer Pack Creator on GitHub. The newest release of the repository is read (its tag, notes and the file to download), compared with the version this build is, and
//! the person is told. The download is the release's own file, taken from that repository only, saved in the Downloads folder and shown in Explorer: the running program is not
//! replaced by itself.

use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;

const REPO: &str = "sandyymandy/Pack-Creator";

#[derive(Serialize)]
pub struct Update {
    pub current: String,
    pub latest: String,
    pub newer: bool,
    /// The release's page, to read what is new.
    pub url: String,
    /// What the release says (its notes), trimmed.
    pub notes: String,
    /// The file to download (the program, or a zip of it), when the release has one.
    pub asset: Option<String>,
    pub asset_name: Option<String>,
}

/// The numbers of a version (`v1.2.3`, `1.2`), for comparing. A part that is not a number counts as 0.
fn numbers(version: &str) -> Vec<u64> {
    version.trim().trim_start_matches(|c| c == 'v' || c == 'V').split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse().unwrap_or(0)).collect()
}

/// Whether `latest` is a newer version than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let (a, b) = (numbers(latest), numbers(current));

    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));

        if x != y {
            return x > y;
        }
    }

    false
}

pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Reads the newest release.
pub fn check() -> Result<Update, String> {
    let body = crate::blockbench::curl(&["-sSL", "--max-time", "15", "-H", "User-Agent: pleasure-horizons-pack-creator", &format!("https://api.github.com/repos/{REPO}/releases/latest")])?;
    let release: Value = serde_json::from_slice(&body).map_err(|_| "GitHub did not answer with the release".to_string())?;
    let tag = release["tag_name"].as_str().ok_or("the repository has no release yet")?.to_string();
    let assets = release["assets"].as_array().cloned().unwrap_or_default();
    /* The program itself first, then an archive of it. */
    let asset = assets
        .iter()
        .find(|a| a["name"].as_str().map_or(false, |n| n.to_lowercase().ends_with(".exe")))
        .or_else(|| assets.iter().find(|a| a["name"].as_str().map_or(false, |n| n.to_lowercase().ends_with(".zip"))));
    let notes: String = release["body"].as_str().unwrap_or("").trim().chars().take(1500).collect();

    Ok(Update {
        current: current().to_string(),
        newer: is_newer(&tag, current()),
        latest: tag,
        url: release["html_url"].as_str().unwrap_or(&format!("https://github.com/{REPO}/releases")).to_string(),
        notes,
        asset: asset.and_then(|a| a["browser_download_url"].as_str()).map(String::from),
        asset_name: asset.and_then(|a| a["name"].as_str()).map(String::from),
    })
}

/// Whether an address is on this repository's GitHub (a page or a release file): the only places the Pack Creator opens or downloads from.
pub fn is_ours(url: &str) -> bool {
    url.starts_with(&format!("https://github.com/{REPO}/"))
}

/// The Downloads folder, or the temp folder when there is none.
fn downloads() -> PathBuf {
    std::env::var("USERPROFILE").map(|p| PathBuf::from(p).join("Downloads")).ok().filter(|p| p.is_dir()).unwrap_or_else(std::env::temp_dir)
}

/// Downloads a release file into the Downloads folder and shows it in Explorer. Returns where it went.
pub fn download(url: &str, name: &str) -> Result<String, String> {
    use std::os::windows::process::CommandExt;

    if !is_ours(url) {
        return Err("The Pack Creator only downloads from its own GitHub releases.".into());
    }

    /* The name is a file name, nothing more. */
    let name: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')).collect();

    if name.is_empty() || name.starts_with('.') {
        return Err("The release file has no usable name.".into());
    }

    let target = downloads().join(&name);

    crate::blockbench::curl(&["-sSL", "-f", "--max-time", "1800", "-o", &target.display().to_string(), url]).map_err(|e| format!("The update could not be downloaded: {e}"))?;
    std::process::Command::new("explorer").raw_arg(format!("/select,{}", target.display().to_string().replace('/', "\\"))).spawn().map_err(|e| format!("Explorer could not be opened: {e}"))?;
    Ok(target.display().to_string())
}

/// Opens one of the repository's pages in the browser.
pub fn open_page(url: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    if !is_ours(url) {
        return Err("The Pack Creator only opens its own GitHub pages.".into());
    }

    std::process::Command::new("explorer").raw_arg(url).spawn().map_err(|e| format!("The browser could not be opened: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_compared_by_their_numbers() {
        assert!(is_newer("v1.2.0", "1.1.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("2.0", "1.99.99"));
        assert!(!is_newer("1.1.0", "1.1.0"));
        assert!(!is_newer("v1.1", "1.1.0"));
        assert!(!is_newer("1.0.9", "1.1.0"));
        assert!(is_newer("1.1.1-beta", "1.1.0"));
    }

    #[test]
    fn only_its_own_pages_are_opened_or_downloaded() {
        assert!(is_ours("https://github.com/sandyymandy/Pack-Creator/releases/tag/v1.2.0"));
        assert!(!is_ours("https://github.com/sandyymandy/Pack-Creator.evil.com/x"));
        assert!(!is_ours("https://example.com/sandyymandy/Pack-Creator/"));
        assert!(!is_ours("http://github.com/sandyymandy/Pack-Creator/"));
        assert!(download("https://example.com/a.exe", "a.exe").is_err());
    }
}
