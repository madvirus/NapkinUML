//! Updates from GitHub Releases.
//!
//! After start the program asks GitHub (in the background) for the latest release. When it
//! is newer than this program and has a build for this computer, the person is asked
//! whether to update. Nothing is said when the check fails (no internet, GitHub down...).
//!
//! Updating downloads the build, checks its size and SHA-256 (when GitHub gives one), and
//! puts it in place of the running program right away: the running program is renamed out
//! of the way (Windows and macOS both allow that), so the next start is the new version.
//! The renamed old program is deleted at the next start.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Duration;

/// The GitHub repository the releases come from.
const REPO: &str = "madvirus/NapkinUML";

/// This program's version (from Cargo.toml).
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// The release file built for this computer.
#[cfg(target_os = "windows")]
const ASSET: &str = if cfg!(target_arch = "aarch64") { "NapkinUML-arm64.exe" } else { "NapkinUML-x64.exe" };
#[cfg(target_os = "macos")]
const ASSET: &str = "NapkinUML-macos.zip";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const ASSET: &str = "";

/// A version number: major.minor.patch, and whether it has a pre-release part
/// ("0.2.1-SNAPSHOT" comes before "0.2.1").
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    nums: [u64; 3],
    release: bool,
}

impl Version {
    pub fn parse(s: &str) -> Option<Version> {
        let s = s.trim();
        let s = s.strip_prefix('v').or_else(|| s.strip_prefix('V')).unwrap_or(s);
        let (core, pre) = match s.split_once(['-', '+']) {
            Some((c, rest)) => (c, s[c.len()..].starts_with('-') && !rest.is_empty()),
            None => (s, false),
        };
        let mut nums = [0u64; 3];
        let mut parts = core.split('.');
        for (i, n) in nums.iter_mut().enumerate() {
            match parts.next() {
                Some(p) => *n = p.parse().ok()?,
                None if i > 0 => break,
                None => return None,
            }
        }
        if parts.next().is_some() {
            return None;
        }
        Some(Version { nums, release: !pre })
    }
}

/// A newer release with a build for this computer.
#[derive(Clone, Debug)]
pub struct Release {
    /// The version without the leading "v", e.g. "0.3.0".
    pub version: String,
    /// The release's page on GitHub.
    pub page: String,
    asset_url: String,
    asset_size: u64,
    /// Lower-case hex SHA-256 of the build, when GitHub gives it.
    sha256: Option<String>,
}

/// The page listing every release (used when no release page is known).
pub fn releases_page() -> String {
    format!("https://github.com/{REPO}/releases/latest")
}

fn agent(timeout: Option<Duration>) -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    let tls = TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(timeout)
        .user_agent(format!("NapkinUML/{CURRENT}"))
        .build()
        .into()
}

/// Ask GitHub for the latest release in the background. The answer is `Some` only when it
/// is newer than this program and has a build for this computer; every failure is `None`.
pub fn check(ctx: egui::Context) -> Receiver<Option<Release>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(latest().ok().flatten());
        ctx.request_repaint();
    });
    rx
}

fn latest() -> Result<Option<Release>, String> {
    latest_since(CURRENT)
}

fn latest_since(current: &str) -> Result<Option<Release>, String> {
    if ASSET.is_empty() {
        return Ok(None);
    }
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let text = agent(Some(Duration::from_secs(20)))
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if v["draft"].as_bool() == Some(true) || v["prerelease"].as_bool() == Some(true) {
        return Ok(None);
    }
    let tag = v["tag_name"].as_str().unwrap_or_default();
    let (Some(new), Some(cur)) = (Version::parse(tag), Version::parse(current)) else { return Ok(None) };
    if new <= cur {
        return Ok(None);
    }
    let Some(asset) = v["assets"].as_array().and_then(|a| a.iter().find(|a| a["name"].as_str() == Some(ASSET))) else { return Ok(None) };
    let Some(asset_url) = asset["browser_download_url"].as_str() else { return Ok(None) };
    let sha256 = asset["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).map(|h| h.to_ascii_lowercase());
    Ok(Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        page: v["html_url"].as_str().map(String::from).unwrap_or_else(releases_page),
        asset_url: asset_url.to_string(),
        asset_size: asset["size"].as_u64().unwrap_or(0),
        sha256,
    }))
}

