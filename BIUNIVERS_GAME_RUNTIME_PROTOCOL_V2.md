# Biunivers Game Runtime Protocol v2

状态：草案

协议标识：`biunivers.game-runtime/2`

固定文件名：`BIUNIVERS_GAME_RUNTIME_PROTOCOL_V2.md`

规范性质：第三方实现时必须原文复制

## 1. 目标

v2 在 v1 的基础上增加两类能力：

1. **WebGPU 3D 渲染**——hosted 模式新增 GPU 后端；
2. **自托管（self-managed）模式**——让 Bevy 等完整引擎接管循环、输入与渲染。

v1 的 2D hosted 契约在 v2 中完整保留，因此 v1 游戏在 v2 外壳下可无改动运行。

## 2. 托管模式

外壳在启动时通过能力协商选择托管方式，游戏可用两个可选导出声明：

- `render_backend() -> u32`：渲染后端。
  - `0` = 2D（hosted，缺省）
  - `1` = WebGPU（hosted）
- `hosting_mode() -> u32`：托管方式。
  - `0` = hosted（外壳托管循环/输入，缺省）
  - `1` = self-managed（游戏/引擎自管）

两者均为可选导出；缺失时按缺省值处理。

### hosted 模式（缺省）

外壳拥有画布、帧循环与输入转发，游戏实现 `step` / `render` 等函数。

- 2D：外壳 `canvas.getContext("2d")` → `setup(ctx2d, width, height)`。
- WebGPU：外壳检测到 `navigator.gpu` 后调用 `setup_gpu(canvas, width, height)`
  （异步，返回是否成功）；失败时回退到 2D 路径。

### self-managed 模式

外壳只作为引导器：

1. 加载 WASM、读取 `biunivers_locale` / `biunivers_theme` 与公开配置；
2. 调用一次 `boot(width, height)`；
3. 之后不再驱动循环、不挂输入监听、不创建上下文；游戏/引擎接管一切（自行创建
   canvas 或 surface、跑循环、挂输入监听、渲染与呈现）。

`boot` 可能长期运行（引擎自建循环）；外壳调用后不阻塞、不再干预。

## 3. 文件与命名

```text
/
├── index.html                 外壳入口（内容无关）
├── runtime.js                 外壳运行时（内容无关）
├── game.js                    wasm-bindgen 胶水（构建产物）
├── game_bg.wasm               游戏本体（构建产物）
├── biunivers.app.json
├── BIUNIVERS_APP_PROTOCOL_V1.md
├── BIUNIVERS_GAME_RUNTIME_PROTOCOL_V2.md
└── LICENSE
```

Rust crate 名固定为 `game`，使 wasm-pack 产物固定为 `game.js` / `game_bg.wasm`。

## 4. 坐标与单位

- hosted 模式的 2D 渲染与指针输入：CSS 像素，原点左上，y 向下（同 v1）。
- WebGPU hosted 与 self-managed 模式：由游戏自定（不强制）。

## 5. 游戏接口（WASM 导出的函数）

### hosted 模式（2D 或 WebGPU）

- `render_backend() -> u32`（可选，缺省 0）
- `hosting_mode() -> u32`（可选，缺省 0）
- `setup(ctx2d: CanvasRenderingContext2d, width: f64, height: f64)`（2D hosted）
- `setup_gpu(canvas: HtmlCanvasElement, width: f64, height: f64) -> bool`
  （WebGPU hosted，异步；`true` 表示成功，`false` 表示失败，外壳据此回退 2D）
- `configure(config: string)`
- `resize(width: f64, height: f64)`
- `step(dt: f64)`
- `render()`
- `key(code: string, down: bool)`
- `pointer_down(x: f64, y: f64, buttons: u32)`
- `pointer_up(x: f64, y: f64, buttons: u32)`
- `pointer_move(x: f64, y: f64, buttons: u32)`
- `wheel(dx: f64, dy: f64)`
- `set_paused(paused: bool)`
- `destroy()`

不使用的函数可空实现（no-op）。

### self-managed 模式

- `hosting_mode() -> u32`（返回 1）
- `boot(width: f64, height: f64)`

## 6. WebGPU hosted 约定

- 外壳把 `HtmlCanvasElement` 交给 `setup_gpu`；游戏用 wgpu 创建 surface，自行请求
  adapter/device、配置 surface 格式，并处理 device loss。
- `resize` 时游戏重配 surface（或按当前纹理尺寸呈现）。
- `render()` 中游戏完成 `get_current_texture()` → 渲染 → `present()`。
- 外壳不保证 WebGPU 可用；游戏需保留 2D 回退路径或给出明确错误。

## 7. 输入约定（hosted）

- 外壳阻止导航键（方向键、空格、Tab、PageUp/Down、Home/End）的默认行为。
- 画布使用 `touch-action: none`，触摸滚动由外壳拦截并转发为 pointer 事件。
- 指针按下时外壳对画布执行 pointer capture。

## 8. 版本化

v2 为加法式升级，v1 的 2D hosted 契约不变。协议正式发布后正文不再修改，需要改变
规范时发布新版本与文件名。