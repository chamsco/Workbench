//! The small models on this machine, and which one to hand a small task to.
//!
//! Chats and workers run on a subscription CLI or a router; neither should be
//! asked to grep a file or label a paragraph. Every local runtime on this
//! machine (Ollama, LM Studio, llama.cpp, MLX, any loopback OpenAI-compatible
//! endpoint the user added) is probed in parallel, its models are ranked, and
//! [`recommend`] says what to wire a delegated subagent to right now.
//!
//! [`is_tiny`] is the one heuristic in here: 4B parameters or less when the
//! count is known, a name that plainly states its size, or a file of 3.5 GiB
//! or less. Nothing else is inferred from a name.
//!
//! A probe reads the runtime's own `/models`-style listing; no model is ever
//! asked to generate anything, and no probe waits more than 2.5s.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::harnesses::{parse_version, run, which};
use crate::prefs::Prefs;

/// The defaults each runtime serves on. Loopback only, by construction.
const LM_STUDIO_URL: &str = "http://127.0.0.1:1234/v1";
const LLAMA_CPP_URL: &str = "http://127.0.0.1:8080/v1";
const MLX_URL: &str = "http://127.0.0.1:8081/v1";

/// A model file of 3.5 GiB or less is a small one whatever its name says.
const TINY_BYTES: u64 = 3_758_096_384;

/// One probe's whole budget. Nothing here is worth a hang.
const PROBE: Duration = Duration::from_millis(2500);

// ---------------------------------------------------------------- shapes

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum LocalKind {
    /// Models on this machine through Ollama.
    #[default]
    Ollama,
    /// Apple's `mlx_lm.server`.
    Mlx,
    /// LM Studio's local server.
    LmStudio,
    /// `llama-server` from llama.cpp.
    LlamaCpp,
    /// Any OpenAI-compatible endpoint the user pointed at this machine.
    Custom,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct LocalModel {
    pub id: String,
    /// The runtime's id, so a pick can be traced back to who serves it.
    pub runtime: String,
    /// Billions of parameters, when the runtime or the name says.
    pub params_b: Option<f32>,
    /// "Q4_K_M", "fp16"...
    pub quant: Option<String>,
    pub size_bytes: Option<u64>,
    pub context: Option<u64>,
    pub family: Option<String>,
    /// Meant for small delegated work ([`is_tiny`]).
    pub tiny: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct LocalRuntime {
    pub id: String,
    pub name: String,
    pub kind: LocalKind,
    /// Where it answers; for Ollama this is the root, not `/v1`.
    pub base_url: String,
    pub installed: bool,
    pub running: bool,
    pub models: Vec<LocalModel>,
    pub version: Option<String>,
    /// What to tell the user when something is off.
    pub message: Option<String>,
    pub install: &'static str,
}

/// What kind of small task is being delegated.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Need {
    /// Read a file, a diff or a log back to the big model.
    FastRead,
    /// Yes/no, routing, labels.
    Classify,
    /// Turn a passage into a paragraph.
    Summarize,
    /// Pull structured fields out of text.
    Extract,
    #[default]
    Any,
}

// ---------------------------------------------------------------- tiny

/// The families that are small by construction, checked only when neither a
/// parameter count nor a size is known. A bare `qwen3` or `granite` with no
/// tag says nothing, so it is not on this list.
const TINY_FAMILIES: &[&str] = &[
    "lfm2.5",
    "lfm2",
    "smollm",
    "minicpm",
    "phi-4-mini",
    "phi4-mini",
];

/// A model meant for small delegated work: 4B parameters or less when the
/// count is known (from the runtime or read out of the name), else a name
/// that plainly states a small size (`lfm2.5`, `smollm`, `minicpm`...), else
/// a file of 3.5 GiB or less. An embedding model is never a delegation
/// target, however small it is. This is the only thing read into a name.
pub fn is_tiny(id: &str, params_b: Option<f32>, size_bytes: Option<u64>) -> bool {
    let name = id.to_ascii_lowercase();
    if name.contains("embed") {
        return false;
    }
    if let Some(p) = params_b.filter(|p| p.is_finite() && *p > 0.0) {
        return p <= 4.0;
    }
    if let Some(p) = params_from_name(id) {
        return p <= 4.0;
    }
    if let Some(s) = size_bytes.filter(|s| *s > 0) {
        return s <= TINY_BYTES;
    }
    TINY_FAMILIES.iter().any(|f| name.contains(f))
}

/// Billions of parameters read out of a model id: `qwen3:1.7b` is 1.7,
/// `llama3.2:3b` is 3.0 (not 3.2 — the `.2` belongs to the family name),
/// `lfm2.5-2.6b` is 2.6, `deepseek-r1-distill-qwen-1.5b` is 1.5.
fn params_from_name(id: &str) -> Option<f32> {
    let s = id.to_ascii_lowercase();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i + 1 < b.len() && b[i] == b'.' && b[i + 1].is_ascii_digit() {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
        }
        // A size suffix only counts when nothing word-like follows it.
        if i < b.len() && (b[i] == b'b' || b[i] == b'm') {
            let after = b.get(i + 1).copied();
            if after.is_none_or(|c| !c.is_ascii_alphanumeric()) {
                let n: f32 = s[start..i].parse().ok()?;
                return Some(if b[i] == b'm' { n / 1000.0 } else { n });
            }
        }
    }
    None
}

