//! Backspace Cloud: chat without installing anything, on three plans.
//!
//! | plan | inside the monthly allowance        | past it                                  |
//! |------|-------------------------------------|------------------------------------------|
//! | Free | small models, one ad per reply      | stops until the period resets            |
//! | Plus | small + standard models, no ads     | small models only, one ad per reply      |
//! | Max  | every model, no ads                 | your choice: ads per reply, or pay as you go |
//!
//! Ads are a separate, labelled card under the reply. They are never written
//! into the model's answer and are not chosen from what you typed: the
//! server picks them without reading the conversation.
//!
//! [`decide`] is the whole policy and is shared by the client (to grey out
//! models in the picker) and the server (to enforce it). [`serve`] is a
//! development server with the same API, so the app can be exercised end to
//! end: it keeps accounts in a JSON file, switches plans without payment,
//! and answers through the models in the harness config.
//!
//!   POST /v1/signup                      {"token","account"}
//!   GET  /v1/account                     Account
//!   POST /v1/plan   {"plan","overage"}   Account   (production: a checkout URL)
//!   POST /v1/chat   {"model","messages"} text/event-stream of
//!        data: {"type":"model","model"} | {"type":"delta","text"} |
//!              {"type":"ad","ad"} | {"type":"done","account","charged_usd"} |
//!              {"type":"error","message"}

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Plan {
    Free,
    Plus,
    Max,
}

/// What Max does past its allowance.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Overage {
    /// Keep every model; one ad per reply.
    #[default]
    Ads,
    /// No ads; billed per token at cost plus a margin.
    Payg,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlanInfo {
    pub plan: Plan,
    pub name: &'static str,
    pub price: &'static str,
    /// Replies per month (per day for Free).
    pub allowance: u32,
    pub period: &'static str,
    pub blurb: &'static str,
    pub points: [&'static str; 3],
}

pub const PLANS: [PlanInfo; 3] = [
    PlanInfo {
        plan: Plan::Free,
        name: "Free",
        price: "$0",
        allowance: 50,
        period: "day",
        blurb: "Small, fast models. A short sponsored card under each reply.",
        points: [
            "50 replies a day",
            "Small models",
            "One ad per reply, never inside the answer",
        ],
    },
    PlanInfo {
        plan: Plan::Plus,
        name: "Plus",
        price: "$8 / month",
        allowance: 1500,
        period: "month",
        blurb: "Standard models with no ads, until the monthly allowance.",
        points: [
            "1,500 replies a month, no ads",
            "Small + standard models",
            "Past that: small models with ads",
        ],
    },
    PlanInfo {
        plan: Plan::Max,
        name: "Max",
        price: "$30 / month",
        allowance: 5000,
        period: "month",
        blurb: "Every model, never an ad inside the allowance.",
        points: [
            "5,000 replies a month, no ads",
            "Every model, frontier included",
            "Past that: ads, or pay as you go",
        ],
    },
];

pub fn plan_info(p: Plan) -> &'static PlanInfo {
    &PLANS[p as usize]
}

