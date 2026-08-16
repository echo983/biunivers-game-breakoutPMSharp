# Biunivers Breakout

经典打砖块（Breakout）游戏，作为 Biunivers 静态桌面应用运行。当前阶段实现了基于真实 2D
物理引擎（Rapier2d）的小球：自由落体 → 弹跳 → 滚动 → 停止。

- 协议：`biunivers.static-app/1` + `biunivers.game-runtime/1`
- 入口：`index.html`（仓库根目录）
- 交付物：预构建的 `game_bg.wasm` + 胶水 `game.js`（已提交，Biunivers 不做构建）
- 无后端、无 secret

## 架构

本项目遵循 **Biunivers Game Runtime Protocol v1**（见
`BIUNIVERS_GAME_RUNTIME_PROTOCOL_V1.md`）：外壳与游戏内容分离。

- 游戏本体：Rust（`src/lib.rs`）编译为单个 WASM 文件 `game_bg.wasm`，内含 Rapier2d
  物理世界与渲染逻辑；通过 `web-sys` 直接调用 canvas API。
- 运行时外壳：`runtime.js` + `index.html`，内容无关——只负责加载 WASM、把 2D 上下文
  交给游戏、转发键盘/指针/滚轮输入、跑 `requestAnimationFrame` 循环、处理缩放/DPR/
  可见性/配置透传。外壳可复用于其他遵循同一协议的游戏。
- 接口：WASM 导出 `setup/configure/resize/step/render/key/pointer_*/wheel/set_paused/destroy`。

## 本地运行

直接预览（无需构建）：

```bash
python3 -m http.server 8000
# 打开 http://localhost:8000/
```

## 构建（开发者）

需要 Rust、`wasm32-unknown-unknown` 目标与 `wasm-pack`：

```bash
./build.sh
```

脚本会构建并把 `game_bg.wasm` 与 `game.js` 复制到仓库根目录。

## 目录

- `index.html`：应用入口（外壳）
- `runtime.js`：内容无关的运行时外壳
- `game.js`：wasm-bindgen 生成的绑定胶水（构建产物）
- `game_bg.wasm`：游戏本体（构建产物）
- `src/lib.rs`：Rust 游戏源码
- `Cargo.toml` / `Cargo.lock`：Rust 工程
- `build.sh`：构建脚本
- `style.css`：样式
- `icon.svg`：桌面图标
- `biunivers.app.json`：应用清单
- `BIUNIVERS_APP_PROTOCOL_V1.md`：静态应用协议原文（请勿修改）
- `BIUNIVERS_GAME_RUNTIME_PROTOCOL_V1.md`：游戏运行时协议原文（请勿修改）
- `AGENTS.md`：AI 开发代理约束

## 配置

暂无公开配置项。
