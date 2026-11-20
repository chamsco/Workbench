//! The llama.cpp build the engine runs: one pinned ggml-org release, the
//! variant that fits this machine, downloaded once and checked against its
//! sha256 before anything in it is executed.
//!
//! The pin and its hashes are the ones Atomic Chat's core ships
//! (`upstream-releases-baseline.json`, atomic-chat-core, MIT): a build someone
//! has already run on real hardware, not whatever `latest` is today. Moving
//! the pin is an edit here, reviewed like any other.
//!
//! Variants are ggml-org's own: Metal on Apple Silicon, Vulkan on Windows and
//! Linux (AMD, NVIDIA and Intel GPUs alike), CPU as the fallback. CUDA and
//! ROCm builds need a separate runtime on Windows and are left out; a
//! `llama-server` already on PATH (Homebrew, winget) is used when nothing is
//! installed here.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PINNED_TAG: &str = "b11463";
const DOWNLOAD_BASE: &str = "https://github.com/ggml-org/llama.cpp/releases/download";

/// One archive of the pinned release.
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
pub struct Asset {
    /// `macos-arm64`, `win-vulkan-x64`, `linux-cpu-x64`...
    pub variant: &'static str,
    /// "Metal", "Vulkan", "CPU", "CUDA 13.4".
    pub accel: &'static str,
    pub name: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

pub const ASSETS: &[Asset] = &[
    Asset {
        variant: "macos-arm64",
        accel: "Metal",
        name: "llama-b11463-bin-macos-arm64.tar.gz",
        sha256: "5fcb0237a9a6d247acfb91792249dde498e3f2231ed62a62851c5ca83eb230df",
        size: 12_087_702,
    },
    Asset {
        variant: "linux-vulkan-x64",
        accel: "Vulkan",
        name: "llama-b11463-bin-ubuntu-vulkan-x64.tar.gz",
        sha256: "5d428312d97e4c25eb28277416de935a76ca80bf7ede6b418d43a2b10c0336cd",
        size: 31_678_494,
    },
    Asset {
        variant: "linux-cpu-x64",
        accel: "CPU",
        name: "llama-b11463-bin-ubuntu-x64.tar.gz",
        sha256: "c7d63a3dfd1fe93a679b4bcd981dacb9153e2509a21bb912b9a01fc28a8447f5",
        size: 17_732_486,
    },
    Asset {
        variant: "linux-cuda-13.4-arm64",
        accel: "CUDA 13.4",
        name: "llama-b11463-bin-ubuntu-cuda-13.4-arm64.tar.gz",
        sha256: "6a7580834b0679e02ad17da07905bba9fbb50c5dd04dbe1e3b95e518c536aa66",
        size: 147_667_327,
    },
    Asset {
        variant: "linux-vulkan-arm64",
        accel: "Vulkan",
        name: "llama-b11463-bin-ubuntu-vulkan-arm64.tar.gz",
        sha256: "b3a97c347ef9284f8a82f433679f2daaa10310816affaaa99c94fb449f086698",
        size: 24_887_749,
    },
    Asset {
        variant: "linux-cpu-arm64",
        accel: "CPU",
        name: "llama-b11463-bin-ubuntu-arm64.tar.gz",
        sha256: "95e17be13037fafbed43c7afd75230ce04defb941d066062d0aff4bb3d52caf4",
        size: 13_724_995,
    },
    Asset {
        variant: "win-vulkan-x64",
        accel: "Vulkan",
        name: "llama-b11463-bin-win-vulkan-x64.zip",
        sha256: "c951fef0f0158bf4b6d8aa75c99a6209658bcbe75fd349d6d2b1b08672933a35",
        size: 31_007_237,
    },
    Asset {
        variant: "win-cpu-x64",
        accel: "CPU",
        name: "llama-b11463-bin-win-cpu-x64.zip",
        sha256: "61fa0338763addfe9e7e71fca1f0fdd2ee8b636abf016bae167aab7973a13728",
        size: 17_636_283,
    },
    Asset {
        variant: "win-cpu-arm64",
        accel: "CPU",
        name: "llama-b11463-bin-win-cpu-arm64.zip",
        sha256: "3c0d239d4573b2dfda51d6832327114a78bf9913300efdec44a535f79cd6d2b2",
        size: 10_424_801,
    },
];

/// The variants for an OS and CPU, best first. `gpu` says what this machine
/// can drive: whether a Vulkan loader is installed, whether an NVIDIA driver is.
pub fn variants_for(os: &str, arch: &str, vulkan: bool, nvidia: bool) -> Vec<&'static Asset> {
    let ids: &[&str] = match (os, arch) {
        ("macos", "aarch64") => &["macos-arm64"],
        ("linux", "x86_64") => &["linux-vulkan-x64", "linux-cpu-x64"],
        ("linux", "aarch64") => &[
            "linux-cuda-13.4-arm64",
            "linux-vulkan-arm64",
            "linux-cpu-arm64",
        ],
        ("windows", "x86_64") => &["win-vulkan-x64", "win-cpu-x64"],
        ("windows", "aarch64") => &["win-cpu-arm64"],
        _ => &[],
    };
    ids.iter()
        .filter(|id| vulkan || !id.contains("vulkan"))
        .filter(|id| nvidia || !id.contains("cuda"))
        .filter_map(|id| ASSETS.iter().find(|a| a.variant == *id))
        .collect()
}

