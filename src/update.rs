//! Finds newer GitHub releases and installs them over the running copy.
use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const LATEST: &str = "https://api.github.com/repos/eitchtee/OpenUlanzi/releases/latest";
const USER_AGENT: &str = concat!("OpenUlanzi/", env!("CARGO_PKG_VERSION"));

/// How this copy got here, which decides how it updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Install {
    Installed,
    Portable,
    /// Built from source: replacing it would only confuse cargo.
    Dev,
}

pub fn install_kind(exe: &Path) -> Install {
    if exe.components().any(|part| part.as_os_str() == "target") {
        Install::Dev
    } else if exe.with_file_name("uninstall.exe").exists() {
        Install::Installed
    } else {
        Install::Portable
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Asset {
    url: String,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Update {
    pub version: String,
    pub notes_url: String,
    setup: Asset,
    portable: Asset,
}

fn version_parts(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.trim_start_matches('v').split('.').map(|p| p.parse().ok());
    let (major, minor, patch) = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some((major, minor, patch))
}

fn is_newer(candidate: &str, current: &str) -> bool {
    matches!((version_parts(candidate), version_parts(current)), (Some(a), Some(b)) if a > b)
}

fn asset(release: &serde_json::Value, name: &str) -> Result<Asset> {
    let assets = release["assets"].as_array().ok_or_else(|| anyhow!("release has no files"))?;
    let found = assets.iter().find(|a| a["name"] == name).ok_or_else(|| anyhow!("release has no {name}"))?;
    let text = |key: &str| found[key].as_str().ok_or_else(|| anyhow!("{name} has no {key}"));
    let sha256 = text("digest")?.strip_prefix("sha256:").ok_or_else(|| anyhow!("{name} has no SHA-256"))?;
    Ok(Asset { url: text("browser_download_url")?.to_string(), sha256: sha256.to_string() })
}

fn parse_release(body: &str, current: &str) -> Result<Option<Update>> {
    let release: serde_json::Value = serde_json::from_str(body).context("reading the release")?;
    let tag = release["tag_name"].as_str().ok_or_else(|| anyhow!("release has no tag"))?;
    if !is_newer(tag, current) {
        return Ok(None);
    }
    let version = tag.trim_start_matches('v').to_string();
    Ok(Some(Update {
        setup: asset(&release, &format!("OpenUlanzi-{version}-setup.exe"))?,
        portable: asset(&release, &format!("OpenUlanzi-{version}-portable.exe"))?,
        notes_url: release["html_url"].as_str().unwrap_or_default().to_string(),
        version,
    }))
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(300))).build().into()
}

/// The latest release, if it is newer than `current`.
pub fn check(current: &str) -> Result<Option<Update>> {
    let mut response = agent().get(LATEST).header("User-Agent", USER_AGENT).call().context("reaching GitHub")?;
    parse_release(&response.body_mut().read_to_string()?, current)
}

fn verified(bytes: &[u8], sha256: &str) -> Result<()> {
    let actual: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
    if !actual.eq_ignore_ascii_case(sha256) {
        bail!("the download is damaged: its checksum doesn't match the release");
    }
    Ok(())
}

fn download(asset: &Asset, to: &Path) -> Result<()> {
    let mut response = agent().get(&asset.url).header("User-Agent", USER_AGENT).call().context("downloading")?;
    let bytes = response.body_mut().with_config().limit(200 * 1024 * 1024).read_to_vec()?;
    verified(&bytes, &asset.sha256)?;
    std::fs::write(to, bytes).with_context(|| format!("writing {}", to.display()))
}

fn old_copy(exe: &Path) -> PathBuf {
    exe.with_extension("exe.old")
}

/// Windows lets a running exe be renamed, just not replaced, so the old one moves aside.
fn swap_exe(new: &Path, exe: &Path) -> Result<()> {
    let old = old_copy(exe);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old).context("moving the current version aside")?;
    if let Err(e) = std::fs::rename(new, exe) {
        std::fs::rename(&old, exe).context("putting the current version back")?;
        return Err(e).context("putting the new version in place");
    }
    Ok(())
}

