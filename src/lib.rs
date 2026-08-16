mod mesh;
mod renderer;

use std::cell::RefCell;
use std::sync::mpsc::{channel, Receiver};

use rapier2d::dynamics::CoefficientCombineRule;
use rapier2d::prelude::*;
use renderer::Renderer;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, MouseEvent};

const DT: f64 = 1.0 / 60.0;
const GRAVITY: f32 = -1200.0;
const BALL_RADIUS: f32 = 12.0;
const WALL_THICKNESS: f32 = 40.0;
// 球自身的弹性为 0，配合 Max 合并规则让"接触面的弹性"决定反弹：
// 墙面/天花板 1.0（不衰减），球拍按类型 0.7 / 0.95 / 0.5。
const WALL_RESTITUTION: f32 = 1.0;
const FRICTION: f32 = 0.5;
const SETTLE_SPEED: f32 = 2.0;
const SETTLE_ANGVEL: f32 = 0.1;

// 球拍
const PADDLE_Y: f32 = 50.0;
const PADDLE_MOVE_SPEED: [f32; 3] = [720.0, 620.0, 500.0];
const PADDLE_HALF_W: [f32; 3] = [90.0, 80.0, 100.0];
const PADDLE_HALF_H: [f32; 3] = [8.0, 18.0, 45.0];
const PADDLE_REST: [f32; 3] = [0.7, 0.95, 0.5];
const VARIANT_SKATE: u8 = 0;
const VARIANT_RUGBY: u8 = 1;
const VARIANT_BOWL: u8 = 2;

const SERVE_SPEED: f32 = 520.0;
const LOSS_Y: f32 = -80.0;
// 球拍接球时的速度增压系数（见 docs/design-v0.1.md §6）
// 由物理仿真标定：+30% 使颠球 3-4 次后球速足以够到砖块区并击破普通砖
//（仿真见 0.4.2 提交说明；B=1.30 时 650→798→947→1100...）
const PADDLE_BOOST: f32 = 1.30;
// 球速上限：防止多次增压后球快到完全无法接住（设计草案 §16 收束条件）
const MAX_BALL_SPEED: f32 = 2000.0;
// 低于该接近速度不视为一次有效的球拍击球（避免发球/停驻误触发）
const BOOST_MIN_APPROACH: f32 = 50.0;

// ---- 砖块（见 docs/design-v1.md §3）----
const BRICK_COLS: u32 = 8;
const BRICK_ROWS: u32 = 6;
const BRICK_W: f32 = 56.0;
const BRICK_H: f32 = 22.0;
const BRICK_GAP: f32 = 4.0;

// 冲击阈值（px/s）：球在本物理子步开始前的速度
const SOFT_THRESHOLD: f32 = 500.0;
const NORMAL_THRESHOLD: f32 = 800.0;
const HARD_THRESHOLD: f32 = 1100.0;

// ---- 球数（见 docs/design-v1.md §4）----
const START_BALLS: u32 = 5;

#[derive(Clone, Copy, PartialEq, Debug)]
enum BrickKind {
    Soft,
    Normal,
    Hard,
}

struct Brick {
    body: RigidBodyHandle,
    collider: ColliderHandle,
    kind: BrickKind,
    hp: u32,
    threshold: f32,
    render_index: usize,
    alive: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Menu,
    Serve,
    Play,
    GameOver,
    Victory,
}

struct Game {
    world: PhysicsWorld,
    ball: Option<RigidBodyHandle>,
    ball_collider: ColliderHandle,
    paddle: Option<RigidBodyHandle>,
    paddle_collider: ColliderHandle,
    walls: Vec<RigidBodyHandle>,
    renderer: Option<Renderer>,
    event_handler: ChannelEventCollector,
    collision_recv: Receiver<CollisionEvent>,
    state: State,
    paddle_kind: u8,
    paddle_half_w: f32,
    paddle_half_h: f32,
    paddle_move_speed: f32,
    paddle_target_x: f32,
    key_left: bool,
    key_right: bool,
    width: f32,
    height: f32,
    accumulator: f64,
    paused: bool,
    bricks: Vec<Brick>,
    brick_cols: u32,
    balls: u32,
    bricks_remaining: u32,
}