/// [`variants_for`] this machine.
pub fn variants() -> Vec<&'static Asset> {
    variants_for(
        std::env::consts::OS,
        std::env::consts::ARCH,
        has_vulkan(),
        has_nvidia(),
    )
}

/// A Vulkan loader is installed: the GPU build can start at all.
fn has_vulkan() -> bool {
    if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        return Path::new(&root)
            .join("System32")
            .join("vulkan-1.dll")
            .exists();
    }
    [
        "/usr/lib/x86_64-linux-gnu/libvulkan.so.1",
        "/usr/lib/aarch64-linux-gnu/libvulkan.so.1",
        "/usr/lib64/libvulkan.so.1",
        "/usr/lib/libvulkan.so.1",
    ]
    .iter()
    .any(|p| Path::new(p).exists())
}

fn has_nvidia() -> bool {
    on_path("nvidia-smi").is_some()
}

/// A binary on PATH, with `.exe` on Windows.
pub fn on_path(bin: &str) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        format!("{bin}.exe")
    } else {
        bin.to_string()
    };
    std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p)
                .map(|d| d.join(&name))
                .find(|f| f.is_file())
        })
        .unwrap_or(None)
}

// ---------------------------------------------------------------- installed

/// A build installed under `<home>/backends/<tag>/<variant>/`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Installed {
    pub tag: String,
    pub variant: String,
    /// `llama-server`, as a path inside the variant's folder.
    pub server: String,
}

fn variant_dir(home: &Path, variant: &str) -> PathBuf {
    home.join("backends").join(PINNED_TAG).join(variant)
}

/// The builds installed for the pinned tag.
pub fn installed(home: &Path) -> Vec<Installed> {
    let Ok(rd) = std::fs::read_dir(home.join("backends").join(PINNED_TAG)) else {
        return vec![];
    };
    let mut v: Vec<Installed> = rd
        .flatten()
        .filter_map(|d| std::fs::read(d.path().join("installed.json")).ok())
        .filter_map(|b| serde_json::from_slice(&b).ok())
        .collect();
    v.sort_by(|a, b| a.variant.cmp(&b.variant));
    v
}

/// The `llama-server` to run: the `prefer`red installed variant, else the
/// best installed one for this machine, else one on PATH.
pub fn server_binary(home: &Path, prefer: Option<&str>) -> Option<PathBuf> {
    let have = installed(home);
    let order: Vec<&str> = prefer
        .into_iter()
        .chain(variants().iter().map(|a| a.variant))
        .collect();
    order
        .iter()
        .find_map(|v| have.iter().find(|i| i.variant == *v))
        .or(have.first())
        .map(|i| variant_dir(home, &i.variant).join(&i.server))
        .filter(|p| p.is_file())
        .or_else(|| on_path("llama-server"))
}