/// Model sizes the plans are written in.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Size {
    Small,
    Standard,
    Frontier,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CloudModel {
    pub id: String,
    pub label: String,
    pub size: Size,
    /// Filled in per account: may this account use it right now.
    #[serde(default)]
    pub allowed: bool,
    #[serde(default)]
    pub with_ads: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Account {
    pub plan: Plan,
    pub overage: Overage,
    /// Replies used this period.
    pub used: u32,
    pub allowance: u32,
    pub period: String,
    /// Unix ms when the allowance resets.
    pub resets_at: u64,
    /// Pay-as-you-go spend this period.
    pub payg_usd: f64,
    pub models: Vec<CloudModel>,
}

/// A sponsored card.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Ad {
    pub id: String,
    pub advertiser: String,
    pub title: String,
    pub body: String,
    pub cta: String,
    pub url: String,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Decision {
    /// Answer with no ad.
    Clean,
    /// Answer, then show an ad.
    WithAd,
    /// Answer and bill per token.
    Payg,
    /// Model not on this plan (or not past the allowance).
    NeedsUpgrade,
    /// Free tier: out of replies until the reset.
    OutOfReplies,
}

/// The plan table, as code.
pub fn decide(plan: Plan, overage: Overage, used: u32, allowance: u32, size: Size) -> Decision {
    let within = used < allowance;
    match plan {
        Plan::Free => {
            if !within {
                Decision::OutOfReplies
            } else if size == Size::Small {
                Decision::WithAd
            } else {
                Decision::NeedsUpgrade
            }
        }
        Plan::Plus => match (within, size) {
            (true, Size::Small | Size::Standard) => Decision::Clean,
            (false, Size::Small) => Decision::WithAd,
            _ => Decision::NeedsUpgrade,
        },
        Plan::Max => match (within, overage) {
            (true, _) => Decision::Clean,
            (false, Overage::Ads) => Decision::WithAd,
            (false, Overage::Payg) => Decision::Payg,
        },
    }
}

pub fn catalog() -> Vec<CloudModel> {
    let m = |id: &str, label: &str, size| CloudModel {
        id: id.into(),
        label: label.into(),
        size,
        allowed: false,
        with_ads: false,
    };
    vec![
        m("claude-haiku-4-5", "Claude Haiku 4.5", Size::Small),
        m("claude-sonnet-5-5", "Claude Sonnet 5.5", Size::Standard),
        m("claude-opus-5-5", "Claude Opus 5.5", Size::Frontier),
    ]
}

fn fill(models: &mut [CloudModel], plan: Plan, overage: Overage, used: u32, allowance: u32) {
    for m in models {
        let d = decide(plan, overage, used, allowance, m.size);
        m.allowed = matches!(d, Decision::Clean | Decision::WithAd | Decision::Payg);
        m.with_ads = d == Decision::WithAd;
    }
}

// ---------------------------------------------------------------- client

pub async fn signup(http: &reqwest::Client, url: &str) -> Result<(String, Account)> {
    let v: Value = call(http, url, "", "POST", "/v1/signup", json!({})).await?;
    let token = v["token"]
        .as_str()
        .ok_or_else(|| anyhow!("no token"))?
        .to_string();
    Ok((token, serde_json::from_value(v["account"].clone())?))
}

pub async fn account(http: &reqwest::Client, url: &str, token: &str) -> Result<Account> {
    Ok(serde_json::from_value(
        call(http, url, token, "GET", "/v1/account", Value::Null).await?,
    )?)
}

pub async fn set_plan(
    http: &reqwest::Client,
    url: &str,
    token: &str,
    plan: Plan,
    overage: Overage,
) -> Result<Account> {
    Ok(serde_json::from_value(
        call(
            http,
            url,
            token,
            "POST",
            "/v1/plan",
            json!({"plan": plan, "overage": overage}),
        )
        .await?,
    )?)
}

async fn call(
    http: &reqwest::Client,
    url: &str,
    token: &str,
    method: &str,
    path: &str,
    body: Value,
) -> Result<Value> {
    let full = format!("{}{path}", url.trim_end_matches('/'));
    let mut rb = if method == "GET" {
        http.get(&full)
    } else {
        http.post(&full).json(&body)
    };
    if !token.is_empty() {
        rb = rb.bearer_auth(token);
    }
    let r = rb
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                anyhow!("Backspace Cloud is unreachable at {url}")
            } else {
                anyhow!(e)
            }
        })?;
    let status = r.status();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!(
            "{}",
            v["error"]
                .as_str()
                .unwrap_or(&format!("Cloud: {status}"))
                .to_string()
        );
    }
    Ok(v)
}

// ---------------------------------------------------------------- dev server

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Acct {
    plan: Plan,
    overage: Overage,
    used: u32,
    period_start: u64,
    payg_usd: f64,
}