thread_local! {
    static GAME: RefCell<Option<Game>> = RefCell::new(None);
}

fn with_game<T>(f: impl FnOnce(&mut Game) -> T) -> T {
    GAME.with(|slot| {
        let mut borrow = slot.borrow_mut();
        f(borrow.as_mut().expect("game is not initialized"))
    })
}

fn insert_bounds(world: &mut PhysicsWorld, width: f32, height: f32) -> Vec<RigidBodyHandle> {
    let mut walls = Vec::with_capacity(3);

    // 天花板（固定）
    walls.push(
        world
            .insert(
                RigidBodyBuilder::fixed()
                    .translation(Vec2::new(width * 0.5, height + WALL_THICKNESS * 0.5)),
                ColliderBuilder::cuboid(width * 0.5 + WALL_THICKNESS, WALL_THICKNESS * 0.5)
                    .restitution(WALL_RESTITUTION)
                    .friction(0.2),
            )
            .0,
    );

    // 左墙
    walls.push(
        world
            .insert(
                RigidBodyBuilder::fixed()
                    .translation(Vec2::new(-WALL_THICKNESS * 0.5, height * 0.5)),
                ColliderBuilder::cuboid(WALL_THICKNESS * 0.5, height * 0.5 + WALL_THICKNESS)
                    .restitution(WALL_RESTITUTION)
                    .friction(0.2),
            )
            .0,
    );

    // 右墙
    walls.push(
        world
            .insert(
                RigidBodyBuilder::fixed()
                    .translation(Vec2::new(width + WALL_THICKNESS * 0.5, height * 0.5)),
                ColliderBuilder::cuboid(WALL_THICKNESS * 0.5, height * 0.5 + WALL_THICKNESS)
                    .restitution(WALL_RESTITUTION)
                    .friction(0.2),
            )
            .0,
    );

    walls
}

/// 碗：凹面向上的抛物线 U 形折线。
fn bowl_points() -> Vec<Vec2> {
    let w = PADDLE_HALF_W[VARIANT_BOWL as usize];
    let depth = PADDLE_HALF_H[VARIANT_BOWL as usize];
    let mut pts = Vec::with_capacity(9);
    for i in 0..=8 {
        let x = -w + 2.0 * w * (i as f32 / 8.0);
        let y = -depth * (1.0 - (x / w).powi(2));
        pts.push(Vec2::new(x, y));
    }
    pts
}

fn paddle_collider(kind: u8) -> ColliderBuilder {
    match kind {
        VARIANT_RUGBY => ColliderBuilder::capsule_x(
            PADDLE_HALF_W[VARIANT_RUGBY as usize] - PADDLE_HALF_H[VARIANT_RUGBY as usize],
            PADDLE_HALF_H[VARIANT_RUGBY as usize],
        )
        .restitution(PADDLE_REST[VARIANT_RUGBY as usize])
        .friction(0.4),
        VARIANT_BOWL => ColliderBuilder::polyline(bowl_points(), None)
            .restitution(PADDLE_REST[VARIANT_BOWL as usize])
            .friction(0.3),
        _ => ColliderBuilder::cuboid(
            PADDLE_HALF_W[VARIANT_SKATE as usize],
            PADDLE_HALF_H[VARIANT_SKATE as usize],
        )
        .restitution(PADDLE_REST[VARIANT_SKATE as usize])
        .friction(0.4),
    }
}

fn ball_rest_y(kind: u8) -> f32 {
    match kind {
        VARIANT_RUGBY => PADDLE_Y + PADDLE_HALF_H[VARIANT_RUGBY as usize] + BALL_RADIUS,
        VARIANT_BOWL => PADDLE_Y - PADDLE_HALF_H[VARIANT_BOWL as usize] + BALL_RADIUS,
        _ => PADDLE_Y + PADDLE_HALF_H[VARIANT_SKATE as usize] + BALL_RADIUS,
    }
}

