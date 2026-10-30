//! M1 hardening, F2: the input bound is read off the request that is sent, in
//! bytes, with allowances for what the provider adds, and Anthropic input is
//! priced at its cache-write rate. No tokenizer is available here, so these
//! tests check the properties the bound is built to have (never below the
//! body's bytes, grows with messages and tools), not agreement with a real
//! tokenizer.

use backspace_core::config::{Config, ModelSpec, ProviderConfig, ProviderKind, DEFAULT_CONFIG};
use backspace_core::effort::Effort;
use backspace_core::gateway::{estimate_tokens, input_bound, BudgetLine, Gateway, PolicyConfig, Role};
use backspace_core::provider::{self, input_price_factor, Message, Request, ToolDef};
use serde_json::json;

fn model() -> ModelSpec {
    ModelSpec {
        id: "m".into(),
        provider: "p".into(),
        model: None,
        tier: 1,
        description: String::new(),
        input_usd_per_mtok: 10.0,
        output_usd_per_mtok: 20.0,
        max_output_tokens: 1000,
        efforts: vec![],
        class: None,
    }
}

fn provider_of(kind: ProviderKind) -> ProviderConfig {
    ProviderConfig { kind, base_url: "http://127.0.0.1:1".into(), api_key_env: String::new(), cli: None }
}

fn prepared(kind: ProviderKind, system: &str, user: &str, tools: &[ToolDef]) -> provider::Prepared {
    let (p, m) = (provider_of(kind), model());
    let msgs = [Message::user_text(user)];
    provider::prepare(&Request { provider: &p, model: &m, effort: Effort::High, system, messages: &msgs, tools, advisor: None }).unwrap()
}

/// Text that tokenizes worse than English prose does.
fn awkward() -> Vec<(&'static str, String)> {
    let rust = "fn main() { let v: Vec<u8> = (0..=255).map(|b| b ^ 0x5a).collect(); println!(\"{:#?}\", &v[..]); }\n";
    let mut b64 = String::new();
    let mut x: u32 = 0x1234_5678;
    for _ in 0..6000 {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        b64.push(b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"[(x >> 26) as usize] as char);
    }
    vec![
        ("cjk", "漢字仮名交じり文".repeat(800)),
        ("emoji", "🦀🔥🧪".repeat(1500)),
        ("combining", "e\u{301}\u{302}\u{303}".repeat(2000)),
        ("source code", rust.repeat(120)),
        ("minified json", format!("[{}]", (0..1500).map(|i| format!("{{\"k{i}\":[{i},null,true]}}")).collect::<Vec<_>>().join(","))),
        ("base64", b64),
        ("mixed", "Größe ≠ 大きさ — 🙂 ".repeat(900)),
    ]
}

#[test]
fn the_bound_is_never_below_the_bytes_of_the_body_that_is_sent() {
    for (name, text) in awkward() {
        let p = prepared(ProviderKind::Openai, "You are a coding agent.", &text, &[]);
        let bound = input_bound(p.bytes.len(), p.messages, p.tools);
        assert!(bound >= p.bytes.len() as u64, "{name}: bound {bound} for {} bytes", p.bytes.len());
        // Two tokens a character is worse than any tokenizer does on these, but a token is
        // never less than a byte (the bound's assumption), so for text that takes fewer than
        // two bytes a character the bytes are the limit.
        let chars = text.chars().count() as u64;
        assert!(bound >= (2 * chars).min(p.bytes.len() as u64), "{name}: bound {bound} for {chars} characters");
    }
}

#[test]
fn counting_characters_was_not_enough_for_text_that_is_not_english() {
    // The reason for the change, kept as a test: for the same body, characters / 3 is
    // far below the number of bytes that must each be at least part of a token.
    for (name, text) in awkward().into_iter().filter(|(n, _)| ["cjk", "emoji", "combining"].contains(n)) {
        let p = prepared(ProviderKind::Openai, "s", &text, &[]);
        let by_chars = estimate_tokens(text.chars().count());
        assert!(by_chars * 2 < p.bytes.len() as u64, "{name}: {by_chars} vs {} bytes", p.bytes.len());
        assert!(input_bound(p.bytes.len(), p.messages, p.tools) > by_chars * 2, "{name}");
    }
}

#[test]
fn long_tool_schemas_are_counted_from_the_wire_body() {
    let tools: Vec<ToolDef> = (0..60)
        .map(|i| ToolDef {
            name: provider::leak(&format!("tool_{i}")),
            description: provider::leak(&"Does something quite specific. ".repeat(40)),
            schema: json!({"type": "object", "properties": (0..25).map(|j| (format!("field_{j}"), json!({"type": "string", "description": "x".repeat(60)}))).collect::<serde_json::Map<_, _>>()}),
        })
        .collect();
    for kind in [ProviderKind::Openai, ProviderKind::Anthropic] {
        let bare = prepared(kind, "s", "hi", &[]);
        let with = prepared(kind, "s", "hi", &tools);
        assert_eq!(with.tools, 60);
        let schema_bytes: usize = tools.iter().map(|t| t.schema.to_string().len() + t.description.len()).sum();
        let (b0, b1) = (input_bound(bare.bytes.len(), bare.messages, bare.tools), input_bound(with.bytes.len(), with.messages, with.tools));
        assert!(b1 >= b0 + schema_bytes as u64, "{kind:?}: {b0} -> {b1}, the schemas alone are {schema_bytes} bytes");
    }
}

#[test]
fn the_allowances_grow_with_messages_and_tools_and_are_not_zero() {
    let base = input_bound(1000, 2, 3);
    assert!(base > 1000, "something is allowed for what the provider adds around the body: {base}");
    assert!(input_bound(1000, 3, 3) > base, "per message");
    assert!(input_bound(1000, 2, 4) > base, "per tool");
    assert!(input_bound(0, 0, 0) > 0, "an empty request is not free");
}

#[test]
fn the_system_prompt_counts_as_a_message_for_both_wire_formats() {
    for kind in [ProviderKind::Openai, ProviderKind::Anthropic] {
        let p = prepared(kind, "system text", "user text", &[]);
        assert_eq!(p.messages, 2, "{kind:?}: the system prompt and one user message");
    }
}

#[test]
fn the_prepared_bytes_are_the_body() {
    let p = prepared(ProviderKind::Anthropic, "s", "hello", &[]);
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&p.bytes).unwrap(), p.body);
    assert!(p.url.ends_with("/v1/messages"));
}

