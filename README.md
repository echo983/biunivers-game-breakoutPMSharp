# Biunivers Breakout

经典打砖块（Breakout）游戏，作为 Biunivers 静态桌面应用运行。当前为 **v1 完整可玩版本**：
开局三选一选择球拍（滑板/橄榄球/碗，物理各异），颠球增压后击破三种砖块，在有限球数内
清版取胜。版本历史见 [`CHANGELOG.md`](CHANGELOG.md)，文档索引见 [`docs/README.md`](docs/README.md)。

- 协议：`biunivers.static-app/1` + `biunivers.game-runtime/2`
- 入口：`index.html`（仓库根目录）
- 交付物：预构建的 `game_bg.wasm` + 胶水 `game.js`（已提交，Biunivers 不做构建）
- 无后端、无 secret

## 玩法

开局显示球拍选择界面：点击卡片或按键盘 **1 / 2 / 3** 选择球拍。

| 球拍 | 形状 | 弹性 | 特性 |
|---|---|---|---|
| 滑板 | 扁平长方体 | 0.70 | 反弹直接，移动最快（720 px/s） |
| 橄榄球 | 凸面胶囊 | 0.95 | 最弹，命中位置不同反弹角多变，移动中速 |
| 碗 | 凹面弧壁 | 0.50 | 收住小球并向上定向送出，可控性最强，移动较慢 |

- 移动：鼠标/触摸（指针跟随）或 ← → 方向键
- 发球：空格或点击（球停在球拍上时）
- 每次球拍成功接球反弹：出射速度 = 接近速度 × **1.30**（只增不减，上限 2000 px/s；
  经物理仿真标定，颠球 3-4 次后球速足以够到砖块区并击破普通砖，见 `docs/design-v1.md`）
- 碗为半透明材质，球进入碗内仍可见

**砖块（8×6 阵，自上而下由硬到软）**

| 类型 | 行 | 生命 | 破砖冲击阈值 |
|---|---|---|---|
| 高阻力砖 | 顶 2 行 | 2 | 1100 px/s |
| 普通砖 | 中 2 行 | 1 | 800 px/s |
| 软砖 | 底 2 行 | 1 | 500 px/s |

- 冲击 ≥ 阈值 → 扣血/破坏；低于阈值 → 砖块反弹（不衰减球速）
- 破砖时球按经典 Breakout 方式反射（击破不扣球速；真穿透见 v1.1 规划）

**规则**
- 初始 **5 球**；球落出底部扣 1 球，0 球则游戏结束
- **清空全部砖块 → 胜利**
- 结算画面点击或按 **R** 重新开始（保留所选球拍）

## 架构

遵循 **Biunivers Game Runtime Protocol v2**（见
`BIUNIVERS_GAME_RUNTIME_PROTOCOL_V2.md`），外壳与游戏内容分离：

- 游戏本体：Rust（`src/lib.rs`）编译为单个 WASM `game_bg.wasm`，内含 Rapier2d 物理世界
  （球拍为运动学刚体，球为动态刚体 + CCD）；渲染用 `wgpu`（WebGPU 后端）画 3D 场景
  （小球球体 + 球拍/墙/天花板立方体或半球 + 透视相机 + 光照）。
- 运行时外壳：`runtime.js` + `index.html`，内容无关——加载 WASM、协商渲染后端（WebGPU）、
  转发输入、跑 `requestAnimationFrame` 循环、处理缩放/DPR/可见性/配置；不支持 WebGPU 时
  显示友好提示（不提供 2D 回退）。
- 界面：球拍选择菜单由游戏本体（Rust）通过 DOM 管理，外壳保持内容无关。
- 接口：`render_backend/hosting_mode/setup/setup_gpu/configure/resize/step/render/key/
  pointer_*/wheel/set_paused/destroy`（见 v2 协议；`select_paddle` 为游戏内部 DOM 接口，
  外壳不调用）。
- 物理单位：像素（100px = 1m），`length_unit = 100` 避免 Rapier 默认 400 m/s 的速度钳制
  变成 400px/s 硬上限（见 git log 0.3.5）。

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

- `index.html`：应用入口（外壳 + 球拍选择菜单 + HUD/结算覆盖层）
- `runtime.js`：内容无关的运行时外壳
- `game.js` / `game_bg.wasm`：wasm-bindgen 胶水 + 游戏本体（构建产物）
- `src/lib.rs`：游戏逻辑、物理、球拍类型与砖块系统
- `src/renderer.rs`：wgpu 3D 渲染器
- `src/mesh.rs`：球体/立方体/半球网格生成
- `Cargo.toml` / `Cargo.lock`：Rust 工程
- `build.sh`：构建脚本
- `style.css` / `icon.svg`：样式与图标
- `biunivers.app.json`：应用清单
- `CHANGELOG.md`：发布历史（SemVer）
- `docs/`：设计/经验/规范文档（索引见 `docs/README.md`）
- `BIUNIVERS_APP_PROTOCOL_V1.md`：静态应用协议原文（冻结，请勿修改）
- `BIUNIVERS_GAME_RUNTIME_PROTOCOL_V1.md` / `_V2.md`：游戏运行时协议草稿（本仓库自有，非冻结）
- `AGENTS.md`：AI 开发代理约束

## 配置

暂无公开配置项。
