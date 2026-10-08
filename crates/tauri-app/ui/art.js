// Paintings in a halftone grain, and the window's backdrop.
//
// Art.band(el, src, opts) draws a picture into `el` with an ordered (Bayer)
// dither of its lightness, strongest on edges and in gradients, faint in
// flat areas, so it reads as the painting first and the dots second. Toward
// its foot it dissolves dot by dot into whatever is behind it. It renders
// once per size, on a small canvas scaled up with hard pixels, and runs
// nothing in between. Ported from Berth's DitherBand (MIT, Sean Brydon;
// see THIRD_PARTY_NOTICES.md), as are the paintings in art/.
//
// Art.apply(id) sets the backdrop from Settings → Appearance: a painting
// muted behind the whole window, the old aurora, or nothing.

var Art = (() => {
  const BAYER8 = [0, 32, 8, 40, 2, 34, 10, 42, 48, 16, 56, 24, 50, 18, 58, 26, 12, 44, 4, 36, 14, 46, 6, 38, 60, 28, 52, 20, 62, 30, 54, 22, 3, 35, 11, 43, 1, 33, 9, 41, 51, 19, 59, 27, 49, 17, 57, 25, 15, 47, 7, 39, 13, 45, 5, 37, 63, 31, 55, 23, 61, 29, 53, 21].map(v => (v + 0.5) / 64);

  // The backdrops Settings offers. The harbour follows the time of day in a
  // light theme and is always the night in a dark one.
  const BACKDROPS = [
    ["harbour", "Harbour"], ["dawn", "Lighthouse at dawn"], ["night", "Moorings at night"],
    ["open-sea", "Open sea"], ["fog", "Fog"], ["aurora", "Aurora"], ["plain", "Plain"],
  ];
  const dark = () => {
    const t = document.documentElement.dataset.theme;
    return t ? t === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
  };
  function light() {
    if (dark()) return "night";
    const h = new Date().getHours();
    return h >= 5 && h < 8 ? "dawn" : h >= 8 && h < 17 ? "day" : "dusk";
  }
  // The picture for a backdrop id (null: not a painting).
  function src(id) {
    if (id === "harbour") return `art/harbour-${light()}.webp`;
    return ["dawn", "night", "open-sea", "fog"].includes(id) ? `art/${id}.webp` : null;
  }
  // How far each is muted toward the page: the night most, so its stars stay calm.
  const muteFor = s => (/night/.test(s) ? 0.3 : 0.07);

  function bgOf(el, cssVar) {
    const probe = document.createElement("span");
    probe.style.color = `var(${cssVar})`;
    el.appendChild(probe);
    const m = (getComputedStyle(probe).color.match(/[\d.]+/g) || [0, 0, 0]).map(Number);
    probe.remove();
    return m[0] <= 1 && m[1] <= 1 && m[2] <= 1 ? [m[0] * 255, m[1] * 255, m[2] * 255] : m.slice(0, 3);
  }

  function dither(frame, levels, fade, mute, bg) {
    const { data, width: w, height: h } = frame;
    const L = levels - 1, from = h * (1 - fade), lum = new Float32Array(w * h);
    for (let i = 0, p = 0; p < w * h; i += 4, p++) {
      if (mute) for (let c = 0; c < 3; c++) data[i + c] += (bg[c] - data[i + c]) * mute;
      lum[p] = 0.299 * data[i] + 0.587 * data[i + 1] + 0.114 * data[i + 2];
    }
    for (let y = 0; y < h; y++) {
      let keep = 1;
      if (y > from) { const t = 1 - (y - from) / (h - from); keep = t * t * (3 - 2 * t); }
      const row = (y & 7) * 8, rowB = ((y + 3) & 7) * 8;
      const up = Math.max(0, y - 1) * w, down = Math.min(h - 1, y + 1) * w;
      for (let x = 0; x < w; x++) {
        const p = y * w + x, i = p * 4;
        if (BAYER8[rowB + ((x + 5) & 7)] >= keep) { data[i + 3] = 0; continue; }
        const Y = lum[p];
        const edge = Math.abs(lum[y * w + Math.min(w - 1, x + 1)] - lum[y * w + Math.max(0, x - 1)]) + Math.abs(lum[down + x] - lum[up + x]);
        const amount = Math.min(1, 0.35 + edge / 40);
        const d = ((Math.floor((Y / 255) * L + BAYER8[row + (x & 7)]) / L) * 255 - Y) * amount;
        data[i] += d; data[i + 1] += d; data[i + 2] += d; data[i + 3] = 255;
      }
    }
  }

  // Draw `src` into `el`, covering it. Returns a function that stops it.
  function band(el, src, { position = 0.5, cell = 2, levels = 6, fade = 0.4, mute = 0, bg = "--desk-base" } = {}) {
    el.innerHTML = "";
    const cv = document.createElement("canvas");
    cv.className = "dither";
    el.appendChild(cv);
    let size = "", alive = true, timer = 0;
    const draw = async () => {
      if (document.hidden) return;
      const w = Math.ceil(el.clientWidth / cell), h = Math.ceil(el.clientHeight / cell);
      if (!w || !h || `${w}x${h}:${src}` === size) return;
      const img = new Image();
      img.src = src;
      try { await img.decode(); } catch { return; }
      if (!alive) return;
      size = `${w}x${h}:${src}`;
      cv.width = w; cv.height = h;
      cv.style.width = w * cell + "px"; cv.style.height = h * cell + "px";
      const ctx = cv.getContext("2d", { willReadFrequently: true });
      const scale = Math.max(w / img.naturalWidth, h / img.naturalHeight);
      const dw = img.naturalWidth * scale, dh = img.naturalHeight * scale;
      // Drawn small, then up: the brushwork softens, so the grain is the dither's.
      const soft = document.createElement("canvas");
      soft.width = Math.max(1, Math.round(w * 0.6)); soft.height = Math.max(1, Math.round(h * 0.6));
      const s = soft.getContext("2d");
      s.imageSmoothingQuality = "high";
      s.drawImage(img, ((w - dw) / 2) * 0.6, (h - dh) * position * 0.6, dw * 0.6, dh * 0.6);
      ctx.clearRect(0, 0, w, h);
      ctx.drawImage(soft, 0, 0, w, h);
      const frame = ctx.getImageData(0, 0, w, h);
      dither(frame, levels, fade, mute, bgOf(el, bg));
      ctx.putImageData(frame, 0, 0);
      cv.classList.add("on");
    };
    const later = () => { clearTimeout(timer); timer = setTimeout(draw, 120); };
    const ro = new ResizeObserver(later);
    ro.observe(el);
    const vis = () => { if (!document.hidden) { size = ""; draw(); } };
    document.addEventListener("visibilitychange", vis);
    draw();
    return () => { alive = false; ro.disconnect(); document.removeEventListener("visibilitychange", vis); cv.width = cv.height = 0; };
  }

  let stopDesk = null, current = null, vivid = false;
  // The window's backdrop: one painting behind the title bar, the sidebar
  // and the page, dissolving before the middle of the window. Full strength
  // on Home; elsewhere muted toward the page so panes stay readable.
  function apply(id) {
    current = BACKDROPS.some(([k]) => k === id) ? id : "harbour";
    const win = document.getElementById("win");
    let layer = document.getElementById("winArt");
    if (!layer) {
      layer = document.createElement("div");
      layer.id = "winArt"; layer.className = "win-art"; layer.setAttribute("aria-hidden", "true");
      win.prepend(layer);
    }
    win.dataset.backdrop = current;
    document.querySelector(".desk").dataset.backdrop = current;
    draw();
  }
  function draw() {
    const layer = document.getElementById("winArt");
    if (stopDesk) { stopDesk(); stopDesk = null; }
    const s = src(current);
    if (!layer) return;
    if (s) stopDesk = band(layer, s, { position: 0.4, fade: 0.5, mute: vivid ? muteFor(s) : 0.6 + muteFor(s) / 2, cell: 2, bg: "--win-bg" });
    else layer.innerHTML = "";
  }
  // Home shows the painting at full strength.
  function setVivid(v) {
    if (v === vivid) return;
    vivid = v;
    document.getElementById("win").classList.toggle("art-vivid", v);
    if (current) draw();
  }
  // A theme change swaps the harbour's light.
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => current && apply(current));

  return { band, apply, setVivid, src, muteFor, BACKDROPS, current: () => current, light };
})();
