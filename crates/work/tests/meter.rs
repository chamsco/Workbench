use std::collections::BTreeMap;
use std::sync::Mutex;

use backspace_runner::{Request, Target, Turn};
use brainpowr_work::governance::*;
use brainpowr_work::meter;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A one-shot Ollama that answers every request with `body` (NDJSON).
async fn ollama(body: &'static str) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move {
        while let Ok((mut s, _)) = l.accept().await {
            let mut buf = vec![0; 65536];
            let _ = s.read(&mut buf).await;
            let r = format!("HTTP/1.1 200 OK\r\ncontent-type: application/x-ndjson\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
            let _ = s.write_all(r.as_bytes()).await;
        }
    });
    url
}

fn gov(url: &str, credits: u64) -> Mutex<Governor> {
    let mut people = BTreeMap::new();
    people.insert("ann".to_string(), Person { id: "ann".into(), name: "Ann".into(), role: "dev".into(), teams: vec![] });
    Mutex::new(Governor {
        catalog: vec![Model {
            id: "local".into(),
            label: "Local".into(),
            model: "llama".into(),
            endpoint: Endpoint::Ollama { url: url.into() },
            residency: Residency::OrgHosted,
            weight: 2.0,
        }],
        policy: Policy {
            default: Grant { models: Some(vec!["local".into()]), allowance: Some(Allowance { credits, per: Period::Week }), external: None },
            ..Default::default()
        },
        people,
        ledger: vec![],
    })
}

fn req() -> Request {
    Request::new(Target::Ollama { url: String::new() }, vec![Turn::user("hello there!")])
}

fn http() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

#[tokio::test]
async fn books_reported_usage_at_the_model_weight() {
    let url = ollama("{\"message\":{\"content\":\"hi\"},\"done\":false}\n{\"message\":{\"content\":\"!\"},\"done\":true,\"prompt_eval_count\":10,\"eval_count\":5}\n").await;
    let g = gov(&url, 1000);
    let t = meter::turn(&http(), &g, Payer::Person("ann".into()), "local", Trust::Company, req(), &|_| None, &mut |_| {}).await.unwrap();
    assert_eq!((t.text.as_str(), t.credits, t.estimated), ("hi!", 30, false));
    assert_eq!(g.lock().unwrap().ledger.len(), 1);
}

#[tokio::test]
async fn estimates_when_the_backend_reports_nothing() {
    let url = ollama("{\"message\":{\"content\":\"12345678\"},\"done\":true}\n").await;
    let g = gov(&url, 1000);
    let t = meter::turn(&http(), &g, Payer::Person("ann".into()), "local", Trust::Company, req(), &|_| None, &mut |_| {}).await.unwrap();
    // "hello there!" is 12 chars: 3 tokens in, 2 out, weight 2.
    assert_eq!((t.credits, t.estimated), (10, true));
}

#[tokio::test]
async fn refuses_before_running_when_the_allowance_is_gone() {
    let g = gov("http://127.0.0.1:9", 0);
    let e = meter::turn(&http(), &g, Payer::Person("ann".into()), "local", Trust::Company, req(), &|_| None, &mut |_| {}).await.err().unwrap();
    assert!(e.to_string().contains("allowance used"), "{e}");
    assert!(g.lock().unwrap().ledger.is_empty());
}