// --------------------------------------------------------------------- price

#[test]
fn only_anthropic_input_is_priced_above_its_listed_rate() {
    assert_eq!(input_price_factor(ProviderKind::Anthropic), 1.25);
    assert_eq!(input_price_factor(ProviderKind::Openai), 1.0);
    assert_eq!(input_price_factor(ProviderKind::Cli), 1.0);
}

#[test]
fn a_call_that_fits_at_the_listed_price_can_be_refused_at_the_cache_write_price() {
    let g = Gateway::new(PolicyConfig::default());
    let m = model();
    // A million prompt tokens at $10 per million is $10, or $12.50 as a cache write.
    let line = |remaining: f64| vec![BudgetLine { label: "project".into(), limit_usd: Some(remaining), limit_tokens: None, spent_usd: 0.0, spent_tokens: 0 }];
    assert!(g.authorize_call_priced(Role::Execution, &m, 1_000_000, 1.0, &line(10.5)).is_ok());
    let e = g.authorize_call_priced(Role::Execution, &m, 1_000_000, 1.25, &line(10.5)).unwrap_err().to_string();
    assert!(e.contains("budget"), "{e}");
    assert!(g.authorize_call_priced(Role::Execution, &m, 1_000_000, 1.25, &line(12.6)).is_ok());
    // authorize_call keeps meaning "listed price".
    assert_eq!(
        g.authorize_call(Role::Execution, &m, 1_000, &line(5.0)).unwrap(),
        g.authorize_call_priced(Role::Execution, &m, 1_000, 1.0, &line(5.0)).unwrap()
    );
}

#[test]
fn the_default_config_still_parses_and_prices_anthropic_models() {
    // The factor applies to the models the default config ships, which are Anthropic's.
    let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
    let kinds: Vec<_> = cfg.models.iter().filter_map(|m| cfg.providers.get(&m.provider)).map(|p| p.kind).collect();
    assert!(kinds.contains(&ProviderKind::Anthropic));
}
