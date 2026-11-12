// Regenerates crates/chrome/tests/golden/wireframe.json from design/cauldron-master-layout.html.
// Run after the wireframe changes, then review the diff: a changed number is a design change.
//   NODE_PATH=$(npm root -g) node tools/wireframe-golden/extract.js
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const root = path.resolve(__dirname, '..', '..');
const page_url = 'file://' + path.join(root, 'design', 'cauldron-master-layout.html');
const OUT = path.join(root, 'crates', 'chrome', 'tests', 'golden', 'wireframe.json');
const SEL = { rail: 'nav.rail', sidebar: '.col', main: '.main', half: '.half', drawer: '.drawer', rrail: '.rrail', notch: '.notch', sbtn: '.sbtn', tabs: '.tabs', topr: '.topr', wcaps: '.wcaps', lights: '.mlights' };
const TOKENS = ['bg', 'frame', 'card', 'raised', 'well', 'line', 'strip', 'fg', 'fg-2', 'fg-3', 'accent', 'on-accent', 'run', 'ok', 'warn', 'bad', 'r', 'r1', 'r2'];
const out = { source: 'design/cauldron-master-layout.html', unit: 'css px, relative to the window top-left (the 1px window border included)', sizes: [], tokens: {} };

(async () => {
  const b = await chromium.launch();
  for (const [ow, oh] of [[0, 0], [1000, 640]]) {
    const p = await b.newPage({ viewport: { width: 1500, height: 950 }, colorScheme: 'dark' });
    await p.goto(page_url);
    if (ow) await p.addStyleTag({ content: `section:nth-of-type(2) .window{width:${ow}px!important;max-width:none!important;height:${oh}px!important;min-height:0!important}` });
    await p.waitForTimeout(300);
    const W = await p.$('section:nth-of-type(2) .window');
    const snap = () => W.evaluate((w, SEL) => {
      const wr = w.getBoundingClientRect();
      const o = { window: { w: Math.round(wr.width), h: Math.round(wr.height) }, rects: {} };
      for (const [k, s] of Object.entries(SEL)) {
        const e = w.querySelector(s); if (!e) continue;
        const cs = getComputedStyle(e), r = e.getBoundingClientRect();
        const vis = cs.visibility !== 'hidden' && cs.display !== 'none' && r.width > 0 && parseFloat(cs.opacity || '1') > 0.01;
        o.rects[k] = { x: Math.round((r.left - wr.left) * 2) / 2, y: Math.round((r.top - wr.top) * 2) / 2, w: Math.round(r.width * 2) / 2, h: Math.round(r.height * 2) / 2, visible: vis };
      }
      return o;
    }, SEL);
    const click = async (fn) => { await W.evaluate(fn); await p.waitForTimeout(450); };
    const states = {};
    states.default = await snap();
    await click(w => w.querySelector('.rrail .rb.on').click());
    states.drawer_closed = await snap();
    await click(w => w.querySelector('.rrail .rb').click());
    await click(w => w.querySelector('button[aria-label=Split]').click());
    states.split_drawer_open = await snap();
    await click(w => w.querySelector('button[aria-label=Split]').click());
    await click(w => w.querySelector('.sbtn').click());
    states.sidebar_hidden = await snap();
    await click(w => w.querySelector('.rrail .rb.on').click());
    states.sidebar_hidden_drawer_closed = await snap();
    out.sizes.push({ window: states.default.window, states });
    await p.close();
  }
  for (const scheme of ['dark', 'light']) {
    const p = await b.newPage({ viewport: { width: 1500, height: 950 }, colorScheme: scheme });
    await p.goto(page_url);
    out.tokens[scheme] = await p.evaluate(names => { const cs = getComputedStyle(document.documentElement); const o = {}; for (const n of names) o[n] = cs.getPropertyValue('--' + n).trim(); return o; }, TOKENS);
    await p.close();
  }
  fs.mkdirSync(path.dirname(OUT), { recursive: true });
  fs.writeFileSync(OUT, JSON.stringify(out, null, 1));
  console.log('wrote', OUT);
  await b.close();
})();
