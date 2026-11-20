//! What a GGUF file says about itself: architecture, name, size label,
//! trained context. Only the metadata block is read, never the tensors, and
//! no string over 1 MiB is believed.

use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{bail, Result};
use serde::Serialize;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub architecture: Option<String>,
    pub name: Option<String>,
    /// "1.7B", "8x7B".
    pub size_label: Option<String>,
    /// The context the model was trained on.
    pub context_length: Option<u64>,
    pub block_count: Option<u64>,
    /// It is an embedding model (a pooling type is declared).
    pub embedding: bool,
}

const MAX_STR: u64 = 1 << 20;
const MAX_KV: u64 = 100_000;

pub fn read(path: &Path) -> Result<Meta> {
    let mut r = BufReader::new(std::fs::File::open(path)?);
    parse(&mut r)
}

pub fn parse(r: &mut impl Read) -> Result<Meta> {
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != b"GGUF" {
        bail!("not a GGUF file");
    }
    let version = u32le(r)?;
    if !(2..=3).contains(&version) {
        bail!("GGUF version {version} is not supported");
    }
    let _tensors = u64le(r)?;
    let kvs = u64le(r)?;
    if kvs > MAX_KV {
        bail!("implausible metadata count {kvs}");
    }
    let mut m = Meta::default();
    let mut ctx_by_arch: Vec<(String, u64)> = Vec::new();
    let mut blocks_by_arch: Vec<(String, u64)> = Vec::new();
    for _ in 0..kvs {
        let key = string(r)?;
        let ty = u32le(r)?;
        let v = value(r, ty)?;
        match key.as_str() {
            "general.architecture" => m.architecture = v.as_str(),
            "general.name" => m.name = v.as_str(),
            "general.size_label" => m.size_label = v.as_str(),
            k if k.ends_with(".context_length") => {
                if let Some(n) = v.as_u64() {
                    ctx_by_arch.push((k.trim_end_matches(".context_length").into(), n));
                }
            }
            k if k.ends_with(".block_count") => {
                if let Some(n) = v.as_u64() {
                    blocks_by_arch.push((k.trim_end_matches(".block_count").into(), n));
                }
            }
            k if k.ends_with(".pooling_type") => m.embedding = true,
            _ => {}
        }
    }
    let arch = m.architecture.clone().unwrap_or_default();
    let pick = |v: &[(String, u64)]| {
        v.iter()
            .find(|(a, _)| *a == arch)
            .or(v.first())
            .map(|(_, n)| *n)
    };
    m.context_length = pick(&ctx_by_arch);
    m.block_count = pick(&blocks_by_arch);
    Ok(m)
}

enum V {
    U(u64),
    S(String),
    Other,
}

impl V {
    fn as_str(&self) -> Option<String> {
        match self {
            V::S(s) => Some(s.clone()),
            _ => None,
        }
    }
    fn as_u64(&self) -> Option<u64> {
        match self {
            V::U(n) => Some(*n),
            _ => None,
        }
    }
}

fn value(r: &mut impl Read, ty: u32) -> Result<V> {
    Ok(match ty {
        0 | 1 | 7 => V::U(bytes::<1>(r)?[0] as u64),
        2 | 3 => V::U(u16::from_le_bytes(bytes::<2>(r)?) as u64),
        4 => V::U(u32le(r)? as u64),
        5 => V::U(i32::from_le_bytes(bytes::<4>(r)?).max(0) as u64),
        6 => {
            bytes::<4>(r)?;
            V::Other
        }
        8 => V::S(string(r)?),
        9 => {
            let ity = u32le(r)?;
            let n = u64le(r)?;
            if n > 50_000_000 {
                bail!("implausible array length {n}");
            }
            if let Some(w) = width(ity) {
                skip(r, n * w)?;
            } else {
                for _ in 0..n {
                    value(r, ity)?;
                }
            }
            V::Other
        }
        10 => V::U(u64le(r)?),
        11 => V::U(i64::from_le_bytes(bytes::<8>(r)?).max(0) as u64),
        12 => {
            bytes::<8>(r)?;
            V::Other
        }
        t => bail!("unknown GGUF value type {t}"),
    })
}

