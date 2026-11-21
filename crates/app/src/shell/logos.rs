//! Real marks for the coding CLIs and the model makers (Charles, 21 Nov: "pull up the actual LLM
//! logos ... I don't want initials"). The SVGs come from Lobe Icons (MIT, see
//! THIRD_PARTY_NOTICES.md), copied unchanged into `assets/logos/`. A mark drawn in
//! `currentColor` takes the colour it is asked for; the colour marks keep their own.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use gpui_kit::{Image, ImageFormat};

macro_rules! marks {
    ($($n:literal),* $(,)?) => { &[$(($n, include_str!(concat!("../../assets/logos/", $n, ".svg")))),*] };
}

const MARKS: &[(&str, &str)] = marks![
    "claudecode-color", "codex-color", "cursor", "opencode", "geminicli-color", "grok", "claude-color", "openai", "gemini-color",
    "gemma-color", "deepseek-color", "qwen-color", "kimi-color", "zhipu-color", "minimax-color", "mistral-color", "xai", "meta-color",
    "nvidia-color", "longcat-color", "xiaomimimo", "cohere-color", "stepfun-color", "baidu-color", "perplexity-color",
];

/// A coding CLI's own mark.
pub(crate) fn cli_mark(provider: &str) -> Option<&'static str> {
    Some(match provider {
        "claude" => "claudecode-color",
        "codex" => "codex-color",
        "cursor" => "cursor",
        "opencode" => "opencode",
        "gemini" => "geminicli-color",
        "grok" => "grok",
        _ => return None,
    })
}

/// The maker behind a CLI's own short model names ("opus", "sonnet" in Claude Code).
pub(crate) fn house_maker(provider: &str) -> Option<&'static str> {
    Some(match provider {
        "claude" => "claude-color",
        "codex" => "openai",
        "gemini" => "gemini-color",
        "grok" => "xai",
        _ => return None,
    })
}

/// The maker of a model, from its id, family or provider ("anthropic/claude-sonnet",
/// "deepseek-v4.1-flash-free", "qwen"). None when it is not one we have a mark for.
pub(crate) fn maker_mark(text: &str) -> Option<&'static str> {
    let t = text.to_lowercase();
    let has = |k: &[&str]| k.iter().any(|k| t.contains(k));
    Some(if has(&["claude", "anthropic"]) {
        "claude-color"
    } else if has(&["gemma"]) {
        "gemma-color"
    } else if has(&["gemini", "google"]) {
        "gemini-color"
    } else if has(&["gpt", "openai", "codex", "o3-", "o4-"]) || t == "o3" || t == "o4" {
        "openai"
    } else if has(&["deepseek"]) {
        "deepseek-color"
    } else if has(&["qwen", "qwq", "alibaba"]) {
        "qwen-color"
    } else if has(&["kimi", "moonshot"]) {
        "kimi-color"
    } else if has(&["glm", "zhipu", "z-ai", "zai-"]) {
        "zhipu-color"
    } else if has(&["minimax"]) {
        "minimax-color"
    } else if has(&["mistral", "devstral", "codestral", "magistral", "ministral"]) {
        "mistral-color"
    } else if has(&["grok", "x-ai", "xai"]) {
        "xai"
    } else if has(&["llama", "meta-"]) {
        "meta-color"
    } else if has(&["nemotron", "nvidia"]) {
        "nvidia-color"
    } else if has(&["longcat"]) {
        "longcat-color"
    } else if has(&["mimo", "xiaomi"]) {
        "xiaomimimo"
    } else if has(&["cohere", "command-"]) {
        "cohere-color"
    } else if has(&["stepfun", "step-"]) {
        "stepfun-color"
    } else if has(&["ernie", "baidu"]) {
        "baidu-color"
    } else if has(&["sonar", "perplexity"]) {
        "perplexity-color"
    } else {
        return None;
    })
}

/// The SVG text of a mark: drawn large so it scales down cleanly, `currentColor` as `ink`.
pub(crate) fn svg(name: &str, ink: &str) -> Option<String> {
    let src = MARKS.iter().find(|(n, _)| *n == name)?.1;
    Some(src.replacen(r#"height="1em""#, r#"height="64""#, 1).replacen(r#"width="1em""#, r#"width="64""#, 1).replace("currentColor", ink))
}

/// A mark as an image for `img()`, cached per colour.
pub(crate) fn image(name: &str, ink: &str) -> Option<Arc<Image>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, String), Arc<Image>>>> = OnceLock::new();
    let key = (name.to_string(), ink.to_string());
    let cache = CACHE.get_or_init(Default::default);
    if let Some(i) = cache.lock().unwrap().get(&key) {
        return Some(i.clone());
    }
    let i = Arc::new(Image::from_bytes(ImageFormat::Svg, svg(name, ink)?.into_bytes()));
    cache.lock().unwrap().insert(key, i.clone());
    Some(i)
}

#[cfg(test)]
mod tests {
    use super::{cli_mark, maker_mark, svg, MARKS};

    #[::core::prelude::v1::test]
    fn every_cli_and_common_maker_has_a_mark_that_parses() {
        for p in ["claude", "codex", "cursor", "opencode", "gemini", "grok"] {
            let n = cli_mark(p).unwrap();
            assert!(svg(n, "#fff").unwrap().contains(r#"width="64""#), "{p}");
        }
        assert_eq!(maker_mark("opencode/deepseek-v4.1-flash-free"), Some("deepseek-color"));
        assert_eq!(maker_mark("anthropic/claude-sonnet-4-5"), Some("claude-color"));
        assert_eq!(maker_mark("openrouter/qwen/qwen3-coder"), Some("qwen-color"));
        assert_eq!(maker_mark("gpt-5.5-codex"), Some("openai"));
        assert_eq!(maker_mark("google/gemma-4"), Some("gemma-color"));
        assert_eq!(maker_mark("beam-free"), None);
        for (n, s) in MARKS {
            assert!(s.starts_with("<svg") && s.contains("viewBox"), "{n}");
        }
        // A monochrome mark takes the colour asked for.
        assert!(svg("opencode", "#abcdef").unwrap().contains("#abcdef"));
    }
}