// ---------------------------------------------------------------- install

/// Download, verify and unpack one variant. `progress(done, total)` is called
/// as bytes arrive; `cancel` stops between chunks. A `.part` from an earlier
/// try is resumed. Nothing is unpacked unless the sha256 matches the pin.
pub fn install(
    home: &Path,
    variant: &str,
    progress: &dyn Fn(u64, u64),
    cancel: &AtomicBool,
) -> Result<Installed> {
    let asset = ASSETS
        .iter()
        .find(|a| a.variant == variant)
        .ok_or_else(|| anyhow!("no {variant} build in llama.cpp {PINNED_TAG}"))?;
    let dir = variant_dir(home, variant);
    if let Some(i) = installed(home).into_iter().find(|i| i.variant == variant) {
        return Ok(i);
    }
    let dl = home.join("backends").join(".download");
    std::fs::create_dir_all(&dl)?;
    let archive = dl.join(asset.name);
    if !archive.exists() {
        let url = format!("{DOWNLOAD_BASE}/{PINNED_TAG}/{}", asset.name);
        download(&url, &archive, asset.size, progress, cancel)?;
    }
    let got = sha256_file(&archive)?;
    if got != asset.sha256 {
        let _ = std::fs::remove_file(&archive);
        bail!(
            "{} failed its checksum (got {got}); deleted, nothing was run",
            asset.name
        );
    }
    let tmp = dir.with_extension("unpacking");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    unpack(&archive, &tmp).with_context(|| format!("unpacking {}", asset.name))?;
    let server =
        find_server(&tmp).ok_or_else(|| anyhow!("{} holds no llama-server", asset.name))?;
    let rel = server
        .strip_prefix(&tmp)?
        .to_string_lossy()
        .replace('\\', "/");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&tmp, &dir)?;
    let inst = Installed {
        tag: PINNED_TAG.into(),
        variant: variant.into(),
        server: rel,
    };
    std::fs::write(
        dir.join("installed.json"),
        serde_json::to_vec_pretty(&inst)?,
    )?;
    let _ = std::fs::remove_file(&archive);
    Ok(inst)
}

/// Remove an installed variant (a running model keeps its open files until it stops).
pub fn uninstall(home: &Path, variant: &str) -> Result<()> {
    if !ASSETS.iter().any(|a| a.variant == variant) {
        bail!("unknown build {variant}");
    }
    let dir = variant_dir(home, variant);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

fn download(
    url: &str,
    dest: &Path,
    size: u64,
    progress: &dyn Fn(u64, u64),
    cancel: &AtomicBool,
) -> Result<()> {
    let part = dest.with_extension("part");
    let have = std::fs::metadata(&part)
        .map(|m| m.len())
        .unwrap_or(0)
        .min(size);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .build()?;
        let mut rb = http.get(url);
        if have > 0 {
            rb = rb.header(reqwest::header::RANGE, format!("bytes={have}-"));
        }
        let mut resp = rb
            .send()
            .await
            .map_err(|e| anyhow!("can't reach {url}: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            bail!("{url}: {status}");
        }
        let resumed = status == reqwest::StatusCode::PARTIAL_CONTENT;
        let mut done = if resumed { have } else { 0 };
        let mut out = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(resumed)
            .truncate(!resumed)
            .open(&part)?;
        progress(done, size);
        while let Some(chunk) = resp.chunk().await? {
            if cancel.load(Ordering::SeqCst) {
                bail!("cancelled");
            }
            out.write_all(&chunk)?;
            done += chunk.len() as u64;
            progress(done, size);
        }
        out.flush()?;
        if done != size {
            bail!("the download stopped at {done} of {size} bytes; try again to resume");
        }
        Ok(())
    })?;
    std::fs::rename(&part, dest)?;
    Ok(())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// `.tar.gz` or `.zip` into `to`. Entries that would land outside it are
/// skipped (tar's own guard; zip's `enclosed_name`).
fn unpack(archive: &Path, to: &Path) -> Result<()> {
    let name = archive.to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".tar.gz") {
        let gz = flate2::read::GzDecoder::new(std::fs::File::open(archive)?);
        let mut t = tar::Archive::new(gz);
        t.set_preserve_permissions(true);
        t.unpack(to)?;
        return Ok(());
    }
    let mut z = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        let Some(rel) = e.enclosed_name() else {
            continue;
        };
        let out = to.join(rel);
        if e.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::io::copy(&mut e, &mut std::fs::File::create(&out)?)?;
        #[cfg(unix)]
        if let Some(mode) = e.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode))?;
        }
    }
    Ok(())
}