fn select_paddle_inner(g: &mut Game, kind: u8) {
    g.paddle_kind = kind;
    g.paddle_half_w = PADDLE_HALF_W[kind as usize];
    g.paddle_half_h = PADDLE_HALF_H[kind as usize];
    g.paddle_move_speed = PADDLE_MOVE_SPEED[kind as usize];
    g.paddle_target_x = g.width * 0.5;

    let (paddle, paddle_collider) = g.world.insert(
        RigidBodyBuilder::kinematic_position_based()
            .translation(Vec2::new(g.width * 0.5, PADDLE_Y)),
        paddle_collider(kind).active_events(ActiveEvents::COLLISION_EVENTS),
    );
    g.paddle = Some(paddle);
    g.paddle_collider = paddle_collider;

    let (ball, ball_collider) = g
        .world
        .insert(
            RigidBodyBuilder::dynamic()
                .translation(Vec2::new(g.width * 0.5, ball_rest_y(kind)))
                .linear_damping(0.2)
                .angular_damping(0.9)
                .ccd_enabled(true),
            ColliderBuilder::ball(BALL_RADIUS)
                .restitution(0.0)
                .restitution_combine_rule(CoefficientCombineRule::Max)
                .friction(FRICTION)
                .density(1.0)
                .active_events(ActiveEvents::COLLISION_EVENTS),
        );
    g.ball = Some(ball);
    g.ball_collider = ball_collider;

    g.state = State::Serve;
    hide_menu();
    show_hud();
    bind_overlay_clicks();
    build_level(g);
}

fn serve(g: &mut Game) {
    let Some(ball) = g.ball else { return };
    let body = g.world.bodies.get_mut(ball).unwrap();
    body.set_linvel(Vec2::new(-60.0, SERVE_SPEED), true);
    g.state = State::Play;
}

fn reset_to_serve(g: &mut Game) {
    let Some(ball) = g.ball else { return };
    let Some(paddle) = g.paddle else { return };
    let px = g.world.bodies[paddle].translation().x;
    let body = g.world.bodies.get_mut(ball).unwrap();
    body.set_translation(Vec2::new(px, ball_rest_y(g.paddle_kind)), true);
    body.set_linvel(Vec2::ZERO, true);
    body.set_angvel(0.0, true);
    g.state = State::Serve;
    g.accumulator = 0.0;
}

// ---- 砖块与关卡 ----

/// 按窗口宽度确定砖块列数（最多 8 列，最少 3 列），确保窄窗口下砖阵不溢出。
fn brick_cols_for_width(width: f32) -> u32 {
    ((width / (BRICK_W + BRICK_GAP)).floor() as u32).clamp(3, BRICK_COLS)
}

/// 砖阵总宽度（px）。
fn brick_total_w(cols: u32) -> f32 {
    cols as f32 * BRICK_W + (cols as f32 - 1.0) * BRICK_GAP
}

/// 砖阵最左列中心 x：按窗口宽度居中。
fn brick_left_x(width: f32, cols: u32) -> f32 {
    (width - brick_total_w(cols)) * 0.5 + BRICK_W * 0.5
}

/// 顶行中心 y：距天花板内壁 28px（球径 12 + 间隙 16），确保球可在砖顶与天花板间活动。
fn brick_top_y(height: f32) -> f32 {
    height - 28.0
}

fn brick_kind_for_row(row: u32) -> BrickKind {
    // 自上而下：硬（顶 2 行）→ 普通（中 2 行）→ 软（底 2 行）
    match row {
        0 | 1 => BrickKind::Hard,
        2 | 3 => BrickKind::Normal,
        _ => BrickKind::Soft,
    }
}

fn brick_stats(kind: BrickKind) -> (u32, f32) {
    match kind {
        BrickKind::Soft => (1, SOFT_THRESHOLD),
        BrickKind::Normal => (1, NORMAL_THRESHOLD),
        BrickKind::Hard => (2, HARD_THRESHOLD),
    }
}

