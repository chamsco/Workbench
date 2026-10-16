// uiharness/server.mjs — the static file server the page is loaded from.
//
// Rooted at crates/tauri-app/ui, read-only: nothing here ever writes into the
// app's directory. The one route that is not a file on disk is
// /__harness/index.html, which is the real index.html with preload.js inlined
// as the first script in <head>.
import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { join, normalize, resolve, extname, sep } from "node:path";

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".htm": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".map": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".gif": "image/gif",
  ".webp": "image/webp",
  ".avif": "image/avif",
  ".ico": "image/x-icon",
  ".woff2": "font/woff2",
  ".woff": "font/woff",
  ".ttf": "font/ttf",
  ".otf": "font/otf",
  ".wasm": "application/wasm",
  ".txt": "text/plain; charset=utf-8",
  ".webmanifest": "application/manifest+json",
  ".xml": "application/xml; charset=utf-8",
};

export const contentType = (file) => TYPES[extname(file).toLowerCase()] || "application/octet-stream";

/** Put `code` in a <script> as the very first thing in <head>. */
export function injectFirstScript(html, code) {
  // </script> inside the code would close the tag early. This is a harness
  // file we control, but escaping "<\/" costs nothing and removes the trap.
  const safe = code.replace(/<\/(script)/gi, "<\\/$1");
  const tag = `<script data-uiharness="preload">\n${safe}\n</script>`;
  // index.html must keep its own <script src="..."> order; the stub only has to
  // come before the first of them. Insert after <head...> rather than before
  // the first <script>, so nothing that sits in <head> ahead of the scripts
  // (charset, viewport) changes position.
  //
  // The <base> is what makes the virtual path work at all: the app is served
  // from the root of the Tauri asset protocol, and its tags are all relative
  // ("style.css", "chat-web/whirl.js"). Without a base they would resolve
  // against /__harness/ and 404. With it, the document behaves exactly as it
  // does at the real root — every subresource is fetched from / as usual.
  const head = /<head[^>]*>/i.exec(html);
  if (head) {
    const at = head.index + head[0].length;
    return html.slice(0, at) + `\n<base href="/">\n` + tag + html.slice(at);
  }
  // No <head> at all: put it in front of the document instead.
  return tag + "\n" + html;
}

/**
 * @param {{root:string, preloadSource:string}} opts
 * @returns {Promise<{port:number, url:string, origin:string, close:()=>Promise<void>}>}
 */
export async function startServer({ root, preloadSource }) {
  const ROOT = resolve(root);
  const indexPath = join(ROOT, "index.html");
  const rawIndex = await readFile(indexPath, "utf8");
  const virtualIndex = injectFirstScript(rawIndex, preloadSource);
  if (!/<script data-uiharness="preload">/.test(virtualIndex)) {
    throw new Error("uiharness: could not inject the preload into index.html");
  }

  const server = createServer(async (req, res) => {
    const send = (status, type, body, extra = {}) => {
      res.writeHead(status, { "content-type": type, "cache-control": "no-store", ...extra });
      if (req.method === "HEAD") res.end();
      else res.end(body);
    };
    try {
      let pathname;
      try {
        pathname = decodeURIComponent(new URL(req.url, "http://127.0.0.1").pathname);
      } catch {
        return send(400, "text/plain; charset=utf-8", "bad url");
      }

      if (pathname === "/__harness/index.html" || pathname === "/__harness/") {
        return send(200, TYPES[".html"], virtualIndex);
      }
      if (pathname === "/__harness/preload.js") {
        return send(200, TYPES[".js"], preloadSource);
      }
      if (pathname === "/favicon.ico") return send(204, "image/x-icon", "");

      const rel = normalize(pathname).replace(/^([/\\])+/, "");
      const file = resolve(ROOT, rel);
      // Containment: the URL must not climb out of the UI root.
      if (file !== ROOT && !file.startsWith(ROOT + sep)) {
        return send(403, "text/plain; charset=utf-8", "outside the UI root");
      }

      let info;
      try {
        info = await stat(file);
      } catch {
        process.stdout.write(`[http] 404 ${pathname}\n`);
        return send(404, "text/plain; charset=utf-8", "not found: " + pathname);
      }
      if (info.isDirectory()) {
        const alt = join(file, "index.html");
        try {
          const body = await readFile(alt);
          return send(200, TYPES[".html"], body);
        } catch {
          return send(404, "text/plain; charset=utf-8", "no index.html in " + pathname);
        }
      }
      const body = await readFile(file);
      return send(200, contentType(file), body, { "content-length": body.length });
    } catch (err) {
      send(500, "text/plain; charset=utf-8", "harness server error: " + (err && err.stack ? err.stack : String(err)));
    }
  });

  server.keepAliveTimeout = 5000;
  await new Promise((ok, no) => {
    server.once("error", no);
    server.listen(0, "127.0.0.1", ok);
  });
  const port = server.address().port;
  const origin = `http://127.0.0.1:${port}`;
  return {
    port,
    origin,
    url: (p = "/__harness/index.html") => origin + (p.startsWith("/") ? p : "/" + p),
    close: () =>
      new Promise((ok) => {
        server.closeAllConnections?.();
        server.close(() => ok());
      }),
  };
}
