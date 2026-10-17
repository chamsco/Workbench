// tools/verify/sidebar-pills.js — the pills over the top of the sidebar: New
// chat on the left (its chevron opens Projects), search + the sidebar toggle in
// one pill on the right, both above Chat | Agents. Folding the sidebar leaves
// New chat where it was and the title bar makes room; unfolding puts it back.
(async () => {
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  await settle(800);
  const win = document.querySelector("#win");
  // The app may boot folded; start from the open card.
  const show = () => document.querySelector('#sidePills button[aria-label="Show sidebar"]');
  if (show()) { show().click(); await settle(700); }
  const slot = document.querySelector("#sidePills");
  const row = slot && slot.firstElementChild;
  const pills = row ? [...row.children] : [];
  const labels = (el) => [...el.querySelectorAll("button")].map((b) => b.getAttribute("aria-label") || b.textContent.trim());
  const box = (el) => el && el.getBoundingClientRect();
  const newChat = () => box(pills[0]);
  const modes = box(document.querySelector("#modesSide"));
  const side = box(document.querySelector("#side"));
  const open = { newChat: newChat(), right: box(pills[1]) };
  const toggle = () => slot.querySelector('button[aria-label$="sidebar"]');
  const openLabel = toggle().getAttribute("aria-label");
  toggle().click();
  await settle(700);
  const closed = { cls: !!show(), newChat: newChat(), right: box(pills[1]), tbarPad: getComputedStyle(document.querySelector(".tbar")).paddingLeft, label: toggle().getAttribute("aria-label"), oldOpenShown: !!document.querySelector("#sideOpen").offsetParent };
  // window.__keepClosed lets a screenshot of the folded state be taken.
  if (!window.__keepClosed) { toggle().click(); await settle(700); }
  return {
    left: labels(pills[0]), right: labels(pills[1]),
    pillsAboveModes: open.newChat.bottom <= modes.top,
    modesFillSide: Math.round(modes.width) >= Math.round(side.width) - 20,
    openLabel,
    closed: closed.cls, closedLabel: closed.label, oldOpenShown: closed.oldOpenShown,
    newChatStayed: Math.abs(open.newChat.left - closed.newChat.left) < 1 && Math.abs(open.newChat.top - closed.newChat.top) < 1,
    rightSlidIn: closed.right.left < open.right.left,
    tbarPad: closed.tbarPad,
    reopened: !show() || !!window.__keepClosed,
  };
})()