fn brick_color(kind: BrickKind) -> [f32; 4] {
    match kind {
        BrickKind::Soft => [0.35, 0.85, 0.5, 1.0],
        BrickKind::Normal => [0.95, 0.65, 0.2, 1.0],
        BrickKind::Hard => [0.85, 0.2, 0.3, 1.0],
    }
}

/// 重建砖块阵（清空旧砖块 + 按布局生成新砖，列数随窗口宽度自适应）。
fn build_level(g: &mut Game) {
    for brick in g.bricks.drain(..) {
        g.world.remove_body(brick.body);
    }
    if let Some(renderer) = g.renderer.as_mut() {
        renderer.clear_bricks();
    }
    g.bricks_remaining = 0;

    let cols = brick_cols_for_width(g.width);
    g.brick_cols = cols;
    let top_y = brick_top_y(g.height);
    let left_x = brick_left_x(g.width, cols);
    for row in 0..BRICK_ROWS {
        let kind = brick_kind_for_row(row);
        let (hp, threshold) = brick_stats(kind);
        let y = top_y - row as f32 * (BRICK_H + BRICK_GAP);
        for col in 0..cols {
            let x = left_x + col as f32 * (BRICK_W + BRICK_GAP);
            let (body, collider) = g.world.insert(
                RigidBodyBuilder::fixed().translation(Vec2::new(x, y)),
                ColliderBuilder::cuboid(BRICK_W * 0.5, BRICK_H * 0.5)
                    .restitution(1.0)
                    .friction(0.3)
                    .active_events(ActiveEvents::COLLISION_EVENTS),
            );
            let render_index = match g.renderer.as_mut() {
                Some(renderer) => {
                    renderer.add_brick(x, y, BRICK_W * 0.5, BRICK_H * 0.5, brick_color(kind))
                }
                None => 0,
            };
            g.bricks.push(Brick {
                body,
                collider,
                kind,
                hp,
                threshold,
                render_index,
                alive: true,
            });
            g.bricks_remaining += 1;
        }
    }
    update_hud(g);
}

fn brick_index_by_collider(bricks: &[Brick], handle: ColliderHandle) -> Option<usize> {
    bricks
        .iter()
        .position(|b| b.alive && b.collider == handle)
}

/// 球丢失：扣球，0 则 GameOver，否则回到发球。
fn on_ball_lost(g: &mut Game) {
    g.balls = g.balls.saturating_sub(1);
    update_hud(g);
    if g.balls == 0 {
        g.state = State::GameOver;
        show_overlay("gameover");
    } else {
        reset_to_serve(g);
    }
}

/// 重新开始：球数回满、重建关卡、回到发球（保留所选球拍）。
fn restart(g: &mut Game) {
    g.balls = START_BALLS;
    g.state = State::Serve;
    g.accumulator = 0.0;
    build_level(g);
    if let (Some(ball), Some(paddle)) = (g.ball, g.paddle) {
        let px = g.world.bodies[paddle].translation().x;
        let body = g.world.bodies.get_mut(ball).unwrap();
        body.set_translation(Vec2::new(px, ball_rest_y(g.paddle_kind)), true);
        body.set_linvel(Vec2::ZERO, true);
        body.set_angvel(0.0, true);
    }
    hide_overlays();
    update_hud(g);
}

// ---- HUD DOM（游戏自管的内容 UI）----

fn set_text(id: &str, text: &str) {
    if let Some(el) = document_element(id) {
        let _ = el.set_text_content(Some(text));
    }
}

fn update_hud(g: &Game) {
    set_text("hud-balls", &format!("球 ×{}", g.balls));
    set_text("hud-bricks", &format!("剩余砖块 {}", g.bricks_remaining));
}

fn show_hud() {
    if let Some(hud) = document_element("hud") {
        let _ = hud.remove_attribute("hidden");
    }
}

fn hide_hud() {
    if let Some(hud) = document_element("hud") {
        let _ = hud.set_attribute("hidden", "");
    }
}

