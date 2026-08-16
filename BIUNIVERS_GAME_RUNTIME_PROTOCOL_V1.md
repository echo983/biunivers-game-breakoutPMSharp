# Biunivers Game Runtime Protocol v1

状态：草案

协议标识：`biunivers.game-runtime/1`

固定文件名：`BIUNIVERS_GAME_RUNTIME_PROTOCOL_V1.md`

规范性质：第三方实现时必须原文复制

## 1. 目标

本协议定义「内容无关的运行时外壳」与「单个 WASM 游戏」之间的固定接口。

一个游戏 = 一个由 Rust（或其他可编译到 WASM 的语言）编译出的 `game_bg.wasm`，
以及 wasm-bindgen 生成的绑定胶水 `game.js`。外壳 `runtime.js` + `index.html`
与游戏内容无关：负责加载 WASM、创建画布、转发输入、驱动帧循环、处理缩放与
DPR，并把 2D 渲染上下文交给游戏。

外壳只提供 2D canvas 渲染上下文。WebGL/WebGPU、音频、手柄等能力不在 v1 范围；
游戏若需要，可在 WASM 内部自行使用浏览器 API，但不属于本协议约定。

## 2. 适配声明

游戏仓库根目录必须包含本文件的完整原文：

```text
BIUNIVERS_GAME_RUNTIME_PROTOCOL_V1.md
```

同时根目录还必须满足 `biunivers.static-app/1` 的要求（见
`BIUNIVERS_APP_PROTOCOL_V1.md`）。

## 3. 文件与命名

```text
/
├── index.html                 外壳入口（内容无关）
├── runtime.js                 外壳运行时（内容无关）
├── game.js                    wasm-bindgen 胶水（构建产物）
├── game_bg.wasm               游戏本体（构建产物）
├── biunivers.app.json
├── BIUNIVERS_APP_PROTOCOL_V1.md
├── BIUNIVERS_GAME_RUNTIME_PROTOCOL_V1.md
└── LICENSE
```

- Rust crate 名固定为 `game`，使 wasm-pack 产物固定为 `game.js` / `game_bg.wasm`。
- 外壳只通过 `import ... from "./game.js"` 加载游戏，不含任何游戏名或游戏逻辑。

## 4. 坐标与单位

- 所有坐标以 CSS 像素计，原点在画布左上角，x 向右、y 向下。
- 外壳负责 devicePixelRatio 缩放（设置 canvas 后备存储尺寸与 2D 变换），游戏不
  关心设备像素。
- 渲染与指针输入共用同一坐标空间。

## 5. 外壳职责

1. 读取 URL 查询参数 `biunivers_locale` / `biunivers_theme`，设置 `<html lang>` 与主题。
2. 获取 `<canvas>` 的 2D 上下文，计算 CSS 像素宽高，按 DPR 设置后备存储与变换。
3. 加载 WASM：`await init()`。
4. 读取公开配置 `./.biunivers/config.json`（缺失时回退 `{}`）。
5. 调用 `setup(ctx, width, height)`，随后调用 `configure(JSON.stringify(config))`。
6. 挂接输入监听（键盘、指针、滚轮）并转发给游戏。
7. 以 requestAnimationFrame 驱动：每帧先 `step(dt)` 再 `render()`；`dt` 以秒计、有上限。
8. 尺寸变化时重新计算并调用 `resize(width, height)`。
9. 页面隐藏/显示时调用 `set_paused(true / false)`。
10. 卸载前调用 `destroy()`。

## 6. 游戏接口（WASM 必须导出的函数）

游戏必须导出下列全部函数；不使用的可空实现（no-op）。

- `setup(ctx: CanvasRenderingContext2d, width: f64, height: f64)`
  初始化；保存上下文与尺寸。
- `configure(config: string)`
  接收外壳透传的公开配置 JSON 字符串，在 `setup` 之后调用一次。
- `resize(width: f64, height: f64)`
  视口尺寸变化时调用。
- `step(dt: f64)`
  推进模拟 `dt` 秒；暂停时应直接返回。
- `render()`
  绘制当前帧。外壳保证调用顺序为 `step` 后 `render`。
- `key(code: string, down: bool)`
  键盘事件；`code` 为 DOM `KeyboardEvent.code`（如 `"ArrowLeft"`、`"Space"`、
  `"KeyA"`）。`down` 为 `true` 表示按下（含自动重复），`false` 表示释放。
- `pointer_down(x: f64, y: f64, buttons: u32)`
- `pointer_up(x: f64, y: f64, buttons: u32)`
- `pointer_move(x: f64, y: f64, buttons: u32)`
  指针（鼠标/触摸）事件；`x`/`y` 为画布内 CSS 坐标；`buttons` 为
  `PointerEvent.buttons` 位掩码。
- `wheel(dx: f64, dy: f64)`
  滚轮事件。
- `set_paused(paused: bool)`
  页面隐藏/显示时调用；游戏可据此暂停 `step`。
- `destroy()`
  卸载前调用；可用于释放资源。

## 7. 输入约定

- 外壳阻止导航键（方向键、空格、Tab、PageUp/Down、Home/End）的默认行为。
- 画布使用 `touch-action: none`，触摸滚动由外壳拦截并转发为 pointer 事件。
- 指针按下时外壳对画布执行 pointer capture。

## 8. 版本化

协议正式发布后正文不再修改。需要改变规范时发布新协议版本与文件名。

## 9. 验证

至少验证：

- `game.js` / `game_bg.wasm` 位于仓库根目录且能通过静态服务器加载；
- 外壳与游戏接口函数名一一对应，无缺失导出；
- 最小/默认/较大尺寸、隐藏/显示、输入转发均正常。