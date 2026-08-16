import init, { setup, resize, step, render } from "./breakout_game.js";

const canvas = document.querySelector("#canvas");
const ctx = canvas.getContext("2d");

let width = 0;
let height = 0;

function applySize() {
  const rect = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  width = Math.max(1, Math.round(rect.width));
  height = Math.max(1, Math.round(rect.height));
  canvas.width = Math.round(width * dpr);
  canvas.height = Math.round(height * dpr);
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
}

const query = new URLSearchParams(window.location.search);
const locale = query.get("biunivers_locale") ?? "zh-CN";
const theme = query.get("biunivers_theme") ?? "system";
document.documentElement.lang = locale;
document.documentElement.dataset.theme = theme;

await init();

applySize();
setup(ctx, width, height);

let lastTime = performance.now();

function frame(now) {
  const dt = Math.min((now - lastTime) / 1000, 0.05);
  lastTime = now;
  step(dt);
  render();
  requestAnimationFrame(frame);
}

window.addEventListener("resize", () => {
  applySize();
  resize(width, height);
});

requestAnimationFrame(frame);