/// Ollama's own `details.parameter_size`: "3.2B", "70B", "135M".
fn parse_params(size: &str) -> Option<f32> {
    let s = size.trim().to_ascii_uppercase();
    let mul = if s.ends_with('B') {
        1.0
    } else if s.ends_with('M') {
        0.001
    } else {
        1.0
    };
    s.trim_end_matches(['B', 'M'])
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|n| n.is_finite() && *n > 0.0)
        .map(|n| n * mul)
}

/// The family a name names, for display only; None when it names none.
fn family_from_name(id: &str) -> Option<String> {
    let n = id.to_ascii_lowercase();
    FAMILIES
        .iter()
        .find(|(k, _)| n.contains(k))
        .map(|(_, v)| (*v).to_string())
}

const FAMILIES: &[(&str, &str)] = &[
    ("lfm2.5", "LFM2.5"),
    ("lfm2", "LFM2"),
    ("r1-distill", "DeepSeek-R1-Distill"),
    ("deepseek-r1", "DeepSeek-R1"),
    ("qwen3", "Qwen3"),
    ("qwen2.5", "Qwen2.5"),
    ("minicpm", "MiniCPM"),
    ("granite", "Granite"),
    ("smollm", "SmolLM"),
    ("gemma3", "Gemma3"),
    ("gemma", "Gemma"),
    ("llama3.2", "Llama3.2"),
    ("llama3", "Llama3"),
    ("llama2", "Llama2"),
    ("phi-4", "Phi-4"),
    ("phi4", "Phi-4"),
    ("mistral", "Mistral"),
    ("nomic", "Nomic"),
    ("mxbai", "mxbai"),
];

/// The quantization a file name states, longest token first.
fn quant_from_name(id: &str) -> Option<String> {
    let n = id.to_ascii_lowercase();
    QUANTS
        .iter()
        .find(|q| n.contains(*q))
        .map(|q| q.to_ascii_uppercase())
}

const QUANTS: &[&str] = &[
    "q8_0", "q6_k", "q5_k_m", "q5_k_s", "q4_k_m", "q4_k_s", "q4_0", "q3_k_m", "iq4_xs", "iq3_xxs",
    "fp16", "bf16", "f16", "int8", "int4",
];

// ---------------------------------------------------------------- ranking

/// Delegation order for a small task: `tiny` models first, then the smaller
/// ones, then by id so two runs of the same machine give the same answer.
pub fn order(models: &[LocalModel]) -> Vec<&LocalModel> {
    let mut v: Vec<&LocalModel> = models.iter().collect();
    v.sort_by(|a, b| cmp_models(a, b));
    v
}

