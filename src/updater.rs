//! Self-updater — check GitHub releases and replace the running binary.
//!
//! Usage: `lens update [--check]`
//!
//! Flow:
//!   1. GET https://api.github.com/repos/fatmuh/lens/releases/latest
//!   2. Compare tag (vX.Y.Z) with current version
//!   3. Download the platform-matching asset (tar.gz / zip)
//!   4. Extract and replace the current binary

use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use owo_colors::OwoColorize;
use serde::Deserialize;

use crate::cli::UpdateArgs;

const REPO: &str = "fatmuh/lens";
const API_URL: &str = "https://api.github.com/repos/fatmuh/lens/releases/latest";

// ── GitHub API types ──────────────────────────────────────────────

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    name: String,
    html_url: String,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

// ── Platform detection ───────────────────────────────────────────

fn platform_target() -> &'static str {
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "x86_64-apple-darwin"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        "x86_64-pc-windows-msvc"
    } else {
        "unknown"
    }
}

/// Find the matching asset for this platform.
fn find_asset<'a>(assets: &'a [GhAsset], target: &str) -> Option<&'a GhAsset> {
    assets.iter().find(|a| a.name.contains(target))
}

// ── Version comparison ───────────────────────────────────────────

/// Parse "v0.4.0" → "0.4.0". Returns the stripped version string.
fn strip_v(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

/// Compare two semver-like version strings. Returns true if `remote > local`.
fn is_newer(remote: &str, local: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.parse().ok())
            .collect::<Vec<_>>()
    };
    let r = parse(remote);
    let l = parse(local);
    r > l
}

// ── Download & extract ───────────────────────────────────────────

fn download(client: &reqwest::blocking::Client, url: &str) -> anyhow::Result<Vec<u8>> {
    let resp = client
        .get(url)
        .header("User-Agent", "lens-updater")
        .send()
        .context("download request failed")?;
    if !resp.status().is_success() {
        bail!("download failed with status {}", resp.status());
    }
    let bytes = resp.bytes().context("reading response body")?;
    Ok(bytes.to_vec())
}

/// Extract the lens binary from a .tar.gz or .zip archive.
fn extract_binary(data: &[u8], asset_name: &str) -> anyhow::Result<Vec<u8>> {
    let is_zip = asset_name.ends_with(".zip");
    let is_tar_gz = asset_name.ends_with(".tar.gz") || asset_name.ends_with(".tgz");

    if is_tar_gz {
        extract_from_tar_gz(data)
    } else if is_zip {
        extract_from_zip(data)
    } else {
        bail!("unsupported archive format: {}", asset_name);
    }
}

fn extract_from_tar_gz(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    let decoder = flate2::read::GzDecoder::new(Cursor::new(data));
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry.context("reading tar entry")?;
        let path = entry.path().context("tar entry path")?;
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // Match "lens" or "lens.exe"
        if name == "lens" || name == "lens.exe" {
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }
    bail!("lens binary not found in tar.gz archive");
}

fn extract_from_zip(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    let reader = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader).context("opening zip archive")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        let basename = Path::new(&name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if basename == "lens" || basename == "lens.exe" {
            let mut buf = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }
    bail!("lens binary not found in zip archive");
}

// ── Self-replace ─────────────────────────────────────────────────

/// Find the path of the currently running lens binary.
fn current_exe() -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe().context("cannot determine current executable path")?;
    Ok(exe)
}

/// Replace the current binary with the new one.
///
/// On Windows we can't overwrite a running exe, so we:
///   1. Rename the old exe to `lens.exe.old`
///   2. Write the new exe to the original path
///   3. The .old file is cleaned up on next run or manually
fn replace_binary(current: &Path, new_data: &[u8]) -> anyhow::Result<()> {
    // Write new binary to a temp file first (atomic-ish)
    let tmp_path = current.with_extension("tmp");

    {
        let mut f = std::fs::File::create(&tmp_path)
            .with_context(|| format!("creating temp file {}", tmp_path.display()))?;
        f.write_all(new_data).context("writing new binary")?;
        f.flush()?;
    }

    #[cfg(unix)]
    {
        // Set executable permission
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))?;
    }

    // On Windows, rename current to .old first (can't overwrite running exe)
    #[cfg(target_os = "windows")]
    {
        let old_path = current.with_extension("old");
        let _ = std::fs::remove_file(&old_path); // clean previous .old
        std::fs::rename(current, &old_path)
            .with_context(|| format!("renaming {} → {}", current.display(), old_path.display()))?;
    }

    // Move new binary into place
    std::fs::rename(&tmp_path, current)
        .with_context(|| format!("renaming {} → {}", tmp_path.display(), current.display()))?;

    Ok(())
}

