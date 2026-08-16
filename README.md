# Biunivers Breakout

经典打砖块（Breakout）游戏，作为 Biunivers 静态桌面应用运行。当前阶段基于真实 2D 物理
引擎（Rapier2d）+ WebGPU 3D 渲染（wgpu）：小球自由落体 → 弹跳 → 滚动 → 停止，以 3D 场景呈现。

- 协议：`biunivers.static-app/1` + `biunivers.game-runtime/2`
- 入口：`index.html`（仓库根目录）
- 交付物：预构建的 `game_bg.wasm` + 胶水 `game.js`（已提交，Biunivers 不做构建）
- 无后端、无 secret

## 架构

遵循 **Biunivers Game Runtime Protocol v2**（见
`BIUNIVERS_GAME_RUNTIME_PROTOCOL_V2.md`），外壳与游戏内容分离：

- 游戏本体：Rust（`src/lib.rs`）编译为单个 WASM `game_bg.wasm`，内含 Rapier2d 物理世界；
  渲染用 `wgpu`（WebGPU 后端）画 3D 场景（小球球体 + 地面/墙立方体 + 透视相机 + 光照）。
- 运行时外壳：`runtime.js` + `index.html`，内容无关——加载 WASM、协商渲染后端（WebGPU）、
  转发输入、跑 `requestAnimationFrame` 循环、处理缩放/DPR/可见性/配置；不支持 WebGPU 时
  显示友好提示（不提供 2D 回退）。
- 接口：`render_backend/hosting_mode/setup/setup_gpu/configure/resize/step/render/key/
  pointer_*/wheel/set_paused/destroy`（见 v2 协议）。

## 本地运行

```bash
python3 -m http.server 8000
# 打开 http://localhost:8000/（需支持 WebGPU 的浏览器；不支持时显示提示）
```

## 构建（开发者）

需要 Rust、`wasm32-unknown-unknown` 目标与 `wasm-pack`：

```bash
./build.sh
```

## 目录

- `index.html`：应用入口（外壳）
- `runtime.js`：内容无关的运行时外壳
- `game.js` / `game_bg.wasm`：wasm-bindgen 胶水 + 游戏本体（构建产物）
- `src/lib.rs`：游戏逻辑与物理
- `src/renderer.rs`：wgpu 3D 渲染器
- `src/mesh.rs`：球体/立方体网格生成
- `Cargo.toml` / `Cargo.lock`：Rust 工程
- `build.sh`：构建脚本
- `style.css` / `icon.svg`：样式与图标
- `biunivers.app.json`：应用清单
- `BIUNIVERS_APP_PROTOCOL_V1.md`：静态应用协议原文（请勿修改）
- `BIUNIVERS_GAME_RUNTIME_PROTOCOL_V1.md` / `_V2.md`：游戏运行时协议原文（请勿修改）
- `AGENTS.md`：AI 开发代理约束

## 配置

暂无公开配置项。