/// `llama-server` anywhere in an unpacked build (ggml-org nests it in `build/bin/` or a `llama-<tag>/` folder).
fn find_server(dir: &Path) -> Option<PathBuf> {
    let want = if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    };
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).ok()?.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().is_some_and(|n| n == want) {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_follow_the_machine() {
        let ids = |v: Vec<&Asset>| v.iter().map(|a| a.variant).collect::<Vec<_>>();
        assert_eq!(
            ids(variants_for("macos", "aarch64", false, false)),
            ["macos-arm64"]
        );
        assert!(variants_for("macos", "x86_64", true, false).is_empty());
        assert_eq!(
            ids(variants_for("windows", "x86_64", true, false)),
            ["win-vulkan-x64", "win-cpu-x64"]
        );
        assert_eq!(
            ids(variants_for("windows", "x86_64", false, false)),
            ["win-cpu-x64"]
        );
        assert_eq!(
            ids(variants_for("linux", "aarch64", true, true)),
            [
                "linux-cuda-13.4-arm64",
                "linux-vulkan-arm64",
                "linux-cpu-arm64"
            ]
        );
        assert_eq!(
            ids(variants_for("linux", "aarch64", false, false)),
            ["linux-cpu-arm64"]
        );
        for a in ASSETS {
            assert!(
                a.name.contains(PINNED_TAG) && a.sha256.len() == 64,
                "{}",
                a.name
            );
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("brainpowr-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn unpacks_a_zip_and_finds_the_server_and_keeps_paths_inside() {
        let d = scratch("zip");
        let archive = d.join("b.zip");
        {
            let mut w = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
            let o = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
            let bin = if cfg!(windows) {
                "llama-b1/llama-server.exe"
            } else {
                "llama-b1/llama-server"
            };
            w.start_file(bin, o).unwrap();
            w.write_all(b"#!/bin/sh\n").unwrap();
            w.start_file("../escape.txt", o).unwrap();
            w.write_all(b"x").unwrap();
            w.finish().unwrap();
        }
        let to = d.join("out");
        std::fs::create_dir_all(&to).unwrap();
        unpack(&archive, &to).unwrap();
        let s = find_server(&to).unwrap();
        assert!(s.starts_with(&to));
        assert!(!d.join("escape.txt").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sha256_of_a_known_file() {
        let d = scratch("sha");
        let f = d.join("x");
        std::fs::write(&f, b"abc").unwrap();
        assert_eq!(
            sha256_file(&f).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn installed_reads_the_marker_and_the_binary_wins_over_path() {
        let d = scratch("inst");
        let dir = variant_dir(&d, "linux-cpu-x64");
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("bin/llama-server"), b"").unwrap();
        let i = Installed {
            tag: PINNED_TAG.into(),
            variant: "linux-cpu-x64".into(),
            server: "bin/llama-server".into(),
        };
        std::fs::write(dir.join("installed.json"), serde_json::to_vec(&i).unwrap()).unwrap();
        assert_eq!(installed(&d), vec![i]);
        assert_eq!(
            server_binary(&d, Some("linux-cpu-x64")),
            Some(dir.join("bin/llama-server"))
        );
        uninstall(&d, "linux-cpu-x64").unwrap();
        assert!(installed(&d).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