/// A download running in the background.
pub struct Job {
    /// Bytes received so far, and the whole size (0 when unknown).
    pub done: Arc<AtomicU64>,
    pub total: u64,
    pub cancel: Arc<AtomicBool>,
    /// The program to start for the new version, or what went wrong.
    pub rx: Receiver<Result<PathBuf, String>>,
}

/// Download the release and put it in place of this program.
pub fn download_and_install(rel: &Release, ctx: egui::Context) -> Job {
    let (tx, rx) = channel();
    let done = Arc::new(AtomicU64::new(0));
    let cancel = Arc::new(AtomicBool::new(false));
    let job = Job { done: done.clone(), total: rel.asset_size, cancel: cancel.clone(), rx };
    let rel = rel.clone();
    std::thread::spawn(move || {
        let result = install(&rel, &done, &cancel, &ctx);
        let _ = tx.send(result);
        ctx.request_repaint();
    });
    job
}

/// Download to `dest`, checking the size and SHA-256.
fn download(rel: &Release, dest: &Path, done: &AtomicU64, cancel: &AtomicBool, ctx: &egui::Context) -> Result<(), String> {
    let mut resp = agent(None).get(&rel.asset_url).call().map_err(|e| e.to_string())?;
    let mut reader = resp.body_mut().with_config().limit(1 << 30).reader();
    let mut file = std::fs::File::create(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut n_total = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| format!("{}: {e}", dest.display()))?;
        hasher.update(&buf[..n]);
        n_total += n as u64;
        done.store(n_total, Ordering::Relaxed);
        ctx.request_repaint();
    }
    file.sync_all().map_err(|e| e.to_string())?;
    if rel.asset_size > 0 && n_total != rel.asset_size {
        return Err(format!("download incomplete ({n_total} of {} bytes)", rel.asset_size));
    }
    if let Some(want) = &rel.sha256 {
        let got: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if &got != want {
            return Err("the downloaded file is damaged (SHA-256 mismatch)".into());
        }
    }
    Ok(())
}

/// `<path>` + `suffix` (e.g. NapkinUML.exe -> NapkinUML.exe.old).
fn with_suffix(p: &Path, suffix: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

#[cfg(target_os = "windows")]
fn install(rel: &Release, done: &AtomicU64, cancel: &AtomicBool, ctx: &egui::Context) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let new = with_suffix(&exe, ".new");
    let old = with_suffix(&exe, ".old");
    if let Err(e) = download(rel, &new, done, cancel, ctx) {
        let _ = std::fs::remove_file(&new);
        return Err(e);
    }
    // a running program can be renamed but not overwritten
    let _ = std::fs::remove_file(&old);
    if let Err(e) = std::fs::rename(&exe, &old) {
        let _ = std::fs::remove_file(&new);
        return Err(format!("{}: {e}", exe.display()));
    }
    if let Err(e) = std::fs::rename(&new, &exe) {
        let _ = std::fs::rename(&old, &exe);
        let _ = std::fs::remove_file(&new);
        return Err(format!("{}: {e}", exe.display()));
    }
    Ok(exe)
}

/// The NapkinUML.app this program runs from.
#[cfg(target_os = "macos")]
fn app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let bundle = exe.parent()?.parent()?.parent()?.to_path_buf();
    (bundle.extension().is_some_and(|e| e == "app")).then_some(bundle)
}