// ── Public entry point ───────────────────────────────────────────

pub fn run_update(args: UpdateArgs) -> anyhow::Result<std::process::ExitCode> {
    let current_version = env!("CARGO_PKG_VERSION");
    let target = platform_target();

    println!(
        "  {} Checking for updates... (current: v{})",
        "\u{1F50D}", current_version
    );

    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(API_URL)
        .header("User-Agent", "lens-updater")
        .send()
        .context("failed to reach GitHub API — are you offline?")?;

    if !resp.status().is_success() {
        let status = resp.status();
        bail!("GitHub API returned {} — try again later", status);
    }

    let release: GhRelease = resp
        .json()
        .context("failed to parse GitHub release response")?;

    let remote_version = strip_v(&release.tag_name);

    if !is_newer(remote_version, current_version) {
        println!(
            "  {} Already up to date — v{}",
            "\u{2705}".green(),
            current_version
        );
        return Ok(std::process::ExitCode::SUCCESS);
    }

    println!(
        "  {} New version available: {} → {}",
        "\u{1F4E5}",
        format!("v{}", current_version).dimmed(),
        release.tag_name.bold().green()
    );
    println!("  {} {}", "\u{1F517}", release.html_url.dimmed());

    // --check: just report, don't download
    if args.check {
        println!(
            "  {} Run {} to install.",
            "\u{2139}\u{FE0F}",
            "lens update".bold()
        );
        return Ok(std::process::ExitCode::SUCCESS);
    }

    // Find matching asset
    let asset = find_asset(&release.assets, target).with_context(|| {
        format!(
            "no binary found for platform '{}' in release {}. Available:\n{}",
            target,
            release.tag_name,
            release
                .assets
                .iter()
                .map(|a| format!("  - {}", a.name))
                .collect::<Vec<_>>()
                .join("\n")
        )
    })?;

    println!(
        "  {} Downloading {} ({})...",
        "\u{2B07}\u{FE0F}",
        asset.name.bold(),
        format_bytes(asset.size).dimmed()
    );

    let archive_data = download(&client, &asset.browser_download_url)?;

    println!("  {} Extracting binary...", "\u{1F4E6}");

    let new_binary = extract_binary(&archive_data, &asset.name)?;

    let current = current_exe()?;
    println!("  {} Installing to {}...", "\u{1F527}", current.display());

    replace_binary(&current, &new_binary)?;

    println!(
        "  {} Successfully updated to {}!",
        "\u{2705}".green(),
        release.tag_name.bold().green()
    );

    Ok(std::process::ExitCode::SUCCESS)
}

// ── Helpers ──────────────────────────────────────────────────────

fn format_bytes(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    if size >= MB {
        format!("{:.1} MB", size as f64 / MB as f64)
    } else {
        format!("{:.0} KB", size as f64 / KB as f64)
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_v() {
        assert_eq!(strip_v("v0.4.0"), "0.4.0");
        assert_eq!(strip_v("0.4.0"), "0.4.0");
    }

    #[test]
    fn test_is_newer() {
        assert!(is_newer("0.5.0", "0.4.0"));
        assert!(is_newer("0.4.1", "0.4.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.4.0", "0.4.0"));
        assert!(!is_newer("0.3.9", "0.4.0"));
    }

    #[test]
    fn test_platform_target() {
        let t = platform_target();
        assert_ne!(t, "unknown", "platform should be detected");
    }

    #[test]
    fn test_find_asset() {
        let assets = vec![
            GhAsset {
                name: "lens-x86_64-pc-windows-msvc.zip".into(),
                browser_download_url: "".into(),
                size: 0,
            },
            GhAsset {
                name: "lens-x86_64-unknown-linux-gnu.tar.gz".into(),
                browser_download_url: "".into(),
                size: 0,
            },
            GhAsset {
                name: "checksums.txt".into(),
                browser_download_url: "".into(),
                size: 0,
            },
        ];
        assert!(find_asset(&assets, "x86_64-pc-windows-msvc").is_some());
        assert!(find_asset(&assets, "x86_64-unknown-linux-gnu").is_some());
        assert!(find_asset(&assets, "aarch64-apple-darwin").is_none());
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(4_000_000), "3.8 MB");
    }
}
