# 开发日志：重大错误与经验

> 项目：Biunivers Breakout（Rust + Rapier2d → WASM，运行于 Biunivers 静态桌面）
> 范围：初始化 → 物理底座 → 三球拍 → v1 完整可玩版本（砖块/球数/结算）
> 维护：经验按发现顺序追加；`CHANGELOG.md` 记录发布历史，本文件记录「为什么」。

## 1. Rapier 0.35 已迁移到 glam 数学类型

**现象**：`cargo build` 报 `expected Vec2, found Matrix<...>`，共 7 处类型不匹配。

**原因**：rapier2d 0.35 从 nalgebra 迁移到 glam；`Vector = glam::Vec2`、`Real = f32`。但
`use rapier2d::prelude::*` 仍 re-export 了 nalgebra 的 `vector!`/`point!` 宏，容易误用。

**解决**：把 `vector![x, y]` 全部改为 `Vec2::new(x, y)`。

**经验**：
- 升级/引入新版本后先确认数学后端（nalgebra vs glam）。
- `Rotation = Rot2`（glamx），有 `.angle()`；glam `Vec2` 有 `ZERO`、`.length()`、`.clamp()`。
- 0.35 提供高层 `PhysicsWorld`（旧版 `World`）：`world.insert(body, collider)` 返回
  `(RigidBodyHandle, ColliderHandle)`，`world.step()` 用 `integration_parameters.dt` 步进。

## 2. wasm-bindgen 不支持相对模块导入路径

**现象**：
1. `#[wasm_bindgen(module = "./runtime.js")]` → 编译错误 `relative module paths aren't supported yet`。
2. 改成 `module = "/runtime.js"` 后，wasm-bindgen 把 `runtime.js` 当 "snippet" 复制进
   `pkg/snippets/<hash>/`，并生成从胶水到 snippet 的 import，造成循环引用 + 相对路径错乱。

**原因**：`#[wasm_bindgen(module)]` 主要为裸说明符/绝对路径设计，不适合「从自己的 JS
模块导入函数」（尤其 `--target web` + 子路径部署）。

**解决**：放弃「Rust 导入 JS 绘图函数」，改为渲染也放进 Rust——用 `web-sys` 的
`CanvasRenderingContext2d` 直接绘制；JS 外壳只负责加载 WASM、把 ctx 传进去、跑循环。
结果更贴合「内容无关外壳」的目标。

**经验**：需要 Rust↔JS 双向调用时，优先考虑「JS 把对象传给 Rust（web-sys）」而不是
「Rust 从 JS 模块 import 函数」。

## 3. web-sys canvas 方法返回类型不一致

**现象/原因**：web-sys 生成的方法是否返回 `Result` 由 IDL 是否可抛异常
（`#[wasm_bindgen(catch)]`）决定：
- 返回 `()`：`clear_rect`、`fill_rect`、`begin_path`、`fill`、`set_fill_style_str`
- 返回 `Result<(), JsValue>`：`arc`（带 catch）

**解决**：写前先 grep web-sys 生成的签名；`arc` 用 `let _ =`；颜色用 `set_fill_style_str`
（免构造 `JsValue`）。

**经验**：web-sys 签名以 `~/.cargo/registry/src/.../web-sys-*/src/features/gen_*.rs` 为准。

## 4. 全局状态：`static mut` 与非 Sync 对象

**现象**：`static mut GAME: Option<Game>` 触发 `static_mut_refs`（rust_2024_compatibility）
警告；且 `Game` 含 `web_sys::CanvasRenderingContext2d`（非 `Send/Sync`），不能放进
`static` 或 `OnceLock`。

**解决**：改用 `thread_local! { static GAME: RefCell<Option<Game>> }`，配合
`with_game(|g| ...)`。

**经验**：wasm 单线程且持有 web-sys 对象时，全局状态用 `thread_local!` 最稳妥。

## 5. 纯 Coulomb 摩擦不能「滚到停」

**现象**：小球落地后一直以约 2px/s 极慢滚动，不能完全停止。

**原因**：Rapier 的 `friction` 是库仑摩擦（阻止滑动），不建模滚动阻力；理想平面上滚动的
球理论上永不停止。

**解决**：加线/角阻尼，并在 `step` 里加低速归零阈值（`|v| < 2px/s && |ω| < 0.1rad/s → 归零`），
显式模拟滚动摩擦。实测 6s 后漂移 0.00px。

**经验**：物理引擎里「滚到停」需要显式建模滚动阻力（阻尼或低速归零），不能只靠 friction。