fn show_overlay(id: &str) {
    if let Some(el) = document_element(id) {
        let _ = el.remove_attribute("hidden");
    }
}

fn hide_overlays() {
    for id in ["gameover", "victory"] {
        if let Some(el) = document_element(id) {
            let _ = el.set_attribute("hidden", "");
        }
    }
}

fn bind_overlay_clicks() {
    static BOUND: std::sync::Once = std::sync::Once::new();
    BOUND.call_once(|| {
        for id in ["gameover", "victory"] {
            if let Some(el) = document_element(id) {
                let cb = Closure::wrap(Box::new(move |_event: MouseEvent| {
                    GAME.with(|slot| {
                        let mut borrow = slot.borrow_mut();
                        if let Some(g) = borrow.as_mut() {
                            match g.state {
                                State::GameOver | State::Victory => restart(g),
                                _ => {}
                            }
                        }
                    });
                }) as Box<dyn FnMut(MouseEvent)>);
                let _ = el.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref());
                cb.forget();
            }
        }
    });
}

// ---- 菜单 DOM（游戏自管的内容 UI）----

fn document_element(id: &str) -> Option<web_sys::Element> {
    let window = web_sys::window()?;
    let document = window.document()?;
    document.get_element_by_id(id)
}

fn show_menu() {
    if let Some(menu) = document_element("menu") {
        let _ = menu.remove_attribute("hidden");
        let _ = menu.set_attribute("aria-hidden", "false");
    }
    bind_menu_clicks();
}

fn hide_menu() {
    if let Some(menu) = document_element("menu") {
        let _ = menu.set_attribute("hidden", "");
    }
}

fn bind_menu_clicks() {
    static BOUND: std::sync::Once = std::sync::Once::new();
    BOUND.call_once(|| {
        let ids = ["paddle-skate", "paddle-rugby", "paddle-bowl"];
        for (index, id) in ids.iter().enumerate() {
            if let Some(el) = document_element(id) {
                let kind = index as u8;
                let cb = Closure::wrap(Box::new(move |_event: MouseEvent| {
                    GAME.with(|slot| {
                        let mut borrow = slot.borrow_mut();
                        if let Some(g) = borrow.as_mut() {
                            if g.state == State::Menu {
                                select_paddle_inner(g, kind);
                            }
                        }
                    });
                }) as Box<dyn FnMut(MouseEvent)>);
                let _ = el.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref());
                cb.forget();
            }
        }
    });
}

// ---- WASM 导出接口（Biunivers Game Runtime Protocol v2）----

#[wasm_bindgen]
pub fn render_backend() -> u32 {
    1 // WebGPU
}

#[wasm_bindgen]
pub fn hosting_mode() -> u32 {
    0 // hosted
}

#[wasm_bindgen]
pub fn setup(_ctx: CanvasRenderingContext2d, _width: f64, _height: f64) {
    // 2D hosted 接口（协议保留）；本游戏使用 WebGPU，由外壳选择 setup_gpu。
}

