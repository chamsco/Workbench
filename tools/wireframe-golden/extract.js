// Regenerates crates/chrome/tests/golden/wireframe.json from the approved Tight tiles board,
// design/cauldron-tight-tiles/index.html. Run after the board changes, then review the diff: a
// changed number is a design change.
//   NODE_PATH=$(npm root -g) node tools/wireframe-golden/extract.js
// PW_CHROME=<path to Chrome> uses an installed Chrome instead of Playwright's own.
const { chromium } = require(process.env.PW_MODULE || 'playwright');
const fs = require('fs');
const path = require('path');
const root = path.resolve(__dirname, '..', '..');
const page_url = 'file://' + path.join(root, 'design', 'cauldron-tight-tiles', 'index.html');
const OUT = path.join(root, 'crates', 'chrome', 'tests', 'golden', 'wireframe.json');

// The board's frames that show each shell state, by their index among the page's figures. The
// extractor checks each frame's platform and panes, so a reordered board fails loudly.
const STATES = {
  win_all_open: { fig: 2, platform: 'windows', sidebar: true, support: true, dock: true },
  win_sidebar_open: { fig: 0, platform: 'windows', sidebar: true, support: false, dock: false },
  win_folded: { fig: 15, platform: 'windows', sidebar: false, support: false, dock: false },
  mac_folded: { fig: 21, platform: 'mac', sidebar: false, support: false, dock: false },
  mac_support_open: { fig: 24, platform: 'mac', sidebar: false, support: true, dock: false },
  mac_sidebar_open: { fig: 25, platform: 'mac', sidebar: true, support: false, dock: false },
};
// The board draws every frame at 1200x760; the second size checks the solver is not tuned to one.
const SIZES = [[1200, 760], [1440, 900]];

(async () => {
  const b = await chromium.launch(process.env.PW_CHROME ? { executablePath: process.env.PW_CHROME } : {});
  const out = { source: 'design/cauldron-tight-tiles/index.html', unit: 'css px from the window top-left', sizes: [] };
  for (const [ww, wh] of SIZES) {
    const p = await b.newPage({ viewport: { width: 1600, height: 1000 }, colorScheme: 'dark' });
    await p.goto(page_url);
    await p.addStyleTag({ content: `.fit>.win{transform:none!important;width:${ww}px!important;height:${wh}px!important}.fit{overflow:visible!important;height:auto!important}` });
    await p.waitForTimeout(500);
    const states = {};
    for (const [name, want] of Object.entries(STATES)) {
      const got = await p.evaluate(({ fig }) => {
        const w = document.querySelectorAll('figure')[fig].querySelector('.win');
        const wr = w.getBoundingClientRect();
        const rr = e => { if (!e) return null; const q = e.getBoundingClientRect(); if (q.width <= 0) return null; return { x: q.left - wr.left, y: q.top - wr.top, w: q.width, h: q.height }; };
        const sb = w.querySelector('.sb');
        const cards = [...w.querySelectorAll('.cols .card')];
        const main = cards.find(c => !c.classList.contains('sup') && !c.classList.contains('dock'));
        const ch = main.querySelector('.ch');
        const tab = rr(ch.querySelector('.tab'));
        const oi = rr(ch.querySelector('.oi'));
        const notched = !!w.querySelector('.notch2');
        const brandIbs = sb ? [...sb.querySelectorAll('.brand .ib')] : [];
        const sbtn = sb ? brandIbs[brandIbs.length - 1] : ch.querySelector(':scope > .ib');
        const lights = w.querySelector('.lights');
        const firstIbs = [...ch.querySelectorAll(':scope > .ib')];
        return {
          platform: notched ? 'windows' : lights ? 'mac' : 'unknown',
          rects: {
            sidebar: rr(sb),
            main: rr(main),
            dock: rr(w.querySelector('.cols .card.dock')),
            support: rr(w.querySelector('.cols .card.sup')),
            rail: rr(w.querySelector('.irail')),
            notch: notched ? rr(w.querySelector('.nout')) : null,
            caps: notched ? rr(w.querySelector('.caps')) : null,
            lights: rr(lights),
            sbtn: rr(sbtn),
            newb: !sb && lights ? rr(firstIbs[1]) : null,
            tabs: { x: tab.x, y: tab.y, w: oi.x - 2 - tab.x, h: tab.h },
          },
        };
      }, want);
      if (got.platform !== want.platform) throw new Error(`${name}: figure ${want.fig} is ${got.platform}, want ${want.platform}`);
      for (const [k, on] of [['sidebar', want.sidebar], ['support', want.support], ['dock', want.dock]]) {
        if (!!got.rects[k] !== on) throw new Error(`${name}: figure ${want.fig} ${k} ${got.rects[k] ? 'shown' : 'hidden'}`);
      }
      const rects = {};
      for (const [k, r] of Object.entries(got.rects)) {
        rects[k] = r ? { x: Math.round(r.x * 2) / 2, y: Math.round(r.y * 2) / 2, w: Math.round(r.w * 2) / 2, h: Math.round(r.h * 2) / 2, visible: true } : { visible: false };
      }
      states[name] = { platform: want.platform, sidebar_open: want.sidebar, support_open: want.support, dock_open: want.dock, rects };
    }
    out.sizes.push({ window: { w: ww, h: wh }, states });
    await p.close();
  }
  fs.mkdirSync(path.dirname(OUT), { recursive: true });
  fs.writeFileSync(OUT, JSON.stringify(out, null, 1) + '\n');
  console.log('wrote', OUT);
  await b.close();
})();