## 6. favicon 404 干扰验证

**现象**：无头浏览器报 `Failed to load resource: 404`，但游戏本身正常。

**原因**：Chrome 默认请求 `/favicon.ico`，仓库没有。

**解决**：`index.html` 加 `<link rel="icon" href="./icon.svg" />`。

**经验**：排查 404 用 `page.on('response')` 过滤 `status >= 400` 抓具体 URL，不要猜。

## 7. 其他小坑

- `cargo add` 在 `src/lib.rs` 不存在时直接报错（`can't find library`），需先建占位 lib.rs。
- Shell 安全策略拒绝 `kill $PID`；起临时静态服务器用 `timeout N python3 -m http.server ...`
  让进程自动结束。
- 交付模型：Biunivers 不执行构建 → 预构建的 `.wasm` + 胶水必须提交到仓库根目录；
  `pkg/`、`target/` 加入 `.gitignore`。
- `.wasm` 需以 `application/wasm` 提供，`instantiateStreaming` 才能工作（Python
  `http.server` 已正确映射）。
- 外壳会 `fetch("./.biunivers/config.json")`（由宿主提供）；本地静态服务器测试时该请求
  必然 404，属预期，验证时需排除这一条（应用已回退 `{}`）。

## 8. WebGPU / wgpu 30 集成

- `async fn` 的 wasm 导出需要直接依赖 `wasm-bindgen-futures`。
- canvas 上下文类型互斥：不能先 `getContext("2d")` 再 `create_surface(Canvas)`；先按需
  协商后端，再取对应上下文。WebGPU 路径表面用画布物理尺寸（`canvas.width/height`），
  物理与输入用 CSS 尺寸。
- wgpu 30 API 变化：`PipelineLayoutDescriptor.bind_group_layouts` 是 `&[Option<...>]`，
  `push_constant_ranges` 改名 `immediate_size`；`DepthStencilState.depth_write_enabled` /
  `depth_compare` 是 `Option`；`get_current_texture` 返回 `CurrentSurfaceTexture` 枚举
  （非 Result），呈现用 `Queue::present(tex)`。
- WGSL：没有 `mat3x3<f32>(mat4x4)` 构造器；从列构造：
  `mat3x3(m[0].xyz, m[1].xyz, m[2].xyz)`。
- glam 弃用：`Mat4::look_at_rh` / `perspective_rh` → `glam::camera::rh::view::look_at_mat4`
  与 `glam::camera::rh::proj::directx::perspective`。
- 无头 Chrome 测 WebGPU：加 `--enable-unsafe-webgpu --enable-features=Vulkan
  --use-angle=vulkan`；且需在 localhost 等安全上下文（`about:blank` 无 `navigator.gpu`）。
- WebGPU canvas 无法用 2D context 读像素，验证要靠截图 + 解码（如 pngjs）。

## 9. 回退路径移除与友好提示

- canvas 上下文类型互斥：一旦走了 `create_surface(Canvas)`（占用 webgpu 上下文），再
  `getContext("2d")` 会返回 null → 画面空白。因此「WebGPU 失败回退 2D」在已占用上下文
  的情况下不可靠。
- 决策：本游戏不提供 2D 回退。不支持 WebGPU 时外壳显示友好提示 overlay，并停止启动。
- 在进入 `setup_gpu` 前先用 JS 预检测 `navigator.gpu.requestAdapter()`；无 adapter 时直接
  提示，避免 wgpu 在无 adapter 环境内部抛未捕获异常（`requestDevice` null）。
- 提示用独立 DOM overlay（`#unsupported`）覆盖在 canvas 上，与渲染后端无关，任何环境都可见。

## 10. 弹力调不动的真凶：Rapier 按「米」调校

**现象**：把 `RESTITUTION` 提到 0.95 后，球落地弹起极小（像掉进沙子）。

**排查**（证据链，非猜测）：
1. 原生探测程序复现相同参数 → 首次冲击速度仅 398 px/s，理论 892；但反弹比 0.947 ≈ 弹性
   （弹性本身没问题）。
2. 逐帧轨迹 → 下落前期自由落体正常，约 -383 px/s 后**指数逼近 -400 并封顶**。
3. 变量隔离 → 去掉阻尼/CCD/换合并规则均无效，全部 ~400。
4. 读 Rapier 源码 → `IntegrationParameters::normalized_max_linear_velocity` 默认 **400.0**。
   米制调校下，`length_unit=1` 让它变成 400 px/s 硬钳制。

