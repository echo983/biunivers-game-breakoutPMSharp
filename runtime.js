import init, {
  render_backend,
  hosting_mode,
  setup,
  setup_gpu,
  configure,
  resize,
  step,
  render,
  key,
  pointer_down,
  pointer_up,
  pointer_move,
  wheel,
  set_paused,
  destroy,
} from "./game.js";

const canvas = document.querySelector("#canvas");

let ctx2d = null;
let width = 0; // CSS 像素
let height = 0;

function applySize() {
  const rect = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  width = Math.max(1, Math.round(rect.width));
  height = Math.max(1, Math.round(rect.height));
  canvas.width = Math.round(width * dpr);
  canvas.height = Math.round(height * dpr);
  if (ctx2d) {
    ctx2d.setTransform(dpr, 0, 0, dpr, 0, 0);
  }
}

function localPoint(event) {
  const rect = canvas.getBoundingClientRect();
  return {
    x: event.clientX - rect.left,
    y: event.clientY - rect.top,
    buttons: event.buttons ?? 0,
  };
}

async function loadConfig() {
  try {
    const response = await fetch("./.biunivers/config.json", { cache: "no-store" });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return await response.json();
  } catch {
    return {};
  }
}

// 内容无关的运行时外壳：加载 WASM、协商后端、转发输入、驱动帧循环。
const query = new URLSearchParams(window.location.search);
const locale = query.get("biunivers_locale") ?? "zh-CN";
const theme = query.get("biunivers_theme") ?? "system";
document.documentElement.lang = locale;
document.documentElement.dataset.theme = theme;

await init();

applySize();

// 能力协商：WebGPU 优先，失败回退 2D。
const backend = render_backend();
const webgpuAvailable = typeof navigator !== "undefined" && !!navigator.gpu;
let gpu = false;
if (backend === 1 && webgpuAvailable) {
  gpu = await setup_gpu(canvas, width, height);
}
if (!gpu) {
  ctx2d = canvas.getContext("2d");
  const dpr = window.devicePixelRatio || 1;
  ctx2d.setTransform(dpr, 0, 0, dpr, 0, 0);
  setup(ctx2d, width, height);
}

const config = await loadConfig();
configure(JSON.stringify(config));

const NAV_KEYS = new Set([
  "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
  "Space", "Tab", "PageUp", "PageDown", "Home", "End",
]);

window.addEventListener("keydown", (event) => {
  if (NAV_KEYS.has(event.code)) event.preventDefault();
  key(event.code, true);
});
window.addEventListener("keyup", (event) => {
  if (NAV_KEYS.has(event.code)) event.preventDefault();
  key(event.code, false);
});

canvas.addEventListener("pointerdown", (event) => {
  event.preventDefault();
  try {
    canvas.setPointerCapture(event.pointerId);
  } catch {
    // 某些环境不支持 pointer capture，忽略即可。
  }
  const p = localPoint(event);
  pointer_down(p.x, p.y, p.buttons);
});
canvas.addEventListener("pointermove", (event) => {
  const p = localPoint(event);
  pointer_move(p.x, p.y, p.buttons);
});
canvas.addEventListener("pointerup", (event) => {
  const p = localPoint(event);
  pointer_up(p.x, p.y, p.buttons);
});
canvas.addEventListener("pointercancel", (event) => {
  const p = localPoint(event);
  pointer_up(p.x, p.y, p.buttons);
});
canvas.addEventListener("wheel", (event) => {
  event.preventDefault();
  wheel(event.deltaX, event.deltaY);
}, { passive: false });

document.addEventListener("visibilitychange", () => {
  set_paused(document.hidden);
});
window.addEventListener("pagehide", () => {
  destroy();
});

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
