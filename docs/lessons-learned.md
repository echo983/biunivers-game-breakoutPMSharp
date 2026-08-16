# 开发日志：重大错误与经验

> 项目：Biunivers Breakout（Rust + Rapier2d → WASM，运行于 Biunivers 静态桌面）
> 范围：初始化 → 引入物理引擎底座

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