/// The ranking key: tiny, then fewer parameters, then a smaller file, then
/// the id. Unknown parameters or size sort last, never first.
fn cmp_models(a: &LocalModel, b: &LocalModel) -> std::cmp::Ordering {
    b.tiny
        .cmp(&a.tiny)
        .then_with(|| {
            params_of(a)
                .partial_cmp(&params_of(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| size_of(a).cmp(&size_of(b)))
        .then_with(|| a.id.cmp(&b.id))
}

fn params_of(m: &LocalModel) -> f32 {
    m.params_b
        .filter(|p| p.is_finite() && *p > 0.0)
        .unwrap_or(f32::MAX)
}

fn size_of(m: &LocalModel) -> u64 {
    m.size_bytes.filter(|s| *s > 0).unwrap_or(u64::MAX)
}

/// The task-kind mapping, in one place. The named families are tried first,
/// in order; the window breaks the tie, and models outside it are used only
/// when nothing tiny is installed.
///
/// | Need      | Window                    | Names first, in order                 |
/// |-----------|---------------------------|---------------------------------------|
/// | FastRead  | smallest                  | LFM2.5, SmolLM2, Qwen3, Gemma3        |
/// | Classify  | smallest                  | LFM2.5, Granite, Qwen3, SmolLM2       |
/// | Summarize | largest tiny (up to 4B)   | Qwen3, Gemma3, Llama3.2, Granite      |
/// | Extract   | largest tiny (up to 4B)   | Qwen3, Phi-4-mini, R1-Distill, MiniCPM |
/// | Any       | the plain [`order`]       | —                                     |
///
/// Nothing here guesses a capability from a name beyond which family it is.
pub fn pick(models: &[LocalModel], need: Need) -> Option<&LocalModel> {
    let ordered = order(models);
    if need == Need::Any {
        return ordered.first().copied();
    }
    let (names, largest) = need_preference(need);
    let mut window: Vec<&LocalModel> = ordered.iter().copied().filter(|m| m.tiny).collect();
    if window.is_empty() {
        window = ordered;
    }
    if largest {
        window.reverse();
    }
    for name in names {
        if let Some(m) = window
            .iter()
            .copied()
            .find(|m| m.id.to_ascii_lowercase().contains(name))
        {
            return Some(m);
        }
    }
    window.first().copied()
}

fn need_preference(need: Need) -> (&'static [&'static str], bool) {
    match need {
        Need::FastRead => (&["lfm2.5", "smollm", "qwen3", "gemma3"], false),
        Need::Classify => (&["lfm2.5", "granite", "qwen3", "smollm"], false),
        Need::Summarize => (&["qwen3", "gemma3", "llama3.2", "granite"], true),
        Need::Extract => (&["qwen3", "phi-4-mini", "r1-distill", "minicpm"], true),
        Need::Any => (&[], false),
    }
}

/// What to wire a subagent to right now: only running runtimes count, each
/// contributes its best model, and the smallest/tinniest of those wins. A
/// tie keeps the order [`detect`] returned (Ollama, LM Studio, llama.cpp,
/// MLX, then the user's endpoints), so the answer is stable. None when
/// nothing local is running.
pub fn recommend(runtimes: &[LocalRuntime]) -> Option<(String, String)> {
    let mut best: Option<(&LocalRuntime, &LocalModel)> = None;
    for rt in runtimes.iter().filter(|r| r.running) {
        let Some(m) = pick(&rt.models, Need::Any) else {
            continue;
        };
        if best.is_none_or(|(_, b)| cmp_models(m, b).is_lt()) {
            best = Some((rt, m));
        }
    }
    best.map(|(rt, m)| (rt.base_url.clone(), m.id.clone()))
}

/// A base_url the Lead can add as an OpenAI-compatible provider: the runtime
/// [`recommend`] picked, in its OpenAI form ([`openai_base`]). Probes the
/// machine the way [`detect`] does, so None means nothing local answered.
pub fn best_endpoint(prefs: &Prefs) -> Option<String> {
    let runtimes = detect(prefs, &[]);
    let (url, _) = recommend(&runtimes)?;
    let kind = runtimes
        .iter()
        .find(|r| r.base_url == url)
        .map(|r| r.kind)?;
    Some(openai_base(kind, &url))
}

/// The root to call a runtime's OpenAI-compatible API on: Ollama serves it
/// under `/v1`; the others already are one.
pub fn openai_base(kind: LocalKind, base_url: &str) -> String {
    let b = base_url.trim_end_matches('/');
    if kind == LocalKind::Ollama {
        format!("{b}/v1")
    } else {
        b.to_string()
    }
}

/// `localhost`, `127.x.x.x`, `[::1]` or `0.0.0.0` — an address that is this
/// machine. Anything else is someone else's.
pub fn is_loopback(url: &str) -> bool {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    // Drop any user:password@ in front of the host.
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let host = if let Some(h) = authority.strip_prefix('[') {
        h.split(']').next().unwrap_or(h)
    } else {
        authority.split(':').next().unwrap_or(authority)
    };
    let host = host.to_ascii_lowercase();
    host == "localhost" || host == "0.0.0.0" || host == "::1" || host.starts_with("127.")
}

// ---------------------------------------------------------------- parsing

/// Ollama's `/api/tags` (and `/api/ps`, which uses the same entry shape).
/// When an entry does not carry `details`, the count is read out of the name.
pub fn parse_ollama_tags(v: &Value) -> Vec<LocalModel> {
    v["models"]
        .as_array()
        .map(|a| a.iter().filter_map(ollama_entry).collect())
        .unwrap_or_default()
}

fn ollama_entry(m: &Value) -> Option<LocalModel> {
    let id = m["name"]
        .as_str()
        .or_else(|| m["model"].as_str())?
        .to_string();
    let details = &m["details"];
    let params_b = details["parameter_size"]
        .as_str()
        .and_then(parse_params)
        .or_else(|| params_from_name(&id));
    let size_bytes = m["size"].as_u64().filter(|s| *s > 0);
    Some(LocalModel {
        tiny: is_tiny(&id, params_b, size_bytes),
        quant: details["quantization_level"]
            .as_str()
            .map(str::to_string)
            .or_else(|| quant_from_name(&id)),
        family: details["family"]
            .as_str()
            .map(str::to_string)
            .or_else(|| family_from_name(&id)),
        // A few builds list it; when they do not, it stays unknown.
        context: m["context_length"].as_u64(),
        params_b,
        size_bytes,
        runtime: "ollama".into(),
        id,
    })
}

/// Any OpenAI-compatible `/v1/models`: LM Studio, llama.cpp, `mlx_lm.server`,
/// a router. Most list nothing but an id, so the parameter count is read out
/// of the name when the server does not state one.
pub fn parse_openai_models(v: &Value, runtime: &str) -> Vec<LocalModel> {
    v["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| {
                    let id = m["id"].as_str()?.to_string();
                    let params_b = m["parameters_b"]
                        .as_f64()
                        .map(|p| p as f32)
                        .or_else(|| m["parameter_size"].as_str().and_then(parse_params))
                        .or_else(|| params_from_name(&id));
                    let size_bytes = m["size"].as_u64().filter(|s| *s > 0);
                    let context = m["context_length"]
                        .as_u64()
                        .or_else(|| m["max_context_length"].as_u64())
                        .or_else(|| m["n_ctx"].as_u64());
                    Some(LocalModel {
                        tiny: is_tiny(&id, params_b, size_bytes),
                        quant: m["quantization"]
                            .as_str()
                            .map(str::to_string)
                            .or_else(|| m["quantization_level"].as_str().map(str::to_string))
                            .or_else(|| quant_from_name(&id)),
                        family: m["family"]
                            .as_str()
                            .map(str::to_string)
                            .or_else(|| family_from_name(&id)),
                        context,
                        params_b,
                        size_bytes,
                        runtime: runtime.to_string(),
                        id,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// "2.0 GB" — decimal units, the way model cards quote sizes.
pub fn human_size(bytes: u64) -> String {
    const GB: f64 = 1e9;
    const MB: f64 = 1e6;
    const KB: f64 = 1e3;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

// ---------------------------------------------------------------- probes

/// Every local runtime this machine has, probed in parallel. A stopped or
/// absent one is still listed (with `install`) so Settings can offer it.
/// `extra_urls` are OpenAI-compatible endpoints the caller knows about (ones
/// the user typed in); each is probed like a router.
pub fn detect(prefs: &Prefs, extra_urls: &[String]) -> Vec<LocalRuntime> {
    let ollama = prefs.ollama_url.clone();
    // (runtime id, name, base_url, api key): loopback routers first, then the
    // endpoints the caller passed.
    let custom: Vec<(String, String, String, String)> = prefs
        .routers
        .iter()
        .filter(|r| is_loopback(&r.base_url))
        .map(|r| {
            (
                format!("router:{}", r.id),
                r.name.clone(),
                r.base_url.clone(),
                r.api_key.clone(),
            )
        })
        .chain(extra_urls.iter().enumerate().map(|(i, url)| {
            (
                format!("local:{i}"),
                format!("Local endpoint {}", i + 1),
                url.clone(),
                String::new(),
            )
        }))
        .collect();
    let mut out = std::thread::scope(|s| {
        let ol = s.spawn(move || probe_ollama(&ollama));
        let lm = s.spawn(probe_lm_studio);
        let lc = s.spawn(probe_llama_cpp);
        let mx = s.spawn(probe_mlx);
        let cs: Vec<_> = custom
            .into_iter()
            .map(|(id, name, url, key)| {
                s.spawn(move || {
                    let mut rt = endpoint(&id, &name, LocalKind::Custom, &url, "");
                    fill_openai(&mut rt, Some(key.as_str()));
                    rt
                })
            })
            .collect();
        let mut v: Vec<LocalRuntime> = [
            ol.join().ok(),
            lm.join().ok(),
            lc.join().ok(),
            mx.join().ok(),
        ]
        .into_iter()
        .flatten()
        .collect();
        // One panicking probe must not hide the others.
        v.extend(cs.into_iter().filter_map(|h| h.join().ok()));
        v
    });
    // One runtime per address: a router pointed at a known runtime's own
    // endpoint is that runtime, not a second one.
    let mut seen: Vec<String> = Vec::new();
    out.retain(|r| {
        let u = r.base_url.trim_end_matches('/').to_ascii_lowercase();
        if u.is_empty() || seen.iter().any(|s| *s == u) {
            return false;
        }
        seen.push(u);
        true
    });
    out
}

/// A runtime as configured, before anything is probed.
fn endpoint(
    id: &str,
    name: &str,
    kind: LocalKind,
    base_url: &str,
    install: &'static str,
) -> LocalRuntime {
    LocalRuntime {
        id: id.into(),
        name: name.into(),
        kind,
        base_url: base_url.trim_end_matches('/').to_string(),
        installed: true,
        running: false,
        models: vec![],
        version: None,
        message: None,
        install,
    }
}

/// Probe an OpenAI-compatible endpoint and fill in what it says.
fn fill_openai(rt: &mut LocalRuntime, key: Option<&str>) {
    match get_json(&format!("{}/models", rt.base_url), key) {
        Ok(v) => {
            rt.running = true;
            rt.models = parse_openai_models(&v, &rt.id);
            if rt.models.is_empty() {
                rt.message = Some(format!(
                    "Running at {}, but it lists no models.",
                    rt.base_url
                ));
            }
        }
        Err(e) => rt.message = Some(not_running(&rt.base_url, &e)),
    }
}

fn not_running(base_url: &str, why: &str) -> String {
    if why == "not running" {
        format!("Nothing answers at {base_url}.")
    } else {
        format!("Nothing answers at {base_url}: {why}.")
    }
}

fn probe_ollama(url: &str) -> LocalRuntime {
    let base = url.trim_end_matches('/').to_string();
    let bin = which("ollama");
    let mut rt = endpoint(
        "ollama",
        "Ollama",
        LocalKind::Ollama,
        &base,
        "https://ollama.com/download",
    );
    rt.installed = bin.is_some();
    rt.version = bin.as_ref().and_then(|b| version_of(b));
    let tags = get_json(&format!("{base}/api/tags"), None);
    let ps = get_json(&format!("{base}/api/ps"), None);
    if let Ok(v) = &tags {
        rt.running = true;
        rt.models = parse_ollama_tags(v);
    }
    // `/api/ps` answers even when `/api/tags` does not, and one loaded model
    // is still a model we can delegate to.
    if let Ok(v) = &ps {
        rt.running = true;
        if rt.models.is_empty() {
            rt.models = parse_ollama_tags(v);
        }
    }
    rt.installed |= rt.running;
    if !rt.running {
        let why = tags.err().unwrap_or_else(|| "not running".into());
        rt.message = Some(if bin.is_some() {
            format!("Installed, but the server at {base} is {why}. Start it with `ollama serve`.")
        } else {
            format!("{} Install: {}", not_running(&base, &why), rt.install)
        });
    } else if rt.models.is_empty() {
        rt.message = Some(format!(
            "Running, but no models are pulled. Pull one, e.g. `ollama pull lfm2.5:2.6b`."
        ));
    }
    rt
}

fn probe_lm_studio() -> LocalRuntime {
    let mut rt = endpoint(
        "lmstudio",
        "LM Studio",
        LocalKind::LmStudio,
        LM_STUDIO_URL,
        "https://lmstudio.ai",
    );
    rt.installed = which("lms").is_some();
    fill_openai(&mut rt, None);
    rt.installed |= rt.running;
    add_install_hint(&mut rt);
    rt
}

fn probe_llama_cpp() -> LocalRuntime {
    let bin = which("llama-server");
    let mut rt = endpoint(
        "llamacpp",
        "llama.cpp",
        LocalKind::LlamaCpp,
        LLAMA_CPP_URL,
        "https://github.com/ggml-org/llama.cpp/releases",
    );
    rt.installed = bin.is_some();
    rt.version = bin.as_ref().and_then(|b| version_of(b));
    fill_openai(&mut rt, None);
    rt.installed |= rt.running;
    add_install_hint(&mut rt);
    rt
}

fn probe_mlx() -> LocalRuntime {
    let bin = which("mlx_lm.server").or_else(|| which("mlx_lm"));
    let mut rt = endpoint("mlx", "MLX", LocalKind::Mlx, MLX_URL, "pip install mlx-lm");
    rt.installed = bin.is_some();
    rt.version = bin.as_ref().and_then(|b| version_of(b));
    fill_openai(&mut rt, None);
    rt.installed |= rt.running;
    add_install_hint(&mut rt);
    rt
}

/// Nothing answers and the binary is missing too: say how to get it.
fn add_install_hint(rt: &mut LocalRuntime) {
    if !rt.installed && !rt.install.is_empty() {
        let m = rt.message.take().unwrap_or_default();
        rt.message = Some(format!("{m} Install: {}", rt.install).trim_start().to_string());
    }
}

/// A CLI's version, when it answers quickly. None is a fine answer.
fn version_of(bin: &std::path::Path) -> Option<String> {
    run(bin, &["--version"], Duration::from_secs(2))
        .and_then(|o| parse_version(&format!("{}\n{}", o.stdout, o.stderr)))
}

/// One GET, decoded. "not running" is the common answer and the one the UI
/// prints, so it gets its own words.
fn get_json(url: &str, key: Option<&str>) -> Result<Value, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async {
        // no_proxy: a proxy configured for the office must not swallow a
        // request to this machine.
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(PROBE)
            .build()
            .map_err(|e| e.to_string())?;
        let mut rb = client.get(url);
        if let Some(k) = key.filter(|k| !k.is_empty()) {
            rb = rb.bearer_auth(k);
        }
        let r = rb.send().await.map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                "not running".to_string()
            } else {
                e.to_string()
            }
        })?;
        let status = r.status();
        if !status.is_success() {
            return Err(format!("HTTP {status}"));
        }
        r.json::<Value>().await.map_err(|e| e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A model as `detect` would build it: `tiny` always comes from `is_tiny`.
    fn model(id: &str, params_b: Option<f32>, size_bytes: Option<u64>) -> LocalModel {
        LocalModel {
            id: id.into(),
            runtime: "ollama".into(),
            params_b,
            size_bytes,
            tiny: is_tiny(id, params_b, size_bytes),
            ..Default::default()
        }
    }

    fn runtime(id: &str, url: &str, running: bool, models: Vec<LocalModel>) -> LocalRuntime {
        LocalRuntime {
            id: id.into(),
            name: id.into(),
            kind: LocalKind::Ollama,
            base_url: url.into(),
            running,
            models,
            ..Default::default()
        }
    }

    #[test]
    fn tiny_by_parameter_count() {
        assert!(is_tiny("lfm2.5-2.6b", Some(2.6), None));
        assert!(is_tiny("qwen3:1.7b", Some(1.7), None));
        assert!(is_tiny("deepseek-r1-distill-qwen-1.5b", Some(1.5), None));
        assert!(is_tiny("llama3.2:3b", Some(3.2), None));
        assert!(is_tiny("gemma3:4b", Some(4.0), None));
        assert!(!is_tiny("qwen3:32b", Some(32.0), None));
        assert!(!is_tiny("llama3.1:70b", Some(70.0), None));
    }

    #[test]
    fn tiny_from_the_name_when_the_count_is_missing() {
        assert!(is_tiny("lfm2.5-2.6b", None, None));
        assert!(is_tiny("deepseek-r1-distill-qwen-1.5b", None, None));
        assert!(is_tiny("qwen3:1.7b", None, None));
        assert!(is_tiny("llama3.2:3b", None, None));
        assert!(is_tiny("minicpm-v", None, None));
        assert!(is_tiny("smollm2", None, None));
        assert!(!is_tiny("qwen3:32b", None, None));
        assert!(!is_tiny("llama3.1:70b", None, None));
        // No size stated anywhere: not evidence of a small model.
        assert!(!is_tiny("qwen3", None, None));
        assert!(!is_tiny("some-model", None, None));
    }

    #[test]
    fn tiny_from_the_file_size_only_when_nothing_else_says() {
        assert!(is_tiny("some-model", None, Some(2_000_000_000)));
        assert!(!is_tiny("some-model", None, Some(20_000_000_000)));
        // A stated parameter count outranks the file size.
        assert!(!is_tiny("qwen3:8b", Some(8.0), Some(2_000_000_000)));
        // And an embedding model is never a delegation target.
        assert!(!is_tiny("nomic-embed-text:latest", None, Some(274_000_000)));
    }

    #[test]
    fn order_puts_the_smallest_tiny_first() {
        let models = vec![
            model("qwen3:32b", Some(32.0), None),
            model("llama3.2:3b", Some(3.2), Some(2_019_393_189)),
            model("lfm2.5-2.6b", Some(2.6), Some(1_600_000_000)),
            model("qwen3:1.7b", Some(1.7), None),
            model("mystery", None, None),
        ];
        let ids: Vec<&str> = order(&models).iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "qwen3:1.7b",
                "lfm2.5-2.6b",
                "llama3.2:3b",
                "qwen3:32b",
                "mystery"
            ]
        );
    }

    #[test]
    fn order_breaks_ties_by_size_then_id() {
        let by_id = vec![model("b:1b", Some(1.0), None), model("a:1b", Some(1.0), None)];
        let ids: Vec<&str> = order(&by_id).iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["a:1b", "b:1b"]);
        let by_size = vec![
            model("z-model", None, Some(2_000_000_000)),
            model("y-model", None, Some(1_000_000_000)),
        ];
        let ids: Vec<&str> = order(&by_size).iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["y-model", "z-model"]);
    }

    #[test]
    fn pick_maps_the_task_kind() {
        // No listed family: the window decides the answer.
        let models = vec![
            model("acme-small:1b", Some(1.0), None),
            model("acme-mid:3b", Some(3.0), None),
            model("qwen3:32b", Some(32.0), None),
        ];
        assert_eq!(pick(&models, Need::FastRead).unwrap().id, "acme-small:1b");
        assert_eq!(pick(&models, Need::Classify).unwrap().id, "acme-small:1b");
        // Summarize may spend up to 4B, so the largest tiny one wins.
        assert_eq!(pick(&models, Need::Summarize).unwrap().id, "acme-mid:3b");
        assert_eq!(pick(&models, Need::Extract).unwrap().id, "acme-mid:3b");
        assert_eq!(pick(&models, Need::Any).unwrap().id, "acme-small:1b");
    }

    #[test]
    fn pick_prefers_the_named_family() {
        let models = vec![
            model("acme-small:1b", Some(1.0), None),
            model("lfm2.5-2.6b", Some(2.6), None),
            model("gemma3:4b", Some(4.0), None),
        ];
        assert_eq!(pick(&models, Need::FastRead).unwrap().id, "lfm2.5-2.6b");
        assert_eq!(pick(&models, Need::Summarize).unwrap().id, "gemma3:4b");
    }

    #[test]
    fn pick_falls_back_and_handles_empty() {
        let models = vec![
            model("qwen3:32b", Some(32.0), None),
            model("llama3.1:70b", Some(70.0), None),
        ];
        assert_eq!(pick(&models, Need::FastRead).unwrap().id, "qwen3:32b");
        assert_eq!(pick(&models, Need::Extract).unwrap().id, "qwen3:32b");
        assert!(pick(&[], Need::Any).is_none());
        assert!(pick(&[], Need::Summarize).is_none());
    }

    #[test]
    fn recommend_needs_a_running_runtime() {
        let stopped = runtime(
            "ollama",
            "http://localhost:11434",
            false,
            vec![model("qwen3:1.7b", Some(1.7), None)],
        );
        assert_eq!(recommend(&[]), None);
        assert_eq!(recommend(&[stopped.clone()]), None);
        let running = runtime(
            "lmstudio",
            "http://127.0.0.1:1234/v1",
            true,
            vec![model("qwen3:32b", Some(32.0), None)],
        );
        assert_eq!(
            recommend(&[stopped, running]),
            Some(("http://127.0.0.1:1234/v1".into(), "qwen3:32b".into()))
        );
    }

    #[test]
    fn recommend_prefers_the_tiny_model_and_stays_stable() {
        let big = runtime(
            "big",
            "http://127.0.0.1:9999/v1",
            true,
            vec![model("llama3.1:70b", Some(70.0), None)],
        );
        let small = runtime(
            "small",
            "http://127.0.0.1:9998/v1",
            true,
            vec![model("lfm2.5-2.6b", Some(2.6), None)],
        );
        let want = Some(("http://127.0.0.1:9998/v1".into(), "lfm2.5-2.6b".into()));
        assert_eq!(recommend(&[big.clone(), small.clone()]), want);
        assert_eq!(recommend(&[small, big]), want);
        let empty = runtime("lmstudio", "http://127.0.0.1:1234/v1", true, vec![]);
        assert_eq!(recommend(&[empty]), None);
    }

    #[test]
    fn is_loopback_only_for_this_machine() {
        assert!(is_loopback("http://localhost:11434"));
        assert!(is_loopback("http://127.0.0.1:1234/v1"));
        assert!(is_loopback("http://127.0.0.1"));
        assert!(is_loopback("http://127.0.0.2:8080"));
        assert!(is_loopback("http://[::1]:8081/v1"));
        assert!(is_loopback("http://0.0.0.0:8080/v1/models"));
        assert!(!is_loopback("https://api.example.com/v1"));
        assert!(!is_loopback("http://192.168.1.5:1234/v1"));
        assert!(!is_loopback(""));
    }

    #[test]
    fn parse_ollama_tags_fixture() {
        let v = json!({"models": [
            {"name": "lfm2.5:2.6b", "size": 1_679_000_000,
             "details": {"family": "lfm2", "parameter_size": "2.6B", "quantization_level": "Q4_K_M"}},
            {"name": "llama3.2:latest", "size": 2_019_393_189,
             "details": {"family": "llama", "parameter_size": "3.2B", "quantization_level": "Q4_K_M"}},
            {"name": "qwen3:32b", "details": {"parameter_size": "32.5B"}},
            {"model": "granite3.2:2b"}
        ]});
        let ms = parse_ollama_tags(&v);
        assert_eq!(ms.len(), 4);
        assert_eq!(ms[0].id, "lfm2.5:2.6b");
        assert_eq!(ms[0].runtime, "ollama");
        assert_eq!(ms[0].params_b, Some(2.6));
        assert_eq!(ms[0].quant.as_deref(), Some("Q4_K_M"));
        assert_eq!(ms[0].family.as_deref(), Some("lfm2"));
        assert_eq!(ms[0].size_bytes, Some(1_679_000_000));
        assert!(ms[0].tiny);
        // A `:latest` suffix changes nothing.
        assert!(ms[1].tiny);
        assert_eq!(ms[1].params_b, Some(3.2));
        assert_eq!(ms[2].params_b, Some(32.5));
        assert!(!ms[2].tiny);
        // A missing size is None, never 0.
        assert_eq!(ms[2].size_bytes, None);
        assert_eq!(ms[2].family.as_deref(), Some("Qwen3"));
        // `model` works when `name` is absent; the count comes from the name.
        assert_eq!(ms[3].id, "granite3.2:2b");
        assert_eq!(ms[3].params_b, Some(2.0));
        assert!(ms[3].tiny);
        assert!(parse_ollama_tags(&json!({})).is_empty());
    }

    #[test]
    fn parse_openai_models_fixture() {
        let v = json!({"data": [
            {"id": "qwen3:1.7b", "object": "model"},
            {"id": "Qwen/Qwen3-32B-GGUF", "object": "model"},
            {"id": "acme-tiny", "object": "model", "size": 1_000_000_000}
        ]});
        let ms = parse_openai_models(&v, "lmstudio");
        assert_eq!(ms.len(), 3);
        assert_eq!(ms[0].runtime, "lmstudio");
        assert_eq!(ms[0].params_b, Some(1.7));
        assert!(ms[0].tiny);
        assert_eq!(ms[1].params_b, Some(32.0));
        assert!(!ms[1].tiny);
        // No count in the name: the file size decides, and stays None.
        assert_eq!(ms[2].params_b, None);
        assert_eq!(ms[2].size_bytes, Some(1_000_000_000));
        assert!(ms[2].tiny);
        assert!(parse_openai_models(&json!({}), "x").is_empty());
        assert!(parse_openai_models(&json!({"data": [{"object": "model"}]}), "x").is_empty());
    }

    #[test]
    fn human_size_reads_in_decimal_units() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(900), "900 B");
        assert_eq!(human_size(120_000), "120 KB");
        assert_eq!(human_size(512_000_000), "512 MB");
        assert_eq!(human_size(2_019_393_189), "2.0 GB");
    }

    #[test]
    fn openai_base_adds_v1_for_ollama() {
        assert_eq!(
            openai_base(LocalKind::Ollama, "http://localhost:11434/"),
            "http://localhost:11434/v1"
        );
        assert_eq!(
            openai_base(LocalKind::LmStudio, "http://127.0.0.1:1234/v1"),
            "http://127.0.0.1:1234/v1"
        );
        assert_eq!(
            openai_base(LocalKind::Custom, "http://127.0.0.1:9000/v1"),
            "http://127.0.0.1:9000/v1"
        );
    }
}
