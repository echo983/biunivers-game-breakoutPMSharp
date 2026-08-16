import init, {
  render_backend,
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
const unsupported = document.querySelector("#unsupported");

let width = 0; // CSS 像素
let height = 0;

function applySize() {
  const rect = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  width = Math.max(1, Math.round(rect.width));
  height = Math.max(1, Math.round(rect.height));
  canvas.width = Math.round(width * dpr);
  canvas.height = Math.round(height * dpr);
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

function showUnsupported() {
  unsupported.hidden = false;
}

// 内容无关的运行时外壳：加载 WASM、协商 WebGPU、转发输入、驱动帧循环。
const query = new URLSearchParams(window.location.search);
const locale = query.get("biunivers_locale") ?? "zh-CN";
const theme = query.get("biunivers_theme") ?? "system";
document.documentElement.lang = locale;
document.documentElement.dataset.theme = theme;

await init();

applySize();

// WebGPU 预检测：在进入 setup_gpu 前确认 adapter 可用，失败直接提示。
async function webgpuAvailable() {
  try {
    if (typeof navigator === "undefined" || !navigator.gpu) return false;
    const adapter = await navigator.gpu.requestAdapter();
    return !!adapter;
  } catch {
    return false;
  }
}

let gpu = false;
if (render_backend() === 1 && (await webgpuAvailable())) {
  gpu = await setup_gpu(canvas, width, height);
}

if (!gpu) {
  showUnsupported();
} else {
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
}
