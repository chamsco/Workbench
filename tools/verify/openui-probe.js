// tools/verify/openui-probe.js — dump what the reply actually contains.
(async () => {
  const H = window.__H;
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  const route = { kind: "cli", provider: "claude", model: null };
  const msg = (id, text) => ({ id, role: "assistant", text, at: Date.now(), status: "done", error: null, reply_to: null, reactions: [], attachments: [], model: "claude" });
  const SRC = [
    "```openui",
    'root = Card([CardHeader("Untangle that task", "Pick one thing"), TextContent("Hello there")])',
    "```",
  ].join("\n");

  H.mock("chat_list", () => [{ id: "t1", title: "T", updated: Date.now(), pinned: false, route, preview: "", busy: false, unread: false }]);
  H.mock("agents_list", () => []);
  H.mock("chat_thread", () => ({ id: "t1", title: "T", created: 1, updated: 1, pinned: false, route, messages: [msg("m1", SRC)], life: "keep", members: [] }));

  window.WhirlChat.open("t1");
  await settle(1200);

  const chat = document.querySelector("#chat");
  const blocks = [...document.querySelectorAll(".openui-block")];
  return {
    chatHidden: chat ? chat.hidden : null,
    chatDisplay: chat ? getComputedStyle(chat).display : null,
    chatZoneHidden: document.querySelector("#canvas") ? document.querySelector("#canvas").hidden : null,
    blocks: blocks.length,
    blockHtml: blocks.map((b) => b.outerHTML.replace(/\s+/g, " ").slice(0, 420)),
    failed: [...document.querySelectorAll(".openui-failed")].map((e) => e.textContent.replace(/\s+/g, " ").trim()),
    replyText: chat ? chat.innerText.replace(/\s+/g, " ").trim().slice(0, 200) : null,
    htmlHasCard: chat ? /openui-card|openui-text|Untangle/.test(chat.innerHTML) : null,
  };
})()
