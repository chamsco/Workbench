// Home: the landing page, a dashboard of cards. Today's date, then what the
// last 30 days of work looked like (prompts per day by who answered them,
// sessions, active hours, the busiest weekday, who wrote the prompts), what is
// left on each CLI login's plan, the threads you were last in, and the
// projects on this machine. Everything is read from the app's own stores;
// nothing is estimated, and a card with nothing to say says so.
var Dash = (() => {
  const DAYS = 30, DAY = 86400000;
  // Fixed order, never cycled (validated for the dark and light surfaces);
  // a fifth provider and beyond fold into Other.
  const HUES = ["#c96a4a", "#3b82f6", "#2a9a72", "#8b6ff0"];
  const NAMES = { claude: "Claude Code", codex: "Codex", gemini: "Gemini CLI", ollama: "Ollama", cloud: "Backspace Cloud", lmstudio: "LM Studio", brainpowr: "Brainpowr" };
  const nameOf = p => NAMES[p] || (p ? p.charAt(0).toUpperCase() + p.slice(1) : "Other");
  let data = null, at = 0, busy = false;

  const dayKey = ms => { const d = new Date(ms); d.setHours(0, 0, 0, 0); return d.getTime(); };
  const hm = ms => new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  const ago = ms => {
    const m = Math.round((Date.now() - ms) / 60000);
    return m < 1 ? "now" : m < 60 ? `${m}m` : m < 1440 ? `${Math.round(m / 60)}h` : `${Math.round(m / 1440)}d`;
  };
  const list = xs => xs.length < 2 ? xs.join("") : xs.slice(0, -1).join(", ") + " and " + xs[xs.length - 1];

  async function load() {
    const today = dayKey(Date.now()), since = today - (DAYS - 1) * DAY;
    const [act, threads, accts] = await Promise.all([
      invoke("chat_activity", { since }).catch(() => null),
      invoke("chat_list").catch(() => null),
      invoke("accounts").then(d => d.accounts || []).catch(() => []),
    ]);
    const a = act || { prompts: [], hours: [], sessions: 0 };
    const mine = a.prompts.filter(p => !p.agent);
    // Providers by how much you used them; the top four keep a hue each.
    const tally = {};
    for (const p of mine) tally[p.provider] = (tally[p.provider] || 0) + 1;
    const ranked = Object.keys(tally).sort((x, y) => tally[y] - tally[x]);
    const series = ranked.slice(0, HUES.length);
    const key = p => (series.includes(p) ? p : "other");
    const days = Array.from({ length: DAYS }, (_, i) => ({ at: since + i * DAY, n: {} }));
    for (const p of mine) {
      const d = days[Math.round((dayKey(p.at) - since) / DAY)];
      if (d) d.n[key(p.provider)] = (d.n[key(p.provider)] || 0) + 1;
    }
    const week = [0, 0, 0, 0, 0, 0, 0];
    for (const p of mine) week[new Date(p.at).getDay()]++;
    const busiest = mine.length ? week.indexOf(Math.max(...week)) : -1;
    data = {
      since, days, series: series.concat(ranked.length > series.length ? ["other"] : []),
      prompts: mine.length, agents: a.prompts.length - mine.length, sessions: a.sessions, hours: a.hours.length,
      busiest: busiest < 0 ? "—" : new Date(2024, 0, 7 + busiest).toLocaleDateString([], { weekday: "long" }),
      threads: (threads || []).filter(t => !t.ghost && !t.app).slice(0, 5),
      accounts: accts.filter(x => x.enabled),
    };
    at = Date.now();
  }

  // ---------------------------------------------------------------- cards
  function head() {
    const d = new Date();
    const from = data.series.filter(s => s !== "other").map(nameOf);
    return `<div class="dh">
      <h1>${esc(d.toLocaleDateString([], { weekday: "long", month: "long", day: "numeric" }))}</h1>
      <p>${from.length ? `From ${esc(list(from))} · ` : ""}updated ${esc(hm(at))}</p>
    </div>`;
  }

  function month() {
    const max = Math.max(1, ...data.days.map(d => Object.values(d.n).reduce((a, b) => a + b, 0)));
    const W = 720, H = 112, gap = 4, bw = (W - gap * (DAYS - 1)) / DAYS;
    const hue = s => (s === "other" ? "var(--fg-4)" : HUES[data.series.indexOf(s)]);
    // Stacked from the baseline, a 2px surface gap between segments; only the
    // top segment gets the rounded end.
    const bars = data.days.map((d, i) => {
      const x = i * (bw + gap);
      let y = H, out = "";
      const segs = data.series.filter(s => d.n[s]);
      segs.forEach((s, j) => {
        const h = Math.max(2, (d.n[s] / max) * H);
        y -= h;
        const top = j === segs.length - 1;
        out += `<rect x="${x.toFixed(1)}" y="${y.toFixed(1)}" width="${bw.toFixed(1)}" height="${(h - (top ? 0 : 2)).toFixed(1)}" rx="${top ? 3 : 0}" fill="${hue(s)}"/>`;
      });
      return `<g class="db" data-d="${i}">${out}<rect class="hit" x="${(x - gap / 2).toFixed(1)}" y="0" width="${(bw + gap).toFixed(1)}" height="${H}"/></g>`;
    }).join("");
    const ticks = data.days.map((d, i) => ((i % 7 === 0 && DAYS - 1 - i >= 4) || i === DAYS - 1) ? `<span style="left:${((i * (bw + gap) + bw / 2) / W * 100).toFixed(2)}%">${esc(new Date(d.at).toLocaleDateString([], { month: "short", day: "numeric" }))}</span>` : "").join("");
    const all = data.prompts + data.agents;
    const you = all ? Math.round((data.prompts / all) * 100) : 0;
    const stat = (v, l) => `<div class="ds"><b>${esc(v)}</b><span>${esc(l)}</span></div>`;
    return `<section class="dc wide">
      <h3>Last ${DAYS} days</h3>
      <div class="dstats">${stat(data.prompts.toLocaleString(), "prompts you typed")}${stat(data.sessions.toLocaleString(), "sessions")}${stat(data.hours.toLocaleString() + " h", "active")}${stat(data.busiest, "busiest day")}</div>
      <div class="dsub"><span>Prompts you typed per day</span><span class="sp"></span>${data.series.map(s => `<span class="dl"><i style="background:${hue(s)}"></i>${esc(s === "other" ? "Other" : nameOf(s))}</span>`).join("")}</div>
      ${data.prompts ? `<div class="dchart"><svg viewBox="0 0 ${W} ${H}" preserveAspectRatio="none" role="img" aria-label="Prompts you typed per day, last ${DAYS} days">${bars}</svg><div class="dtip" hidden></div></div><div class="dticks">${ticks}</div>`
        : `<div class="dempty">Nothing typed in the last ${DAYS} days. Your prompts show here by day and by who answered them.</div>`}
      <div class="dwho"><span>Who wrote the prompts</span><span class="dbar">${all ? `<i class="you" style="width:${you}%"></i><i class="ag" style="width:${100 - you}%"></i>` : ""}</span>
        <span class="dl"><i class="you"></i>You ${all ? you : 0}%</span><span class="dl"><i class="ag"></i>Agents ${all ? 100 - you : 0}%</span></div>
    </section>`;
  }

  function plans() {
    const fmtReset = ms => { const d = new Date(ms); return d.toLocaleDateString([], { weekday: "short" }) + " " + hm(ms); };
    const rows = data.accounts.map(a => {
      const u = a.usage, pct = u && u.limit ? Math.min(100, Math.round(((u.used || 0) / u.limit) * 100)) : null;
      const cool = a.health && a.health.cooldown_until > Date.now();
      return `<div class="dp">
        <div class="dp-h"><b>${esc(nameOf(a.provider))}</b><span>${esc(a.name)}</span></div>
        <div class="dp-r"><span class="dp-l">${cool ? "Limit reached" : u && u.report ? esc(u.report) : "Usage"}</span>
          <span class="dmeter"><i style="width:${cool ? 100 : pct || 0}%"></i></span>
          <b class="dp-p">${cool ? "100%" : pct != null ? pct + "%" : "—"}</b>
          <span class="dp-t">${cool ? `again ${esc(fmtReset(a.health.cooldown_until))}` : u && u.resets_at ? `resets ${esc(fmtReset(u.resets_at))}` : ""}</span></div>
      </div>`;
    }).join("");
    return `<section class="dc"><h3>Plan usage</h3>${rows || `<div class="dempty">No CLI login has reported a quota yet. Logins and their limits live in <button class="dlink" data-go="accounts">Settings → Accounts</button>.</div>`}</section>`;
  }

  function threads() {
    const rows = data.threads.map(t => `<button class="dt" data-t="${esc(t.id)}"><span class="dt-ic">${icon(t.project ? "codei" : "bubble")}</span><span class="dt-b"><b>${esc(t.title || "Chat")}</b><span>${esc(t.project ? base(t.project) : (window.Chat ? Chat.routeName(t.route) : t.route.provider))}</span></span><span class="dt-w">${esc(ago(t.updated))}</span></button>`).join("");
    return `<section class="dc"><h3>Recent threads</h3>${rows || `<div class="dempty">No threads yet. Ask in the column on the right, or start one in Chat.</div>`}</section>`;
  }

  function projects() {
    const ps = (prefs.projects || []).slice(0, 5), cur = hasProject && snap.workspace;
    const rows = ps.map(p => `<button class="dt" data-p="${esc(p)}"><span class="dt-ic">${icon("folder")}</span><span class="dt-b"><b>${esc(base(p))}</b><span>${esc(p)}</span></span>${p === cur ? `<span class="dt-w on">open</span>` : ""}</button>`).join("");
    return `<section class="dc"><h3>Projects</h3>${rows || `<div class="dempty">No projects yet.</div>`}<button class="dlink add" data-act="open">${icon("plus")}Open a folder…</button></section>`;
  }

  // ---------------------------------------------------------------- page
  async function render() {
    const el = $("#dash"); if (!el || busy) return;
    busy = true;
    try { if (!data || Date.now() - at > 60000) await load(); } finally { busy = false; }
    if (!data) return;
    el.innerHTML = `<div class="dash-in">${head()}${month()}<div class="dgrid">${plans()}${threads()}${projects()}</div></div>`;
    wire(el);
  }
  function wire(el) {
    // A project's thread opens in Code's ADE, where it lives; the rest in Chat.
    $$("[data-t]", el).forEach(b => (b.onclick = () => {
      const t = data.threads.find(x => x.id === b.dataset.t);
      if (t && t.project) { S.codeView = "pair"; setMode("code"); } else setMode("chat");
      if (window.Chat) Chat.open(b.dataset.t);
    }));
    $$("[data-p]", el).forEach(b => (b.onclick = () => { openProject(b.dataset.p); setMode("code"); }));
    $$("[data-go]", el).forEach(b => (b.onclick = () => openSettings(b.dataset.go)));
    const op = $('[data-act="open"]', el); if (op) op.onclick = () => pickProject();
    // One tooltip for the chart: the day, then each series' count.
    const chart = $(".dchart", el); if (!chart) return;
    const tip = $(".dtip", chart);
    $$(".db", chart).forEach(g => {
      g.onmouseenter = () => {
        const d = data.days[+g.dataset.d], tot = Object.values(d.n).reduce((a, b) => a + b, 0);
        tip.innerHTML = `<b>${esc(new Date(d.at).toLocaleDateString([], { weekday: "short", month: "short", day: "numeric" }))}</b>${tot ? data.series.filter(s => d.n[s]).map(s => `<span><i style="background:${s === "other" ? "var(--fg-4)" : HUES[data.series.indexOf(s)]}"></i>${esc(s === "other" ? "Other" : nameOf(s))}<em>${d.n[s]}</em></span>`).join("") : `<span>No prompts</span>`}`;
        tip.hidden = false;
        const r = g.getBoundingClientRect(), box = chart.getBoundingClientRect();
        tip.style.left = Math.min(box.width - tip.offsetWidth, Math.max(0, r.left - box.left + r.width / 2 - tip.offsetWidth / 2)) + "px";
        g.classList.add("on");
      };
      g.onmouseleave = () => { tip.hidden = true; g.classList.remove("on"); };
    });
  }
  // Something changed (a reply landed, a project opened): read again next time.
  function stale() { at = 0; if (S.mode === "home") render(); }

  return { render, stale };
})();