#[wasm_bindgen]
pub async fn setup_gpu(canvas: HtmlCanvasElement, width: f64, height: f64) -> bool {
    let width = width as f32;
    let height = height as f32;
    let renderer = Renderer::new(canvas, width, height).await;

    match renderer {
        Ok(renderer) => {
            let mut world = PhysicsWorld::new();
            world.gravity = Vec2::new(0.0, GRAVITY);
            world.integration_parameters.dt = DT as f32;
            // 像素单位：100px = 1m。默认 length_unit=1 时 400 m/s 的速度钳制
            // 会变成 400px/s 硬上限，导致落体被限速（见 git log 0.3.5）。
            world.integration_parameters.length_unit = 100.0;

            let walls = insert_bounds(&mut world, width, height);

            // 碰撞事件通道：用于检测球-球拍接触，实现 +10% 增压。
            let (collision_send, collision_recv) = channel();
            let (force_send, _force_recv) = channel();
            let event_handler = ChannelEventCollector::new(collision_send, force_send);

            GAME.with(|slot| {
                *slot.borrow_mut() = Some(Game {
                    world,
                    ball: None,
                    ball_collider: ColliderHandle::from_raw_parts(u32::MAX, u32::MAX),
                    paddle: None,
                    paddle_collider: ColliderHandle::from_raw_parts(u32::MAX, u32::MAX),
                    walls,
                    renderer: Some(renderer),
                    event_handler,
                    collision_recv,
                    state: State::Menu,
                    paddle_kind: VARIANT_SKATE,
                    paddle_half_w: PADDLE_HALF_W[VARIANT_SKATE as usize],
                    paddle_half_h: PADDLE_HALF_H[VARIANT_SKATE as usize],
                    paddle_move_speed: PADDLE_MOVE_SPEED[VARIANT_SKATE as usize],
                    paddle_target_x: width * 0.5,
                    key_left: false,
                    key_right: false,
                    width,
                    height,
                    accumulator: 0.0,
                    paused: false,
                    bricks: Vec::new(),
                    brick_cols: 0,
                    balls: START_BALLS,
                    bricks_remaining: 0,
                });
            });
            show_menu();
            true
        }
        Err(_) => false,
    }
}

/// 选择球拍：0=滑板 1=橄榄球 2=碗（点击菜单或键盘 1/2/3 触发）。
#[wasm_bindgen]
pub fn select_paddle(kind: u32) {
    GAME.with(|slot| {
        let mut borrow = slot.borrow_mut();
        let Some(g) = borrow.as_mut() else { return };
        if g.state != State::Menu {
            return;
        }
        select_paddle_inner(g, (kind % 3) as u8);
    });
}

#[wasm_bindgen]
pub fn configure(config: &str) {
    // 当前游戏没有公开配置；保留接口，未来用 serde 解析。
    let _ = config;
}

#[wasm_bindgen]
pub fn resize(width: f64, height: f64) {
    with_game(|g| {
        let width = width as f32;
        let height = height as f32;

        for handle in g.walls.drain(..) {
            g.world.remove_body(handle);
        }
        g.walls = insert_bounds(&mut g.world, width, height);
        g.width = width;
        g.height = height;

        if let Some(paddle) = g.paddle {
            let half = g.paddle_half_w;
            g.paddle_target_x = g.paddle_target_x.clamp(half, width - half);
            let body = g.world.bodies.get_mut(paddle).unwrap();
            body.set_translation(Vec2::new(g.paddle_target_x, PADDLE_Y), true);
        }
        if let Some(ball) = g.ball {
            let body = g.world.bodies.get_mut(ball).unwrap();
            let mut pos = body.translation();
            pos.x = pos.x.clamp(BALL_RADIUS, width - BALL_RADIUS);
            pos.y = pos.y.clamp(BALL_RADIUS, height - BALL_RADIUS);
            body.set_translation(pos, true);
        }

        // 砖块按新窗口尺寸重新布局：列数变化则重建（含进度重置），否则仅重定位。
        // Menu 状态尚无砖块，跳过（避免提前建砖，选拍时会统一重建）。
        if g.state != State::Menu {
            let cols = brick_cols_for_width(width);
            if cols != g.brick_cols {
                build_level(g);
            } else {
                let top_y = brick_top_y(height);
                let left_x = brick_left_x(width, g.brick_cols);
                for (i, brick) in g.bricks.iter_mut().enumerate() {
                    if !brick.alive {
                        continue;
                    }
                    let col = (i as u32 % g.brick_cols) as f32;
                    let row = (i as u32 / g.brick_cols) as f32;
                    let x = left_x + col * (BRICK_W + BRICK_GAP);
                    let y = top_y - row * (BRICK_H + BRICK_GAP);
                    if let Some(body) = g.world.bodies.get_mut(brick.body) {
                        body.set_translation(Vec2::new(x, y), true);
                    }
                    if let Some(renderer) = g.renderer.as_mut() {
                        renderer.move_brick(brick.render_index, x, y, BRICK_W * 0.5, BRICK_H * 0.5);
                    }
                }
            }
        }

        if let Some(renderer) = g.renderer.as_mut() {
            renderer.resize(width, height);
        }
    });
}

