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
const PADDLE_BOOST: f32 = 1.10;
// 低于该接近速度不视为一次有效的球拍击球（避免发球/停驻误触发）
const BOOST_MIN_APPROACH: f32 = 50.0;

#[derive(Clone, Copy, PartialEq)]
enum State {
    Menu,
    Serve,
    Play,
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
            State::Menu => {}
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
                    // 记录接近速度（本子步开始前），用于球拍接球 +10% 增压
                    let approach_speed = g.world.bodies[g.ball.unwrap()].linvel().length();
                    let vy_before = g.world.bodies[g.ball.unwrap()].linvel().y;

                    g.world.step_with_events(&(), &g.event_handler);

                    // 球-球拍新接触（Started）：按 1.1× 接近速度重设出射速度。
                    // 方向保留求解器结果（由接触面局部法线决定），只改大小。
                    let mut hit_paddle = false;
                    while let Ok(ev) = g.collision_recv.try_recv() {
                        if let CollisionEvent::Started(c1, c2, _) = ev {
                            if (c1 == g.ball_collider && c2 == g.paddle_collider)
                                || (c1 == g.paddle_collider && c2 == g.ball_collider)
                            {
                                hit_paddle = true;
                            }
                        }
                    }
                    // vy_before < 0：必须是向下接近球拍（发球时球向上离开，不触发）
                    if hit_paddle && vy_before < 0.0 && approach_speed > BOOST_MIN_APPROACH {
                        let body = g.world.bodies.get_mut(g.ball.unwrap()).unwrap();
                        let v = body.linvel();
                        if v.length() > 0.0 {
                            body.set_linvel(v.normalize() * (approach_speed * PADDLE_BOOST), true);
                        }
                    }

                    g.accumulator -= DT;
                }

                // 低速归零；若球已落在球拍附近则回到发球状态。
                let body = g.world.bodies.get_mut(g.ball.unwrap()).unwrap();
                if body.linvel().length() < SETTLE_SPEED && body.angvel().abs() < SETTLE_ANGVEL {
                    if body.translation().y > PADDLE_Y - 60.0 {
                        g.state = State::Serve;
                    }
                }

                // 球落出画面：重置发球（生命系统后续再加）
                if g.state == State::Play
                    && g.world.bodies[g.ball.unwrap()].translation().y < LOSS_Y
                {
                    reset_to_serve(g);
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
