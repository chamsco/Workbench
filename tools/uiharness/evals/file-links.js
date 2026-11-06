// tools/uiharness/evals/file-links.js
//
// A symlink in a file listing (FileEntry.link) must read as one, in both
// places the UI lists files: the right panel's Files tab and the Worktree
// files canvas. The core marks a link and never enters one that leaves the
// project, so the UI's job is to show which rows are links at a glance: a
// link marker after the name, an italic name, a title and an accessible name
// that say "symbolic link". Ordinary rows must not change.
//
// Run: node tools/uiharness/run.mjs --eval-file tools/uiharness/evals/file-links.js
//        [--shot out.png]
// Failures are written with console.error, so the run exits non-zero.
(async () => {
  const agent = { id: 0, role: "main", kind: "main", parent: null, depth: 0, key: "main", title: "Main", brief: "", depends_on: [], status: "idle", decision: null, log: [], cost_usd: 0, input_tokens: 0, output_tokens: 0, deliverable: null, budget_usd: null, ticket: null, worktree: "/tmp/ws", branch: "main", escalations: [] };
  window.__H.mock("snapshot", () => ({ name: "demo", workspace: "/tmp/ws", agents: [agent], approvals: [], tickets: [], artifacts: [], options: [], canvas_ops: [], board: [], total_cost_usd: 0, router_cost_usd: 0, config_source: "harness", link: { state: "off" }, index: 0 }));
  // What files::scan returns: ordinary rows, a link to a folder inside the
  // project (its contents are listed), a link to a file, and a link that
  // leaves the project (listed, never entered).
  const rows = [
    { depth: 0, name: "src", path: "/tmp/ws/src", dir: true, link: false },
    { depth: 1, name: "main.rs", path: "/tmp/ws/src/main.rs", dir: false, link: false },
    { depth: 0, name: "shared", path: "/tmp/ws/shared", dir: true, link: true },
    { depth: 1, name: "util.rs", path: "/tmp/ws/shared/util.rs", dir: false, link: false },
    { depth: 0, name: "CHANGELOG.md", path: "/tmp/ws/CHANGELOG.md", dir: false, link: true },
    { depth: 0, name: "outside-notes", path: "/tmp/ws/outside-notes", dir: false, link: true },
    { depth: 0, name: "README.md", path: "/tmp/ws/README.md", dir: false },
  ];
  window.__H.mock("list_files", () => rows);
  // One tab with a conversation and the worktree files canvas side by side.
  window.__H.mock("tabs_for", () => ({ tabs: [{ name: "Bench", panes: [{ kind: "agent", agent: 0, url: null }, { kind: "files", agent: 0, url: null }], layout: null }], active: 0 }));
  window.__H.fire("state", {});
  await window.__H.frames(4);
  await Code.loadTabs("/tmp/ws");
  setMode("code");
  RPanel.show("files");
  Code.setWb("canvases");
  await window.__H.frames(4);
  await new Promise((r) => setTimeout(r, 800));

  const failures = [];
  const check = (ok, what) => { if (!ok) failures.push(what); };
  const panes = {
    "right panel": { rows: [...document.querySelectorAll("#rpanel .rp-file[data-path]")], name: (b) => b.querySelector("span"), cls: "rp-link" },
    "worktree canvas": { rows: [...document.querySelectorAll(".ftree .ft[data-path]")], name: (b) => b, cls: "ft-link" },
  };
  const report = {};
  for (const [pane, p] of Object.entries(panes)) {
    check(p.rows.length === rows.length, `${pane}: ${p.rows.length} rows listed, ${rows.length} expected`);
    report[pane] = p.rows.map((b) => {
      const name = b.dataset.path.split("/").pop();
      const src = rows.find((r) => r.path === b.dataset.path) || {};
      const marker = b.querySelector(".lnk");
      const style = getComputedStyle(p.name(b));
      const row = { name, link: !!src.link, class: b.className, marker: !!marker, title: b.title, aria: b.getAttribute("aria-label"), fontStyle: style.fontStyle };
      if (src.link) {
        check(b.classList.contains(p.cls), `${pane}: ${name} lacks .${p.cls}`);
        check(!!marker && marker.getBoundingClientRect().width > 0, `${pane}: ${name} has no visible link marker`);
        check(/symbolic link/i.test(b.title), `${pane}: ${name} title does not say symbolic link (${b.title})`);
        check(/symbolic link/i.test(b.getAttribute("aria-label") || ""), `${pane}: ${name} has no accessible "symbolic link" (${b.getAttribute("aria-label")})`);
        check(style.fontStyle === "italic", `${pane}: ${name} is not set apart in italics (${style.fontStyle})`);
      } else {
        check(!b.classList.contains(p.cls) && !marker && !b.title && style.fontStyle !== "italic", `${pane}: ordinary row ${name} changed`);
      }
      return row;
    });
  }
  // A link still opens as the thing it points at: folder links keep their folder icon and indent.
  const shared = document.querySelector('#rpanel .rp-file[data-path="/tmp/ws/shared"]');
  check(shared && shared.dataset.dir === "true", "a link to a folder inside the project is still a folder row");
  const out = document.querySelector('#rpanel .rp-file[data-path="/tmp/ws/outside-notes"]');
  check(out && out.dataset.dir === "false", "a link that leaves the project is not offered as a folder");

  if (failures.length) console.error("FAIL: " + failures.join(" | "));
  return { pass: failures.length === 0, failures, report };
})()