#[wasm_bindgen]
pub fn step(dt: f64) {
    with_game(|g| {
        if g.paused {
            return;
        }

        // 键盘移动球拍（Serve 与 Play 都生效）
        if g.key_left {
            g.paddle_target_x -= g.paddle_move_speed * dt as f32;
        }
        if g.key_right {
            g.paddle_target_x += g.paddle_move_speed * dt as f32;
        }
        if let Some(paddle) = g.paddle {
            let half = g.paddle_half_w;
            g.paddle_target_x = g.paddle_target_x.clamp(half, g.width - half);
            let body = g.world.bodies.get_mut(paddle).unwrap();
            body.set_next_kinematic_translation(Vec2::new(g.paddle_target_x, PADDLE_Y));
        }

        match g.state {
            State::Menu | State::GameOver | State::Victory => {}
            State::Serve => {
                // 球粘在球拍上，等待发球
                let (ball, paddle) = (g.ball.unwrap(), g.paddle.unwrap());
                let px = g.world.bodies[paddle].translation().x;
                let body = g.world.bodies.get_mut(ball).unwrap();
                body.set_translation(Vec2::new(px, ball_rest_y(g.paddle_kind)), true);
                body.set_linvel(Vec2::ZERO, true);
                body.set_angvel(0.0, true);
                g.accumulator = 0.0;
            }
            State::Play => {
                g.accumulator += dt;
                if g.accumulator > 0.25 {
                    g.accumulator = 0.25;
                }
                while g.accumulator >= DT {
                    // 记录接近速度（本子步开始前）：用于球拍增压与砖块破坏判定
                    let approach_speed = g.world.bodies[g.ball.unwrap()].linvel().length();
                    let vy_before = g.world.bodies[g.ball.unwrap()].linvel().y;

                    g.world.step_with_events(&(), &g.event_handler);

                    // 收集本子步的接触事件
                    let mut hit_paddle = false;
                    let mut hit_bricks: Vec<usize> = Vec::new();
                    while let Ok(ev) = g.collision_recv.try_recv() {
                        if let CollisionEvent::Started(c1, c2, _) = ev {
                            if (c1 == g.ball_collider && c2 == g.paddle_collider)
                                || (c1 == g.paddle_collider && c2 == g.ball_collider)
                            {
                                hit_paddle = true;
                            }
                            if c1 == g.ball_collider {
                                if let Some(i) = brick_index_by_collider(&g.bricks, c2) {
                                    hit_bricks.push(i);
                                }
                            } else if c2 == g.ball_collider {
                                if let Some(i) = brick_index_by_collider(&g.bricks, c1) {
                                    hit_bricks.push(i);
                                }
                            }
                        }
                    }

                    // 球-球拍：按 1.3× 接近速度重设出射速度（方向保留求解器结果）。
                    // vy_before < 0：必须是向下接近球拍（发球时球向上离开，不触发）。
                    if hit_paddle && vy_before < 0.0 && approach_speed > BOOST_MIN_APPROACH {
                        let body = g.world.bodies.get_mut(g.ball.unwrap()).unwrap();
                        let v = body.linvel();
                        if v.length() > 0.0 {
                            let speed = (approach_speed * PADDLE_BOOST).min(MAX_BALL_SPEED);
                            body.set_linvel(v.normalize() * speed, true);
                        }
                    }

                    // 球-砖块：冲击 ≥ 阈值则扣血/破坏；否则砖块反弹（restitution 1.0 不衰减球速）。
                    for &i in &hit_bricks {
                        if i >= g.bricks.len() || !g.bricks[i].alive {
                            continue;
                        }
                        let (destroy, hp_after) = {
                            let b = &g.bricks[i];
                            if approach_speed >= b.threshold {
                                if b.hp <= 1 {
                                    (true, 0)
                                } else {
                                    (false, b.hp - 1)
                                }
                            } else {
                                (false, b.hp)
                            }
                        };
                        if destroy {
                            let body = g.bricks[i].body;
                            let render_index = g.bricks[i].render_index;
                            g.world.remove_body(body);
                            g.bricks[i].alive = false;
                            g.bricks_remaining -= 1;
                            if let Some(renderer) = g.renderer.as_mut() {
                                renderer.hide_brick(render_index);
                            }
                            update_hud(g);
                        } else if hp_after != g.bricks[i].hp {
                            g.bricks[i].hp = hp_after;
                        }
                    }

                    g.accumulator -= DT;
                }

                // 清版胜利
                if g.bricks_remaining == 0 {
                    g.state = State::Victory;
                    update_hud(g);
                    show_overlay("victory");
                }

                // 低速归零；若球已落在球拍附近则回到发球状态。
                if g.state == State::Play {
                    let body = g.world.bodies.get_mut(g.ball.unwrap()).unwrap();
                    if body.linvel().length() < SETTLE_SPEED && body.angvel().abs() < SETTLE_ANGVEL
                    {
                        if body.translation().y > PADDLE_Y - 60.0 {
                            g.state = State::Serve;
                        }
                    }
                }

                // 球落出画面：扣球，0 则 GameOver
                if g.state == State::Play
                    && g.world.bodies[g.ball.unwrap()].translation().y < LOSS_Y
                {
                    on_ball_lost(g);
                }
            }
        }
    });
}

