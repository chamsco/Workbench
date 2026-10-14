use crate::model::data_dir;
use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Command;

struct ImageDef {
    url: &'static str,
    file: &'static str,
}

/// Image catalog. URLs verified against cloud-images.ubuntu.com listings.
fn wsl_image(os: &str) -> Result<ImageDef> {
    match os {
        "ubuntu-24.04" | "ubuntu" => Ok(ImageDef {
            url: "https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64-root.tar.xz",
            file: "ubuntu-24.04-root.tar.xz",
        }),
        other => bail!("unknown WSL image '{other}' (supported: ubuntu-24.04)"),
    }
}

fn hyperv_image(os: &str) -> Result<ImageDef> {
    match os {
        "ubuntu-24.04" | "ubuntu" => Ok(ImageDef {
            url: "https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64-azure.vhd.tar.gz",
            file: "ubuntu-24.04-azure.vhd.tar.gz",
        }),
        other => bail!("unknown Hyper-V image '{other}' (supported: ubuntu-24.04)"),
    }
}

fn cache_dir() -> Result<PathBuf> {
    let d = data_dir()?.join("images");
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

fn download(url: &str, dest: &PathBuf) -> Result<()> {
    if dest.exists() {
        return Ok(());
    }
    let tmp = dest.with_extension("partial");
    let status = Command::new("curl.exe")
        .args(["-fSL", "--retry", "3", "-o"])
        .arg(&tmp)
        .arg(url)
        .status()
        .context("failed to launch curl.exe")?;
    if !status.success() {
        let _ = std::fs::remove_file(&tmp);
        bail!("download failed: {url}");
    }
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

/// Ensure the WSL rootfs tarball for `os` is cached; returns path to a .tar
/// suitable for `wsl --import`.
pub fn ensure_wsl_rootfs(os: &str) -> Result<PathBuf> {
    let def = wsl_image(os)?;
    let tar = cache_dir()?.join(def.file.replace(".tar.xz", ".tar"));
    if tar.exists() {
        return Ok(tar);
    }
    let xz = cache_dir()?.join(def.file);
    download(def.url, &xz)?;
    // wsl --import wants a plain .tar on older builds: explode and re-tar with bsdtar
    let tmp = cache_dir()?.join("extract-tmp");
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp)?;
    }
    std::fs::create_dir_all(&tmp)?;
    run_tar(&["-xf", xz.to_str().context("bad path")?, "-C", tmp.to_str().unwrap()])?;
    run_tar(&["-cf", tar.to_str().unwrap(), "-C", tmp.to_str().unwrap(), "."])?;
    std::fs::remove_dir_all(&tmp)?;
    Ok(tar)
}

/// Ensure the Hyper-V base VHDX for `os` is cached (downloaded azure VHD,
/// converted to dynamic VHDX). Returns path to the base image.
pub fn ensure_hyperv_base(os: &str) -> Result<PathBuf> {
    let def = hyperv_image(os)?;
    let vhdx = cache_dir()?.join(def.file.replace(".vhd.tar.gz", ".vhdx"));
    if vhdx.exists() {
        return Ok(vhdx);
    }
    let tgz = cache_dir()?.join(def.file);
    download(def.url, &tgz)?;
    let dir = cache_dir()?;
    run_tar(&["-xf", tgz.to_str().unwrap(), "-C", dir.to_str().unwrap()])?;
    // the archive contains a single *.vhd; find it
    let vhd = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.extension().map(|e| e == "vhd").unwrap_or(false))
        .context("no .vhd found after extracting image archive")?;
    crate::powershell::ps(&format!(
        "Convert-VHD -Path {} -DestinationPath {} -VHDType Dynamic",
        crate::powershell::quote(&vhd.to_string_lossy()),
        crate::powershell::quote(&vhdx.to_string_lossy()),
    ))?;
    std::fs::remove_file(&vhd).ok();
    Ok(vhdx)
}

fn run_tar(args: &[&str]) -> Result<()> {
    let status = Command::new("tar.exe")
        .args(args)
        .status()
        .context("failed to launch tar.exe")?;
    if !status.success() {
        bail!("tar {:?} failed", args);
    }
    Ok(())
}

/// Clone an existing WSL distro into a tar we can import.
pub fn export_wsl_distro(distro: &str) -> Result<PathBuf> {
    let tar = cache_dir()?.join(format!("clone-{distro}.tar"));
    if tar.exists() {
        return Ok(tar);
    }
    let status = Command::new("wsl.exe")
        .args(["--export", distro])
        .arg(&tar)
        .status()
        .context("wsl --export failed")?;
    if !status.success() {
        bail!("wsl --export {distro} failed");
    }
    Ok(tar)
}