struct Server {
    file: PathBuf,
    accounts: Mutex<HashMap<String, Acct>>,
    ads: Vec<Ad>,
    next_ad: Mutex<usize>,
    cfg: crate::Config,
    http: reqwest::Client,
    /// Answer with a fixed text instead of calling a model (no keys needed).
    mock: bool,
}

const DAY: u64 = 24 * 3600 * 1000;

fn period_ms(plan: Plan) -> u64 {
    if plan == Plan::Free {
        DAY
    } else {
        30 * DAY
    }
}

/// House ads for development. Production would fill these from an ad
/// network, still without sending it the conversation.
fn house_ads() -> Vec<Ad> {
    let a = |id: &str, adv: &str, title: &str, body: &str, cta: &str, url: &str| Ad {
        id: id.into(),
        advertiser: adv.into(),
        title: title.into(),
        body: body.into(),
        cta: cta.into(),
        url: url.into(),
    };
    vec![
        a(
            "bs-plus",
            "Backspace",
            "No ads, better models",
            "Plus removes ads and adds standard models for $8 a month.",
            "See plans",
            "backspace://plans",
        ),
        a(
            "bs-local",
            "Backspace",
            "Run models on your own machine",
            "Ollama chats are free, private and offline. Set it up in Settings.",
            "Set up Ollama",
            "backspace://settings/providers",
        ),
        a(
            "bs-cli",
            "Backspace",
            "Already pay for Claude or ChatGPT?",
            "Chat through your Claude Code or Codex subscription instead.",
            "Connect a CLI",
            "backspace://settings/providers",
        ),
    ]
}

impl Server {
    fn account_view(&self, a: &Acct) -> Account {
        let info = plan_info(a.plan);
        let mut models = catalog();
        fill(&mut models, a.plan, a.overage, a.used, info.allowance);
        Account {
            plan: a.plan,
            overage: a.overage,
            used: a.used,
            allowance: info.allowance,
            period: info.period.into(),
            resets_at: a.period_start + period_ms(a.plan),
            payg_usd: a.payg_usd,
            models,
        }
    }

    fn save(&self) {
        let accts = self.accounts.lock().unwrap().clone();
        if let Ok(s) = serde_json::to_string_pretty(&accts) {
            let _ = std::fs::write(&self.file, s);
        }
    }

    /// The account for a token, with its period rolled over if due.
    fn get(&self, token: &str) -> Option<Acct> {
        let mut accts = self.accounts.lock().unwrap();
        let a = accts.get_mut(token)?;
        let now = crate::chat::now_ms();
        if now >= a.period_start + period_ms(a.plan) {
            a.period_start = now;
            a.used = 0;
            a.payg_usd = 0.0;
        }
        Some(a.clone())
    }

    fn next_ad(&self) -> Ad {
        let mut i = self.next_ad.lock().unwrap();
        let ad = self.ads[*i % self.ads.len()].clone();
        *i += 1;
        ad
    }
}

fn bearer(req: &crate::remote::Request) -> &str {
    req.auth
        .as_deref()
        .and_then(|a| a.strip_prefix("Bearer "))
        .unwrap_or("")
}

async fn sse(stream: &mut TcpStream, v: Value) -> Result<()> {
    stream
        .write_all(format!("data: {v}\n\n").as_bytes())
        .await?;
    stream.flush().await?;
    Ok(())
}