#[cfg(target_os = "macos")]
fn install(rel: &Release, done: &AtomicU64, cancel: &AtomicBool, ctx: &egui::Context) -> Result<PathBuf, String> {
    let bundle = app_bundle().ok_or("NapkinUML is not running from an .app bundle")?;
    let dir = bundle.parent().ok_or("no folder")?;
    // work next to the bundle so the final rename stays on the same disk
    let work = dir.join(".NapkinUML-update");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
    let result = (|| {
        let zip = work.join(ASSET);
        download(rel, &zip, done, cancel, ctx)?;
        let out = work.join("app");
        let status = std::process::Command::new("ditto").arg("-x").arg("-k").arg(&zip).arg(&out).status().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("could not unpack the update".to_string());
        }
        let new_app = std::fs::read_dir(&out)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "app"))
            .ok_or("no .app in the update")?;
        let old = old_bundle(&bundle);
        let _ = std::fs::remove_dir_all(&old);
        std::fs::rename(&bundle, &old).map_err(|e| format!("{}: {e}", bundle.display()))?;
        if let Err(e) = std::fs::rename(&new_app, &bundle) {
            let _ = std::fs::rename(&old, &bundle);
            return Err(format!("{}: {e}", bundle.display()));
        }
        let _ = std::fs::remove_dir_all(&old);
        Ok(bundle.clone())
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

#[cfg(target_os = "macos")]
fn old_bundle(bundle: &Path) -> PathBuf {
    let name = bundle.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    bundle.with_file_name(format!(".{name}.old"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn install(_rel: &Release, _done: &AtomicU64, _cancel: &AtomicBool, _ctx: &egui::Context) -> Result<PathBuf, String> {
    Err("updates are not supported on this system".into())
}

/// Delete what an earlier update left behind (the old program it replaced).
pub fn cleanup_old() {
    #[cfg(target_os = "windows")]
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(with_suffix(&exe, ".old"));
        let _ = std::fs::remove_file(with_suffix(&exe, ".new"));
    }
    #[cfg(target_os = "macos")]
    if let Some(b) = app_bundle() {
        let _ = std::fs::remove_dir_all(old_bundle(&b));
    }
}

/// Start the updated program, opening `project` in it.
pub fn relaunch(target: &Path, project: Option<&Path>) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let mut c = std::process::Command::new("open");
        c.arg("-n").arg(target);
        if let Some(p) = project {
            c.arg("--args").arg(p);
        }
        c.spawn().map(|_| ())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut c = std::process::Command::new(target);
        if let Some(p) = project {
            c.arg(p);
        }
        c.spawn().map(|_| ())
    }
}

/// Open a web page in the default browser.
pub fn open_page(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

#[cfg(test)]
mod tests {
    use super::Version;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn compares_versions() {
        assert!(v("v0.2.1") > v("0.2.0"));
        assert!(v("0.2.1") > v("0.2.1-SNAPSHOT"));
        assert!(v("0.2.1-SNAPSHOT") > v("0.2.0"));
        assert!(v("v1.0") > v("0.9.9"));
        assert!(v("0.10.0") > v("0.9.0"));
        assert_eq!(v("v0.2.0"), v("0.2.0"));
        assert_eq!(v("0.2.0+build5"), v("0.2.0"));
    }

    #[test]
    fn rejects_bad_versions() {
        assert!(Version::parse("").is_none());
        assert!(Version::parse("latest").is_none());
        assert!(Version::parse("1.2.3.4").is_none());
    }
}

/// Talks to GitHub: `cargo test --bin NapkinUML update -- --ignored`
#[cfg(test)]
mod net_tests {
    use super::*;

    #[test]
    #[ignore]
    fn finds_and_downloads_a_release() {
        let rel = latest_since("0.0.1").expect("GitHub answered").expect("a release with a build for this computer");
        assert!(Version::parse(&rel.version).is_some());
        let dir = std::env::temp_dir().join("napkinuml-update-test");
        let _ = std::fs::create_dir_all(&dir);
        let dest = dir.join(ASSET);
        let ctx = egui::Context::default();
        download(&rel, &dest, &AtomicU64::new(0), &AtomicBool::new(false), &ctx).expect("download checks out");
        assert_eq!(std::fs::metadata(&dest).unwrap().len(), rel.asset_size);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(latest_since("999.0.0").unwrap().is_none());
    }
}
