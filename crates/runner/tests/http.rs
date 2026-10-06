//! The HTTP targets against tiny local servers: an OpenAI-compatible
//! stream, and an A2A agent that answers with a task it finishes on the
//! second look.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use backspace_runner::{a2a, ask, run, Event, Request, Target, Turn};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Serve each request with `reply(path, body) -> (content-type, body)`.
async fn serve(reply: impl Fn(&str, &str) -> (String, String) + Send + Sync + 'static) -> String {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    let reply = Arc::new(reply);
    tokio::spawn(async move {
        loop {
            let (mut s, _) = l.accept().await.unwrap();
            let reply = reply.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 65536];
                let mut got = 0;
                // Read headers, then the body by Content-Length.
                let (head_end, len) = loop {
                    let n = s.read(&mut buf[got..]).await.unwrap();
                    got += n;
                    let text = String::from_utf8_lossy(&buf[..got]).to_string();
                    if let Some(i) = text.find("\r\n\r\n") {
                        let len = text[..i]
                            .lines()
                            .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                            .unwrap_or(0);
                        break (i + 4, len);
                    }
                };
                while got < head_end + len {
                    got += s.read(&mut buf[got..]).await.unwrap();
                }
                let req = String::from_utf8_lossy(&buf[..got]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (ct, body) = reply(&path, &req[head_end..]);
                let resp = format!("HTTP/1.1 200 OK\r\ncontent-type: {ct}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
                s.write_all(resp.as_bytes()).await.unwrap();
            });
        }
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn openai_compatible_stream() {
    let base = serve(|path, body| {
        assert_eq!(path, "/v1/chat/completions");
        assert!(body.contains(r#""model":"muse-spark""#) && body.contains("Be brief") && body.contains("hello"));
        let sse = [
            r#"data: {"choices":[{"delta":{"content":"Hi "}}]}"#,
            r#"data: {"choices":[{"delta":{"content":"there"}}]}"#,
            r#"data: {"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":2}}"#,
            "data: [DONE]",
        ]
        .join("\n\n");
        ("text/event-stream".into(), sse)
    })
    .await;
    let mut req = Request::new(Target::OpenAi { base: format!("{base}/v1"), key: "k".into() }, vec![Turn::user("hello")]);
    req.model = Some("muse-spark".into());
    req.system = Some("Be brief".into());
    let http = reqwest::Client::new();
    let mut ev = vec![];
    run(&http, &req, &mut |e| ev.push(e)).await.unwrap();
    assert!(ev.contains(&Event::Usage { input: 12, output: 2 }));
    assert_eq!(ask(&http, &req).await.unwrap(), "Hi there");
}

#[tokio::test]
async fn a2a_card_and_task() {
    let polls = Arc::new(AtomicUsize::new(0));
    let p = polls.clone();
    let base = serve(move |path, body| {
        let json = "application/json".to_string();
        if path == "/.well-known/agent-card.json" {
            return (json, r#"{"name":"WorkBot","description":"Files expenses","url":"","skills":[{"id":"exp","name":"Expenses"}]}"#.into());
        }
        let v: serde_json::Value = serde_json::from_str(body).unwrap();
        let rid = v["id"].clone();
        match v["method"].as_str().unwrap() {
            "message/send" => {
                assert_eq!(v["params"]["message"]["parts"][0]["text"], "file my taxi receipt");
                (json, serde_json::json!({"jsonrpc":"2.0","id":rid,"result":{"kind":"task","id":"t9","contextId":"c1","status":{"state":"working"}}}).to_string())
            }
            "tasks/get" => {
                p.fetch_add(1, Ordering::SeqCst);
                (json, serde_json::json!({"jsonrpc":"2.0","id":rid,"result":{"kind":"task","id":"t9","contextId":"c1","status":{"state":"completed"},
                    "artifacts":[{"parts":[{"kind":"text","text":"Filed: €18.40 taxi, 6 Oct."}]}]}}).to_string())
            }
            m => panic!("unexpected {m}"),
        }
    })
    .await;
    let http = reqwest::Client::new();
    let card = a2a::card(&http, &base, None).await.unwrap();
    assert_eq!(card.name, "WorkBot");
    assert_eq!(card.url, base, "an empty url in the card means the base");
    let req = Request::new(Target::A2a { url: card.url.clone(), token: None }, vec![Turn::user("file my taxi receipt")]);
    let mut ev = vec![];
    run(&http, &req, &mut |e| ev.push(e)).await.unwrap();
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert!(ev.contains(&Event::Session { id: "c1".into() }));
    assert!(ev.contains(&Event::Text { text: "Filed: €18.40 taxi, 6 Oct.".into() }));
    assert!(ev.iter().any(|e| matches!(e, Event::ToolEnd { output, error: false, .. } if output == "completed")));
}

#[tokio::test]
async fn bad_agent_addresses_say_so() {
    let http = reqwest::Client::new();
    let e = a2a::card(&http, "http", None).await.unwrap_err().to_string();
    assert!(e.contains("isn't a web address") || e.contains("couldn't reach"), "{e}");
    let e = a2a::card(&http, "http://127.0.0.1:1", None).await.unwrap_err().to_string();
    assert!(e.contains("couldn't reach"), "{e}");
}