async fn handle(mut stream: TcpStream, srv: Arc<Server>) -> Result<()> {
    use crate::remote::{read_request, respond};
    let req = read_request(&mut stream).await?;
    let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
    match (req.method.as_str(), req.path.as_str()) {
        ("POST", "/v1/signup") => {
            let token = format!("bsc_{}", crate::remote::new_token());
            let a = Acct {
                plan: Plan::Free,
                overage: Overage::Ads,
                used: 0,
                period_start: crate::chat::now_ms(),
                payg_usd: 0.0,
            };
            let view = srv.account_view(&a);
            srv.accounts.lock().unwrap().insert(token.clone(), a);
            srv.save();
            respond(&mut stream, 200, &json!({"token": token, "account": view})).await
        }
        ("GET", "/v1/account") => match srv.get(bearer(&req)) {
            Some(a) => respond(&mut stream, 200, &json!(srv.account_view(&a))).await,
            None => {
                respond(
                    &mut stream,
                    401,
                    &json!({"error": "unknown account; sign up again"}),
                )
                .await
            }
        },
        ("POST", "/v1/plan") => {
            let token = bearer(&req).to_string();
            let plan: Option<Plan> = serde_json::from_value(body["plan"].clone()).ok();
            let overage: Overage =
                serde_json::from_value(body["overage"].clone()).unwrap_or_default();
            let view = {
                let mut accts = srv.accounts.lock().unwrap();
                match (accts.get_mut(&token), plan) {
                    (Some(a), Some(p)) => {
                        if a.plan != p {
                            a.plan = p;
                            a.used = 0;
                            a.period_start = crate::chat::now_ms();
                        }
                        a.overage = overage;
                        Some(a.clone())
                    }
                    _ => None,
                }
            };
            srv.save();
            match view {
                Some(a) => respond(&mut stream, 200, &json!(srv.account_view(&a))).await,
                None => {
                    respond(
                        &mut stream,
                        400,
                        &json!({"error": "unknown account or plan"}),
                    )
                    .await
                }
            }
        }
        ("POST", "/v1/chat") => chat(stream, srv, bearer(&req).to_string(), body).await,
        _ => respond(&mut stream, 404, &json!({"error": "not found"})).await,
    }
}

async fn chat(mut stream: TcpStream, srv: Arc<Server>, token: String, body: Value) -> Result<()> {
    use crate::remote::respond;
    let Some(a) = srv.get(&token) else {
        return respond(
            &mut stream,
            401,
            &json!({"error": "unknown account; sign up again"}),
        )
        .await;
    };
    let info = plan_info(a.plan);
    let cat = catalog();
    let want = body["model"].as_str().unwrap_or("");
    // No model chosen: the best one this account may use right now.
    let model = cat
        .iter()
        .rev()
        .find(|m| {
            if want.is_empty() {
                matches!(
                    decide(a.plan, a.overage, a.used, info.allowance, m.size),
                    Decision::Clean | Decision::WithAd | Decision::Payg
                )
            } else {
                m.id == want
            }
        })
        .cloned();
    let Some(model) = model else {
        return respond(
            &mut stream,
            400,
            &json!({"error": format!("unknown model {want}")}),
        )
        .await;
    };
    let decision = decide(a.plan, a.overage, a.used, info.allowance, model.size);
    match decision {
        Decision::OutOfReplies => {
            return respond(
                &mut stream,
                429,
                &json!({"error": "You've used today's free replies. Upgrade, or chat through Ollama or a CLI.", "account": srv.account_view(&a)}),
            )
            .await
        }
        Decision::NeedsUpgrade => {
            return respond(
                &mut stream,
                402,
                &json!({"error": format!("{} isn't on your plan right now. Upgrade, or pick a smaller model.", model.label), "account": srv.account_view(&a)}),
            )
            .await
        }
        _ => {}
    }
    stream
        .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n")
        .await?;
    sse(&mut stream, json!({"type": "model", "model": model.label})).await?;
    let (text, usd) = match answer(&srv, &model.id, &body["messages"]).await {
        Ok(x) => x,
        Err(e) => {
            sse(
                &mut stream,
                json!({"type": "error", "message": e.to_string()}),
            )
            .await?;
            return Ok(());
        }
    };
    // Whole answer from a non-streaming call, sent in pieces.
    let mut piece = String::new();
    for w in text.split_inclusive(' ') {
        piece.push_str(w);
        if piece.len() > 24 {
            sse(&mut stream, json!({"type": "delta", "text": piece})).await?;
            piece.clear();
        }
    }
    if !piece.is_empty() {
        sse(&mut stream, json!({"type": "delta", "text": piece})).await?;
    }
    if decision == Decision::WithAd {
        sse(&mut stream, json!({"type": "ad", "ad": srv.next_ad()})).await?;
    }
    let charged = if decision == Decision::Payg {
        usd * 1.2
    } else {
        0.0
    };
    let view = {
        let mut accts = srv.accounts.lock().unwrap();
        let acct = accts.get_mut(&token).unwrap();
        acct.used += 1;
        acct.payg_usd += charged;
        srv.account_view(acct)
    };
    srv.save();
    sse(
        &mut stream,
        json!({"type": "done", "account": view, "charged_usd": charged}),
    )
    .await
}