#[wasm_bindgen]
pub fn render() {
    with_game(|g| {
        let Some(renderer) = g.renderer.as_mut() else {
            return;
        };
        if let Some(ball) = g.ball {
            let pos = g.world.bodies[ball].translation();
            renderer.update_ball(pos.x, pos.y, BALL_RADIUS);
        }
        if let Some(paddle) = g.paddle {
            let pos = g.world.bodies[paddle].translation();
            renderer.update_paddle(pos.x, pos.y, g.paddle_kind, g.paddle_half_w, g.paddle_half_h);
        }
        renderer.render();
    });
}

#[wasm_bindgen]
pub fn key(code: &str, down: bool) {
    GAME.with(|slot| {
        let mut borrow = slot.borrow_mut();
        let Some(g) = borrow.as_mut() else {
            return;
        };

        if g.state == State::Menu {
            if down {
                match code {
                    "Digit1" => select_paddle_inner(g, VARIANT_SKATE),
                    "Digit2" => select_paddle_inner(g, VARIANT_RUGBY),
                    "Digit3" => select_paddle_inner(g, VARIANT_BOWL),
                    _ => {}
                }
            }
            return;
        }

        if matches!(g.state, State::GameOver | State::Victory) {
            if down && code == "KeyR" {
                restart(g);
            }
            return;
        }

        match code {
            "ArrowLeft" => g.key_left = down,
            "ArrowRight" => g.key_right = down,
            "Space" if down && g.state == State::Serve => serve(g),
            _ => {}
        }
    });
}

#[wasm_bindgen]
pub fn pointer_down(_x: f64, _y: f64, _buttons: u32) {
    with_game(|g| {
        if g.state == State::Serve {
            serve(g);
        }
    });
}

#[wasm_bindgen]
pub fn pointer_up(_x: f64, _y: f64, _buttons: u32) {}

#[wasm_bindgen]
pub fn pointer_move(x: f64, _y: f64, _buttons: u32) {
    with_game(|g| {
        if g.state == State::Menu {
            return;
        }
        let half = g.paddle_half_w;
        g.paddle_target_x = (x as f32).clamp(half, g.width - half);
    });
}

#[wasm_bindgen]
pub fn wheel(_dx: f64, _dy: f64) {}

#[wasm_bindgen]
pub fn set_paused(paused: bool) {
    with_game(|g| g.paused = paused);
}

#[wasm_bindgen]
pub fn destroy() {}
