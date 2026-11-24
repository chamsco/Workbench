//! A link's preview card (Charles, 23 Nov: "a preview for a link"): the page's title, the site
//! and a line of description, read from its Open Graph tags or its `<title>`. Fetched once per URL
//! by the app, off the UI thread, with a short timeout and a cap on how much is read.

use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Unfurl {
    /// The host the link points at, without "www.".
    pub site: String,
    pub title: Option<String>,
    pub description: Option<String>,
}

/// The host of `url`, without "www.".
pub fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let h = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let h = h.rsplit('@').next().unwrap_or(h);
    h.strip_prefix("www.").unwrap_or(h).to_string()
}

/// Fetches `url` and reads its preview. Blocking: call it from a background thread. A page that
/// cannot be read still gives its site.
pub fn fetch(url: &str) -> Unfurl {
    let base = Unfurl { site: host(url), ..Default::default() };
    let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return base };
    let got = rt.block_on(async {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).user_agent("Mozilla/5.0 (compatible; link preview)").build().ok()?;
        let mut resp = client.get(url).send().await.ok()?;
        let html = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_none_or(|t| t.contains("html"));
        if !resp.status().is_success() || !html {
            return None;
        }
        // The head is enough; a big page stops at 256 KB.
        let mut body = Vec::new();
        while let Ok(Some(chunk)) = resp.chunk().await {
            body.extend_from_slice(&chunk);
            if body.len() > 256 * 1024 || String::from_utf8_lossy(&body).contains("</head>") {
                break;
            }
        }
        Some(String::from_utf8_lossy(&body).to_string())
    });
    match got {
        Some(html) => Unfurl { site: base.site, ..parse(&html) },
        None => base,
    }
}

/// The preview a page's HTML gives: og:title, else twitter:title, else `<title>`; og:description,
/// else the meta description; og:site_name for the site when there is one.
pub fn parse(html: &str) -> Unfurl {
    let low = html.to_ascii_lowercase();
    let mut metas: Vec<(String, String)> = Vec::new();
    let mut at = 0;
    while let Some(i) = low[at..].find("<meta") {
        let start = at + i;
        let end = low[start..].find('>').map_or(low.len(), |e| start + e);
        let tag = &html[start..end];
        let key = attr(tag, "property").or_else(|| attr(tag, "name")).map(|k| k.to_ascii_lowercase());
        if let (Some(k), Some(v)) = (key, attr(tag, "content")) {
            metas.push((k, v));
        }
        at = end.max(start + 5);
    }
    let meta = |names: &[&str]| names.iter().find_map(|n| metas.iter().find(|(k, v)| k == n && !v.trim().is_empty()).map(|(_, v)| clean(v)));
    let title_tag = low.find("<title").and_then(|i| {
        let open = i + low[i..].find('>')? + 1;
        let close = open + low[open..].find("</title")?;
        Some(clean(&html[open..close])).filter(|t| !t.is_empty())
    });
    Unfurl {
        site: meta(&["og:site_name"]).unwrap_or_default(),
        title: meta(&["og:title", "twitter:title"]).or(title_tag),
        description: meta(&["og:description", "twitter:description", "description"]),
    }
}

/// An attribute's value in a tag, quoted with " or '.
fn attr(tag: &str, name: &str) -> Option<String> {
    let low = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = low[from..].find(name) {
        let at = from + i;
        let before_ok = at == 0 || low.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = low[at + name.len()..].trim_start();
        if before_ok && rest.starts_with('=') {
            let off = tag.len() - rest.len() + 1;
            let v = tag[off..].trim_start();
            let q = v.chars().next()?;
            if q == '"' || q == '\'' {
                return v[1..].find(q).map(|e| v[1..1 + e].to_string());
            }
            return Some(v.split_whitespace().next().unwrap_or("").trim_end_matches('/').to_string());
        }
        from = at + name.len();
    }
    None
}

/// Entities decoded and whitespace folded.
fn clean(s: &str) -> String {
    let s = s.replace("&amp;", "&").replace("&quot;", "\"").replace("&#39;", "'").replace("&#x27;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&nbsp;", " ");
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Catches: picking the bare `<title>` over the page's own share title, or losing the
    /// description to attribute order or quoting.
    #[test]
    fn open_graph_wins_over_the_title_tag() {
        let html = r#"<html><head><title>Repo · GitHub</title>
            <meta content='Rust UI for agents &amp; streams' property="og:description">
            <meta property="og:title" content="chamsco/Workbench">
            <meta property="og:site_name" content="GitHub"></head><body>x</body></html>"#;
        let u = parse(html);
        assert_eq!(u.title.as_deref(), Some("chamsco/Workbench"));
        assert_eq!(u.description.as_deref(), Some("Rust UI for agents & streams"));
        assert_eq!(u.site, "GitHub");
        assert_eq!(parse("<title>\n  Plain  page </title>").title.as_deref(), Some("Plain page"));
        assert_eq!(host("https://www.example.com:443/a?b"), "example.com:443");
    }
}