async fn answer(srv: &Server, model: &str, messages: &Value) -> Result<(String, f64)> {
    use crate::provider::{self, Block, Message, Role};
    let msgs: Vec<Message> = messages
        .as_array()
        .map(|a| {
            a.iter()
                .map(|m| {
                    let text = match &m["content"] {
                        Value::String(s) => s.clone(),
                        Value::Array(parts) => parts
                            .iter()
                            .filter_map(|p| p["text"].as_str())
                            .collect::<Vec<_>>()
                            .join("\n"),
                        _ => String::new(),
                    };
                    Message {
                        role: if m["role"] == "assistant" {
                            Role::Assistant
                        } else {
                            Role::User
                        },
                        content: vec![Block::Text { text }],
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    if srv.mock {
        let last = msgs
            .last()
            .map(|m| {
                m.content
                    .iter()
                    .map(|b| match b {
                        Block::Text { text } => text.as_str(),
                        _ => "",
                    })
                    .collect::<String>()
            })
            .unwrap_or_default();
        return Ok((
            format!("(Mock cloud reply from {model}.) You said: \u{201c}{}\u{201d}. Start the cloud server without BACKSPACE_CLOUD_MOCK to get real answers.", last.trim()),
            0.0004,
        ));
    }
    let spec = srv
        .cfg
        .models
        .iter()
        .find(|m| m.id == model)
        .ok_or_else(|| anyhow!("the cloud server's config has no model {model}"))?;
    let prov = srv
        .cfg
        .providers
        .get(&spec.provider)
        .ok_or_else(|| anyhow!("no provider {}", spec.provider))?;
    if prov.api_key().is_none() {
        bail!("the cloud server has no API key for {}", spec.provider);
    }
    let c = provider::complete(
        &srv.http,
        provider::Request {
            provider: prov,
            model: spec,
            system: "You are a helpful assistant in a chat app. Answer clearly; use Markdown when it helps.",
            messages: &msgs,
            tools: &[],
            effort: crate::Effort::Low,
        },
    )
    .await?;
    let usd = (c.input_tokens as f64 * spec.input_usd_per_mtok
        + c.output_tokens as f64 * spec.output_usd_per_mtok)
        / 1e6;
    Ok((c.text(), usd))
}

/// Run the development cloud. `dir` keeps accounts.json.
pub async fn serve(addr: &str, dir: PathBuf, mock: bool) -> Result<()> {
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("cloud-accounts.json");
    let accounts = std::fs::read_to_string(&file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let (cfg, _) = crate::Config::load(&dir)?;
    let srv = Arc::new(Server {
        file,
        accounts: Mutex::new(accounts),
        ads: house_ads(),
        next_ad: Mutex::new(0),
        cfg,
        http: reqwest::Client::new(),
        mock,
    });
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (s, _) = listener.accept().await?;
        let srv = srv.clone();
        tokio::spawn(async move {
            let _ = handle(s, srv).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Decision::*;

    #[test]
    fn plan_table() {
        // Free: small models with an ad, nothing bigger, stop at the limit.
        assert_eq!(decide(Plan::Free, Overage::Ads, 0, 50, Size::Small), WithAd);
        assert_eq!(
            decide(Plan::Free, Overage::Ads, 0, 50, Size::Standard),
            NeedsUpgrade
        );
        assert_eq!(
            decide(Plan::Free, Overage::Ads, 50, 50, Size::Small),
            OutOfReplies
        );
        // Plus: no ads within; past it, small with ads only.
        assert_eq!(
            decide(Plan::Plus, Overage::Ads, 10, 1500, Size::Standard),
            Clean
        );
        assert_eq!(
            decide(Plan::Plus, Overage::Ads, 10, 1500, Size::Frontier),
            NeedsUpgrade
        );
        assert_eq!(
            decide(Plan::Plus, Overage::Ads, 1500, 1500, Size::Small),
            WithAd
        );
        assert_eq!(
            decide(Plan::Plus, Overage::Ads, 1500, 1500, Size::Standard),
            NeedsUpgrade
        );
        // Max: everything clean; past it, the user's choice.
        assert_eq!(
            decide(Plan::Max, Overage::Ads, 0, 5000, Size::Frontier),
            Clean
        );
        assert_eq!(
            decide(Plan::Max, Overage::Ads, 5000, 5000, Size::Frontier),
            WithAd
        );
        assert_eq!(
            decide(Plan::Max, Overage::Payg, 5000, 5000, Size::Frontier),
            Payg
        );
    }

    #[test]
    fn dev_server_end_to_end() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let dir = std::env::temp_dir().join(format!("bs-cloud-{}", crate::chat::now_ms()));
            let addr = "127.0.0.1:47431";
            tokio::spawn({
                let dir = dir.clone();
                async move { serve(addr, dir, true).await }
            });
            let http = reqwest::Client::new();
            let url = format!("http://{addr}");
            // Wait for the listener (it also loads the config first).
            let mut tries = 0;
            let (token, acct) = loop {
                match signup(&http, &url).await {
                    Ok(x) => break x,
                    Err(e) if tries < 100 => {
                        tries += 1;
                        let _ = e;
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            assert_eq!(acct.plan, Plan::Free);
            assert!(acct.models.iter().any(|m| m.allowed && m.with_ads));
            assert!(acct.models.iter().any(|m| !m.allowed));
            // Free asking for a frontier model: 402 with a reason.
            let r = http
                .post(format!("{url}/v1/chat"))
                .bearer_auth(&token)
                .json(&json!({"model": "claude-opus-5-5", "messages": [{"role": "user", "content": "hi"}]}))
                .send()
                .await
                .unwrap();
            assert_eq!(r.status().as_u16(), 402);
            // Free with the small model: an answer, then an ad.
            let body = http
                .post(format!("{url}/v1/chat"))
                .bearer_auth(&token)
                .json(&json!({"model": "claude-haiku-4-5", "messages": [{"role": "user", "content": "hi"}]}))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            assert!(body.contains("\"type\":\"delta\""), "{body}");
            assert!(body.contains("\"type\":\"ad\""), "{body}");
            assert!(body.contains("\"used\":1"), "{body}");
            // Max: no ad.
            let a = set_plan(&http, &url, &token, Plan::Max, Overage::Ads).await.unwrap();
            assert_eq!(a.plan, Plan::Max);
            assert!(a.models.iter().all(|m| m.allowed && !m.with_ads));
            let body = http
                .post(format!("{url}/v1/chat"))
                .bearer_auth(&token)
                .json(&json!({"model": "claude-opus-5-5", "messages": [{"role": "user", "content": "hi"}]}))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            assert!(!body.contains("\"type\":\"ad\""), "{body}");
            let _ = std::fs::remove_dir_all(dir);
        });
    }
}