fn width(ty: u32) -> Option<u64> {
    match ty {
        0 | 1 | 7 => Some(1),
        2 | 3 => Some(2),
        4..=6 => Some(4),
        10..=12 => Some(8),
        _ => None,
    }
}

fn skip(r: &mut impl Read, n: u64) -> Result<()> {
    let got = std::io::copy(&mut r.take(n), &mut std::io::sink())?;
    if got != n {
        bail!("the file ends inside its metadata");
    }
    Ok(())
}

fn bytes<const N: usize>(r: &mut impl Read) -> Result<[u8; N]> {
    let mut b = [0u8; N];
    r.read_exact(&mut b)?;
    Ok(b)
}
fn u32le(r: &mut impl Read) -> Result<u32> {
    Ok(u32::from_le_bytes(bytes::<4>(r)?))
}
fn u64le(r: &mut impl Read) -> Result<u64> {
    Ok(u64::from_le_bytes(bytes::<8>(r)?))
}
fn string(r: &mut impl Read) -> Result<String> {
    let n = u64le(r)?;
    if n > MAX_STR {
        bail!("implausible string length {n}");
    }
    let mut b = vec![0u8; n as usize];
    r.read_exact(&mut b)?;
    Ok(String::from_utf8_lossy(&b).into_owned())
}

/// A GGUF header in bytes, for tests here and in the session supervisor's.
#[cfg(test)]
pub(crate) fn fixture(kv: &[(&str, FixV)]) -> Vec<u8> {
    let mut b = b"GGUF".to_vec();
    b.extend(3u32.to_le_bytes());
    b.extend(0u64.to_le_bytes());
    b.extend((kv.len() as u64).to_le_bytes());
    let s = |b: &mut Vec<u8>, s: &str| {
        b.extend((s.len() as u64).to_le_bytes());
        b.extend(s.as_bytes());
    };
    for (k, v) in kv {
        s(&mut b, k);
        match v {
            FixV::S(x) => {
                b.extend(8u32.to_le_bytes());
                s(&mut b, x);
            }
            FixV::U32(n) => {
                b.extend(4u32.to_le_bytes());
                b.extend(n.to_le_bytes());
            }
            FixV::Strs(xs) => {
                b.extend(9u32.to_le_bytes());
                b.extend(8u32.to_le_bytes());
                b.extend((xs.len() as u64).to_le_bytes());
                for x in xs.iter() {
                    s(&mut b, x);
                }
            }
            FixV::F32s(n) => {
                b.extend(9u32.to_le_bytes());
                b.extend(6u32.to_le_bytes());
                b.extend((*n as u64).to_le_bytes());
                b.extend(std::iter::repeat_n(0u8, n * 4));
            }
        }
    }
    b
}

#[cfg(test)]
pub(crate) enum FixV {
    S(&'static str),
    U32(u32),
    Strs(&'static [&'static str]),
    F32s(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_fields_and_skips_arrays() {
        let b = fixture(&[
            ("general.architecture", FixV::S("qwen3")),
            ("tokenizer.ggml.tokens", FixV::Strs(&["a", "b", "c"])),
            ("tokenizer.ggml.scores", FixV::F32s(3)),
            ("general.name", FixV::S("Qwen3 1.7B")),
            ("general.size_label", FixV::S("1.7B")),
            ("llama.context_length", FixV::U32(2048)),
            ("qwen3.context_length", FixV::U32(40960)),
            ("qwen3.block_count", FixV::U32(28)),
        ]);
        let m = parse(&mut b.as_slice()).unwrap();
        assert_eq!(m.architecture.as_deref(), Some("qwen3"));
        assert_eq!(m.name.as_deref(), Some("Qwen3 1.7B"));
        assert_eq!(m.size_label.as_deref(), Some("1.7B"));
        // The architecture's own key wins over another's.
        assert_eq!(m.context_length, Some(40960));
        assert_eq!(m.block_count, Some(28));
        assert!(!m.embedding);
    }

    #[test]
    fn refuses_what_is_not_gguf_or_is_cut_short() {
        assert!(parse(&mut &b"GGML0000"[..]).is_err());
        let b = fixture(&[("general.name", FixV::S("x"))]);
        assert!(parse(&mut &b[..b.len() - 1]).is_err());
    }
}
