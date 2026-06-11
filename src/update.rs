use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

use crate::version::{current_release_target, latest_release_info_for_update, release_asset_name};

const BIN_NAME: &str = "so-context";
const RELEASE_REPO: &str = "vestin-io/so-context";

pub async fn update_current_binary() -> Result<()> {
    let invocation_path =
        std::env::current_exe().context("resolve the current so-context binary path")?;
    let install_path = invocation_path
        .canonicalize()
        .unwrap_or_else(|_| invocation_path.clone());
    let target = current_release_target()?;
    let release = latest_release_info_for_update().await?;

    let temp_dir = create_temp_dir()?;
    let _guard = TempDirGuard {
        path: temp_dir.clone(),
    };
    ensure_command_available("tar")?;

    let asset_name = release_asset_name(&target);
    if let Some(asset) = release.assets.iter().find(|asset| asset.name == asset_name) {
        let tarball_path = temp_dir.join(&asset.name);
        download_to_path(&asset.download_url, &tarball_path).await?;

        let extract_dir = temp_dir.join("extract");
        fs::create_dir_all(&extract_dir)
            .with_context(|| format!("create temp extract dir {}", extract_dir.display()))?;
        extract_tarball(&tarball_path, &extract_dir)?;

        let extracted_binary = find_binary(&extract_dir)?;
        install_binary(&extracted_binary, &install_path)?;
    } else {
        install_from_source_release(&release.tag_name, &temp_dir, &install_path).await?;
    }

    println!("Updated so-context to the latest release.");
    println!("binary: {}", install_path.display());
    println!("release: {}", release.tag_name);
    if invocation_path != install_path {
        println!("invocation path: {}", invocation_path.display());
    }
    println!("Restart the daemon if you want it to pick up the new release.");
    Ok(())
}

async fn download_to_path(url: &str, output_path: &Path) -> Result<()> {
    let response = reqwest::Client::builder()
        .user_agent(format!("so-context/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(15))
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()
        .context("build release download client")?
        .get(url)
        .send()
        .await
        .with_context(|| format!("download release archive from {url}"))?
        .error_for_status()
        .with_context(|| format!("release download failed for {url}"))?;

    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("read release archive bytes from {url}"))?;

    fs::write(output_path, &bytes)
        .with_context(|| format!("write temp archive {}", output_path.display()))?;
    Ok(())
}

#[cfg(test)]
fn latest_release_asset_url(target: &str) -> String {
    format!("https://github.com/{RELEASE_REPO}/releases/latest/download/{BIN_NAME}-{target}.tar.gz")
}

fn release_source_archive_url(tag_name: &str) -> String {
    format!("https://github.com/{RELEASE_REPO}/archive/refs/tags/{tag_name}.tar.gz")
}

fn extract_tarball(tarball_path: &Path, extract_dir: &Path) -> Result<()> {
    let status = std::process::Command::new("tar")
        .args(["-xzf"])
        .arg(tarball_path)
        .args(["-C"])
        .arg(extract_dir)
        .status()
        .with_context(|| format!("extract release tarball {}", tarball_path.display()))?;
    if !status.success() {
        bail!(
            "failed to extract release tarball {} (status: {status})",
            tarball_path.display()
        );
    }
    Ok(())
}

fn find_binary(root: &Path) -> Result<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let entries = fs::read_dir(&path)
            .with_context(|| format!("read extracted dir {}", path.display()))?;
        for entry in entries {
            let entry = entry.with_context(|| format!("read entry under {}", path.display()))?;
            let entry_path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("inspect extracted entry {}", entry_path.display()))?;
            if file_type.is_dir() {
                stack.push(entry_path);
                continue;
            }
            if file_type.is_file()
                && entry_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name == BIN_NAME)
            {
                return Ok(entry_path);
            }
        }
    }

    bail!("latest release archive did not contain a {BIN_NAME} binary")
}

async fn install_from_source_release(
    tag_name: &str,
    temp_dir: &Path,
    destination: &Path,
) -> Result<()> {
    ensure_command_available("cargo")?;

    let archive_path = temp_dir.join(format!("{BIN_NAME}-{tag_name}-source.tar.gz"));
    download_to_path(&release_source_archive_url(tag_name), &archive_path).await?;

    let extract_dir = temp_dir.join("source-extract");
    fs::create_dir_all(&extract_dir)
        .with_context(|| format!("create source extract dir {}", extract_dir.display()))?;
    extract_tarball(&archive_path, &extract_dir)?;

    let source_root = find_source_root(&extract_dir)?;
    let install_root = temp_dir.join("install-root");
    cargo_install_from_source(&source_root, &install_root)?;

    let installed_binary = install_root.join("bin").join(BIN_NAME);
    if !installed_binary.exists() {
        bail!(
            "cargo install did not produce {}",
            installed_binary.display()
        );
    }

    install_binary(&installed_binary, destination)
}

fn find_source_root(root: &Path) -> Result<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let entries = fs::read_dir(&path)
            .with_context(|| format!("read extracted dir {}", path.display()))?;
        for entry in entries {
            let entry = entry.with_context(|| format!("read entry under {}", path.display()))?;
            let entry_path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("inspect extracted entry {}", entry_path.display()))?;
            if file_type.is_dir() {
                stack.push(entry_path);
                continue;
            }
            if file_type.is_file()
                && entry_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name == "Cargo.toml")
            {
                return entry_path
                    .parent()
                    .map(Path::to_path_buf)
                    .ok_or_else(|| anyhow!("source archive root had no parent"));
            }
        }
    }

    bail!("release source archive did not contain Cargo.toml")
}

fn cargo_install_from_source(source_root: &Path, install_root: &Path) -> Result<()> {
    let status = std::process::Command::new("cargo")
        .args(["install", "--locked", "--path"])
        .arg(source_root)
        .args(["--root"])
        .arg(install_root)
        .arg("--force")
        .status()
        .with_context(|| format!("build release from source {}", source_root.display()))?;
    if !status.success() {
        bail!(
            "cargo install failed for release source {} (status: {status})",
            source_root.display()
        );
    }
    Ok(())
}

fn install_binary(source: &Path, destination: &Path) -> Result<()> {
    let Some(parent) = destination.parent() else {
        bail!(
            "cannot update {} without a parent directory",
            destination.display()
        );
    };
    fs::create_dir_all(parent)
        .with_context(|| format!("create binary dir {}", parent.display()))?;

    let staging_path = parent.join(format!("{BIN_NAME}.update-tmp"));
    fs::copy(source, &staging_path).with_context(|| {
        format!(
            "stage updated binary from {} to {}",
            source.display(),
            staging_path.display()
        )
    })?;
    set_executable_permissions(&staging_path)?;
    fs::rename(&staging_path, destination).with_context(|| {
        format!(
            "replace current binary {} with {}",
            destination.display(),
            staging_path.display()
        )
    })?;
    Ok(())
}

#[cfg(unix)]
fn set_executable_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .with_context(|| format!("read metadata for {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("set executable permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_executable_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

fn create_temp_dir() -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!("so-context-update-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).with_context(|| format!("create temp dir {}", path.display()))?;
    Ok(path)
}

fn ensure_command_available(command: &str) -> Result<()> {
    let status = std::process::Command::new(command)
        .arg("--version")
        .status()
        .with_context(|| format!("check required command {command}"))?;
    if !status.success() {
        bail!("required command {command} is not available");
    }
    Ok(())
}

struct TempDirGuard {
    path: PathBuf,
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod update_tests;