/// Removes what a portable update left behind. The old exe can be deleted once it has exited.
pub fn clean_up(exe: &Path) {
    let _ = std::fs::remove_file(old_copy(exe));
}

/// Downloads the build matching this copy and starts it. The caller must exit straight after.
pub fn install(update: &Update, exe: &Path) -> Result<()> {
    match install_kind(exe) {
        Install::Dev => bail!("this copy was built from source, so update it with git"),
        Install::Installed => {
            let setup = std::env::temp_dir().join(format!("OpenUlanzi-{}-setup.exe", update.version));
            download(&update.setup, &setup)?;
            // Passive shows progress without asking anything; /R starts the app when it's done.
            Command::new(setup).args(["/P", "/UPDATE", "/R"]).spawn().context("starting the installer")?;
        }
        Install::Portable => {
            let new = exe.with_extension("exe.new");
            download(&update.portable, &new).context("is this folder writable?")?;
            swap_exe(&new, exe)?;
            let pid = std::process::id().to_string();
            Command::new(exe).args(["--after-update", &pid]).spawn().context("starting the new version")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELEASE: &str = r#"{
        "tag_name": "v1.2.0",
        "html_url": "https://example.com/notes",
        "assets": [
            { "name": "OpenUlanzi-1.2.0-portable.exe", "digest": "sha256:aa", "browser_download_url": "https://example.com/p" },
            { "name": "OpenUlanzi-1.2.0-setup.exe", "digest": "sha256:bb", "browser_download_url": "https://example.com/s" }
        ]
    }"#;

    #[test]
    fn versions_compare_as_numbers() {
        assert!(is_newer("v1.10.0", "1.9.0"), "not as text");
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(!is_newer("v1.1.0", "1.1.0"));
        assert!(!is_newer("v1.0.9", "1.1.0"));
        assert!(!is_newer("nightly", "1.1.0"));
        assert!(!is_newer("1.2", "1.1.0"));
    }

    #[test]
    fn a_newer_release_gives_both_builds() {
        let update = parse_release(RELEASE, "1.1.0").unwrap().unwrap();
        assert_eq!(update.version, "1.2.0");
        assert_eq!(update.setup, Asset { url: "https://example.com/s".into(), sha256: "bb".into() });
        assert_eq!(update.portable.sha256, "aa");
        assert_eq!(parse_release(RELEASE, "1.2.0").unwrap(), None);
    }

    #[test]
    fn a_release_without_checksums_is_refused() {
        let unsafe_release = RELEASE.replace("sha256:bb", "unknown");
        assert!(parse_release(&unsafe_release, "1.1.0").is_err());
        assert!(parse_release(&RELEASE.replace(r#""digest": "sha256:aa","#, ""), "1.1.0").is_err());
    }

    #[test]
    fn damaged_downloads_are_rejected() {
        let hello = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(verified(b"hello", hello).is_ok());
        assert!(verified(b"hellp", hello).is_err());
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openulanzi-update-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_kind_of_install_comes_from_where_it_lives() {
        let dir = temp("kind");
        let exe = dir.join("app.exe");
        assert_eq!(install_kind(&exe), Install::Portable);
        std::fs::write(dir.join("uninstall.exe"), b"").unwrap();
        assert_eq!(install_kind(&exe), Install::Installed);
        assert_eq!(install_kind(Path::new(r"C:\code\target\release\app.exe")), Install::Dev);
    }

    #[test]
    fn swapping_keeps_the_old_exe_until_cleaned_up() {
        let dir = temp("swap");
        let (exe, new) = (dir.join("app.exe"), dir.join("app.exe.new"));
        std::fs::write(&exe, b"old").unwrap();
        std::fs::write(&new, b"new").unwrap();
        swap_exe(&new, &exe).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        assert_eq!(std::fs::read(old_copy(&exe)).unwrap(), b"old");
        clean_up(&exe);
        assert!(!old_copy(&exe).exists());
    }

    #[test]
    fn a_failed_swap_restores_the_current_version() {
        let dir = temp("restore");
        let exe = dir.join("app.exe");
        std::fs::write(&exe, b"old").unwrap();
        assert!(swap_exe(&dir.join("missing.exe"), &exe).is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"old");
    }
}