**解决**：像素游戏设 `length_unit = 100.0`（100px = 1m）→ 钳制上限变为 40000 px/s，
同时预测距离/容差等米制参数正确缩放到像素尺度。实测冲击 400→900、首峰 91→334（0.3.5）。

**经验**：
- Rapier 是「米」调校引擎；用像素单位必须设 `length_unit`，否则默认速度钳制会悄悄吃掉
  物理行为。症状表现可能和「弹性」「摩擦」混淆，先用探测程序分离变量。
- 排查物理问题先写**原生复现程序**（不经过 wasm/浏览器），把理论值、逐帧数据、变量隔离
  摆出来，再读源码，最后才改代码。

## 11. 接触对合并规则决定「谁的弹性生效」

**现象**：球 0.95、墙面 0.0，球却几乎不弹。

**原因**：Rapier 默认 `CoefficientCombineRule::Average`：有效弹性 = (0.95+0)/2 = 0.475。

**解决**：球碰撞体设 `.restitution_combine_rule(CoefficientCombineRule::Max)`，让球的 0.95
生效（0.3.3）。此后 v1 沿用同一手法：球弹性 0 + `Max` → 接触面（墙 1.0 / 球拍 0.7~0.95 /
砖 1.0）各自决定反弹。

**经验**：双碰撞体都带材料参数时，先想清楚「谁决定合并值」；`Max` 规则让强的一方生效，
`Min` 让弱的一方生效，`Average` 是默认折中。

## 12. CCD 修「首次弹跳弱」，但要先确认是不是速度钳制

**现象**：首次落地（球速 ~920 px/s，单步位移 15px > 球半径 12px）穿透深，弹回高度异常低。

**解决**：球 `ccd_enabled(true)`（0.3.4）让碰撞在接触表面精确反弹。

**经验**：CCD 治「高速穿透导致的弱反弹」，但**先查速度钳制**（经验 10）——两者症状相似；
本项目的顺序是：先 CCD 无效 → 再用探测锁定钳制 → 修 `length_unit` 才真正解决。

## 13. Rapier 碰撞事件在求解器之后触发 → 破砖是「反射」不是「穿透」

**现象/机制**：`step_with_events` 里，碰撞求解（反弹）先发生，`CollisionEvent::Started`
后触发；事件处理时移除砖块，但球**已经反弹**了。

**结论**：v1 的破砖行为 = 经典 Breakout「击破 + 反射」。「多砖连穿」（v0.1 §8 穿透设计）
需要传感器 + 手动轨道，或先移除砖块再解算——留 v1.1。

**经验**：设计文档写「穿透」前，先确认引擎事件时序；「事件后破坏」天然得到反射而非穿过。

## 14. 碰撞事件的工程细节（wasm 单线程）

- 用 `ChannelEventCollector` + `std::sync::mpsc` 收事件；`world.step_with_events(&(), &handler)`
  替代 `step()`（`()` 实现 `PhysicsHooks`）。wasm 上 `mpsc` 可用。
- 碰撞体需 `active_events(ActiveEvents::COLLISION_EVENTS)`（默认不开，省性能）。
- `world.insert` 返回 `(RigidBodyHandle, ColliderHandle)`，分别保存以备事件比对。
- 发球/重置时球与球拍接触会触发 `Started`，用「向下接近（vy<0）+ 速度下限」守卫避免
  误增压；事件频道里残留事件要在每个子步清空。

## 15. 布局与取景要随窗口自适应

- **砖块布局**：固定坐标在最小窗口（360×400）会出屏。列数随宽度收缩（3-8 列）、顶行
  锚定天花板；resize 仅重定位存活砖块（列数变化才重建）。渲染侧砖块对象保留颜色/尺寸，
  供 `move_brick` 重写 uniform。
- **相机取景**：target 取 `0.35h` 在早期无砖场景可用；加入顶部砖块后顶排出视锥。改
  target/eye 取 `0.53h/0.55h` 居中「球拍→砖顶」游玩区。验证用视锥数学（点在视锥内
  `|v| ≤ d·tan(fov/2)`），不要只靠目测。

## 16. 手感参数的标定方法

- 「+10% 够不够」不能拍脑袋：用仿真复现物理（球拍自动跟踪球模拟熟练玩家），测
  颠球 N 次后的发射速度/可达高度，对照理论下限（够到砖块区需 L ≥ √(2g·Δh)）。
- 结论：+10% 永远够不到砖块区；+30%（并设 2000 px/s 上限）让颠球 3-4 次即可破普通砖
  （0.4.2）。这类参数记入设计文档，标定依据与数据一并归档。