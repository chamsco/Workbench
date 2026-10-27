//! One metered turn: check the payer may, run it through `backspace-runner`,
//! book what it cost.
//!
//! Usage comes from the backend's own `Usage` event when it sends one
//! (OpenAI-compatible APIs, Ollama, Claude Code). A2A agents and some CLIs
//! report nothing; for those the turn is estimated at four characters a
//! token and booked as estimated, so an admin can see which numbers are
//! guesses. A seat on a CLI subscription (Claude Code on someone's own plan)
//! is not metered here at all: the org never sees those tokens, which is why
//! the catalog only holds API endpoints.

use std::sync::Mutex;

use anyhow::{anyhow, Result};
use backspace_runner::{Event, Request, Target};

use crate::governance::{Endpoint, Governor, Model, Payer, Trust};

/// The runner target for a catalog model. `secret` reads a key by name.
pub fn target(m: &Model, secret: &dyn Fn(&str) -> Option<String>) -> Result<Target> {
    Ok(match &m.endpoint {
        Endpoint::OpenAi { base, key_env } => Target::OpenAi {
            base: base.clone(),
            key: secret(key_env).ok_or_else(|| anyhow!("no key for {} ({key_env} is not set)", m.label))?,
        },
        Endpoint::Ollama { url } => Target::Ollama { url: url.clone() },
        Endpoint::A2a { url, token_env } => Target::A2a { url: url.clone(), token: token_env.as_deref().and_then(secret) },
    })
}

/// Four characters a token, the usual rough rate for English.
pub fn estimate(req: &Request, answer: &str) -> (u64, u64) {
    let input: usize = req.system.as_deref().map(str::len).unwrap_or(0) + req.history.iter().map(|t| t.text.len()).sum::<usize>();
    (input.div_ceil(4) as u64, answer.len().div_ceil(4) as u64)
}

pub struct Turned {
    pub text: String,
    pub credits: u64,
    pub estimated: bool,
}

/// Run `req` on catalog `model` for `payer`, refusing before it starts if the
/// policy says no. `on` sees every event, for streaming into the room.
#[allow(clippy::too_many_arguments)]
pub async fn turn(
    http: &reqwest::Client,
    gov: &Mutex<Governor>,
    payer: Payer,
    model: &str,
    trust: Trust,
    mut req: Request,
    secret: &dyn Fn(&str) -> Option<String>,
    on: &mut (dyn FnMut(&Event) + Send),
) -> Result<Turned> {
    let m = {
        let g = gov.lock().unwrap();
        g.check(&payer, model, trust, crate::now_ms()).map_err(|r| anyhow!("{r}"))?;
        g.model(model).cloned().ok_or_else(|| anyhow!("unknown model"))?
    };
    req.target = target(&m, secret)?;
    req.model = Some(m.model.clone());
    let (mut text, mut fin, mut usage) = (String::new(), String::new(), None);
    let res = backspace_runner::run(http, &req, &mut |e| {
        match &e {
            Event::Text { text: t } => text.push_str(t),
            Event::Replace { text: t } => text = t.clone(),
            Event::Final { text: t } => fin = t.clone(),
            Event::Usage { input, output } => {
                let (i, o) = usage.unwrap_or((0, 0));
                usage = Some((i + input, o + output));
            }
            _ => {}
        }
        on(&e);
    })
    .await;
    let text = if text.trim().is_empty() { fin } else { text };
    // A turn that failed halfway still spent what it spent.
    let estimated = usage.is_none();
    let (input, output) = usage.unwrap_or_else(|| estimate(&req, &text));
    let credits = gov.lock().unwrap().record(payer, model, input, output, estimated, crate::now_ms());
    res?;
    Ok(Turned { text, credits, estimated })
}
