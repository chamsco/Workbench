// tools/verify/sidebar-pills.js — the sidebar's top row: a New chat pill on the
// left (its chevron opens Projects), and search + the sidebar toggle together in
// one pill on the right. The toggle folds the sidebar; the header brings it back.
(async () => {
  const settle = (ms) => new Promise((r) => setTimeout(r, ms));
  await settle(800);
  const side = document.querySelector("#chatList .whirl-side");
  const row = side && side.firstElementChild;
  const pills = row ? [...row.children] : [];
  const labels = (el) => [...el.querySelectorAll("button")].map((b) => b.getAttribute("aria-label") || b.textContent.trim());
  const left = pills[0] ? labels(pills[0]) : [];
  const right = pills[1] ? labels(pills[1]) : [];
  const lr = pills[0] && pills[0].getBoundingClientRect();
  const rr = pills[1] && pills[1].getBoundingClientRect();
  const oldToggleShown = !!document.querySelector("#sideClose")?.offsetParent;
  document.querySelector('#chatList button[aria-label="Hide sidebar"]')?.click();
  await settle(300);
  const closed = document.querySelector("#win").classList.contains("side-closed");
  const reopenShown = !document.querySelector("#sideOpen").hidden;
  document.querySelector("#sideOpen").click();
  await settle(300);
  const reopened = !document.querySelector("#win").classList.contains("side-closed");
  return {
    pills: pills.length, left, right,
    leftOfRight: !!(lr && rr && lr.right < rr.left), sameLine: !!(lr && rr && Math.abs(lr.top - rr.top) < 4),
    oldToggleShown, closed, reopenShown, reopened,
  };
})()
