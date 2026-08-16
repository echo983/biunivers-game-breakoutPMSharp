const query = new URLSearchParams(window.location.search);
const locale = query.get("biunivers_locale") ?? "zh-CN";
const theme = query.get("biunivers_theme") ?? "system";

async function loadConfig() {
  try {
    const response = await fetch("./.biunivers/config.json", {
      cache: "no-store",
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }

    return await response.json();
  } catch {
    return {};
  }
}

const config = await loadConfig();
document.documentElement.lang = locale;
document.documentElement.dataset.theme = theme;

const canvas = document.querySelector("#canvas");
const ctx = canvas.getContext("2d");

function resize() {
  const rect = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(rect.width * dpr);
  canvas.height = Math.round(rect.height * dpr);
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  draw();
}

function draw() {
  const width = canvas.clientWidth;
  const height = canvas.clientHeight;

  ctx.clearRect(0, 0, width, height);

  ctx.textAlign = "center";
  ctx.textBaseline = "middle";

  ctx.fillStyle = "#e2e8f0";
  ctx.font = `600 ${Math.min(28, width / 16)}px system-ui, sans-serif`;
  ctx.fillText("Biunivers Breakout", width / 2, height / 2 - 12);

  ctx.fillStyle = "#8b98ab";
  ctx.font = `14px system-ui, sans-serif`;
  ctx.fillText("初始化完成，等待游戏逻辑", width / 2, height / 2 + 18);
}

window.addEventListener("resize", resize);
resize();