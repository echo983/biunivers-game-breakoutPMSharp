const query = new URLSearchParams(window.location.search);
const locale = query.get("biunivers_locale") ?? "zh-CN";
const theme = query.get("biunivers_theme") ?? "system";

document.documentElement.lang = locale;
document.documentElement.dataset.theme = theme;

const canvas = document.querySelector("#canvas");
const ctx = canvas.getContext("2d");

const BALL_SPEED = 260; // 像素/秒
const BALL_COLOR = "#ffcf5c";

const clamp = (value, min, max) => Math.min(max, Math.max(min, value));

const ball = {
  x: 0,
  y: 0,
  vx: 0,
  vy: 0,
  radius: 10,
};

let width = 0;
let height = 0;

function resize() {
  const rect = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  width = Math.max(1, rect.width);
  height = Math.max(1, rect.height);
  canvas.width = Math.round(width * dpr);
  canvas.height = Math.round(height * dpr);
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  ball.radius = clamp(Math.min(width, height) * 0.02, 6, 14);
  ball.x = clamp(ball.x, ball.radius, width - ball.radius);
  ball.y = clamp(ball.y, ball.radius, height - ball.radius);
}

function resetBall() {
  ball.x = width / 2;
  ball.y = height / 2;
  // 斜向起步，避免接近水平或垂直的单调轨迹
  const angle = (Math.random() * 0.6 + 0.2) * Math.PI;
  ball.vx = Math.cos(angle) * BALL_SPEED;
  ball.vy = Math.sin(angle) * BALL_SPEED;
}

function update(dt) {
  ball.x += ball.vx * dt;
  ball.y += ball.vy * dt;

  if (ball.x - ball.radius < 0) {
    ball.x = ball.radius;
    ball.vx = Math.abs(ball.vx);
  } else if (ball.x + ball.radius > width) {
    ball.x = width - ball.radius;
    ball.vx = -Math.abs(ball.vx);
  }

  if (ball.y - ball.radius < 0) {
    ball.y = ball.radius;
    ball.vy = Math.abs(ball.vy);
  } else if (ball.y + ball.radius > height) {
    ball.y = height - ball.radius;
    ball.vy = -Math.abs(ball.vy);
  }
}

function draw() {
  ctx.clearRect(0, 0, width, height);

  const glow = ctx.createRadialGradient(
    ball.x,
    ball.y,
    ball.radius * 0.5,
    ball.x,
    ball.y,
    ball.radius * 2.5,
  );
  glow.addColorStop(0, "rgba(255, 207, 92, 0.35)");
  glow.addColorStop(1, "rgba(255, 207, 92, 0)");
  ctx.fillStyle = glow;
  ctx.beginPath();
  ctx.arc(ball.x, ball.y, ball.radius * 2.5, 0, Math.PI * 2);
  ctx.fill();

  ctx.fillStyle = BALL_COLOR;
  ctx.beginPath();
  ctx.arc(ball.x, ball.y, ball.radius, 0, Math.PI * 2);
  ctx.fill();
}

let lastTime = performance.now();

function frame(now) {
  const dt = Math.min((now - lastTime) / 1000, 0.05);
  lastTime = now;
  update(dt);
  draw();
  requestAnimationFrame(frame);
}

window.addEventListener("resize", resize);

resize();
resetBall();
requestAnimationFrame(frame);