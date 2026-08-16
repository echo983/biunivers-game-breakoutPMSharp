mod mesh;
mod renderer;

use std::cell::RefCell;

use rapier2d::prelude::*;
use renderer::Renderer;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

const DT: f64 = 1.0 / 60.0;
const GRAVITY: f32 = -1200.0;
const BALL_RADIUS: f32 = 12.0;
const WALL_THICKNESS: f32 = 40.0;
const RESTITUTION: f32 = 0.68;
const FRICTION: f32 = 0.5;
const SETTLE_SPEED: f32 = 2.0;
const SETTLE_ANGVEL: f32 = 0.1;

struct Game {
    world: PhysicsWorld,
    ball: RigidBodyHandle,
    walls: Vec<RigidBodyHandle>,
    ctx: Option<CanvasRenderingContext2d>,
    renderer: Option<Renderer>,
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

    // 地面（固定）
    walls.push(
        world
            .insert(
                RigidBodyBuilder::fixed()
                    .translation(Vec2::new(width * 0.5, -WALL_THICKNESS * 0.5)),
                ColliderBuilder::cuboid(width * 0.5 + WALL_THICKNESS, WALL_THICKNESS * 0.5)
                    .friction(FRICTION),
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
                    .friction(FRICTION),
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
                    .friction(FRICTION),
            )
            .0,
    );

    walls
}

fn init_world(width: f32, height: f32) -> (PhysicsWorld, RigidBodyHandle, Vec<RigidBodyHandle>) {
    let mut world = PhysicsWorld::new();
    world.gravity = Vec2::new(0.0, GRAVITY);
    world.integration_parameters.dt = DT as f32;

    let walls = insert_bounds(&mut world, width, height);

    let ball = world
        .insert(
            RigidBodyBuilder::dynamic()
                .translation(Vec2::new(width * 0.5, height * 0.7))
                .linvel(Vec2::new(120.0, 0.0))
                .linear_damping(0.2)
                .angular_damping(0.9),
            ColliderBuilder::ball(BALL_RADIUS)
                .restitution(RESTITUTION)
                .friction(FRICTION)
                .density(1.0),
        )
        .0;

    (world, ball, walls)
}

#[wasm_bindgen]
pub fn render_backend() -> u32 {
    1 // WebGPU
}

#[wasm_bindgen]
pub fn hosting_mode() -> u32 {
    0 // hosted
}

#[wasm_bindgen]
pub fn setup(ctx: CanvasRenderingContext2d, width: f64, height: f64) {
    let width = width as f32;
    let height = height as f32;
    let (world, ball, walls) = init_world(width, height);

    GAME.with(|slot| {
        *slot.borrow_mut() = Some(Game {
            world,
            ball,
            walls,
            ctx: Some(ctx),
            renderer: None,
            width,
            height,
            accumulator: 0.0,
            paused: false,
        });
    });
}

#[wasm_bindgen]
pub async fn setup_gpu(canvas: HtmlCanvasElement, width: f64, height: f64) -> bool {
    let width = width as f32;
    let height = height as f32;
    let renderer = Renderer::new(canvas, width, height).await;

    match renderer {
        Ok(renderer) => {
            let (world, ball, walls) = init_world(width, height);
            GAME.with(|slot| {
                *slot.borrow_mut() = Some(Game {
                    world,
                    ball,
                    walls,
                    ctx: None,
                    renderer: Some(renderer),
                    width,
                    height,
                    accumulator: 0.0,
                    paused: false,
                });
            });
            true
        }
        Err(_) => false,
    }
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

        let body = g.world.bodies.get_mut(g.ball).unwrap();
        let mut pos = body.translation();
        pos.x = pos.x.clamp(BALL_RADIUS, width - BALL_RADIUS);
        pos.y = pos.y.clamp(BALL_RADIUS, height - BALL_RADIUS);
        body.set_translation(pos, true);

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

        g.accumulator += dt;
        if g.accumulator > 0.25 {
            g.accumulator = 0.25;
        }
        while g.accumulator >= DT {
            g.world.step();
            g.accumulator -= DT;
        }

        // 低速时归零，模拟滚动摩擦，让小球真正停下。
        let body = g.world.bodies.get_mut(g.ball).unwrap();
        if body.linvel().length() < SETTLE_SPEED && body.angvel().abs() < SETTLE_ANGVEL {
            body.set_linvel(Vec2::ZERO, false);
            body.set_angvel(0.0, false);
        }
    });
}

#[wasm_bindgen]
pub fn render() {
    with_game(|g| {
        let pos = g.world.bodies[g.ball].translation();

        if let Some(renderer) = g.renderer.as_mut() {
            renderer.update_ball(pos.x, pos.y, BALL_RADIUS);
            renderer.render();
            return;
        }

        if let Some(ctx) = g.ctx.as_ref() {
            let width = g.width as f64;
            let height = g.height as f64;
            let radius = BALL_RADIUS as f64;

            ctx.clear_rect(0.0, 0.0, width, height);

            // 底部地面条带
            ctx.set_fill_style_str("rgb(90, 106, 138)");
            ctx.fill_rect(0.0, height - 4.0, width, 4.0);

            let angle = g.world.bodies[g.ball].rotation().angle() as f64;
            let sx = pos.x as f64;
            let sy = height - pos.y as f64;

            // 小球
            ctx.begin_path();
            let _ = ctx.arc(sx, sy, radius, 0.0, std::f64::consts::TAU);
            ctx.set_fill_style_str("rgb(255, 207, 92)");
            ctx.fill();

            // 旋转标记
            let marker_offset = radius * 0.55;
            let mx = sx + angle.cos() * marker_offset;
            let my = sy - angle.sin() * marker_offset;

            ctx.begin_path();
            let _ = ctx.arc(mx, my, radius * 0.3, 0.0, std::f64::consts::TAU);
            ctx.set_fill_style_str("rgb(92, 64, 18)");
            ctx.fill();
        }
    });
}

#[wasm_bindgen]
pub fn key(code: &str, down: bool) {
    let _ = (code, down);
}

#[wasm_bindgen]
pub fn pointer_down(x: f64, y: f64, buttons: u32) {
    let _ = (x, y, buttons);
}

#[wasm_bindgen]
pub fn pointer_up(x: f64, y: f64, buttons: u32) {
    let _ = (x, y, buttons);
}

#[wasm_bindgen]
pub fn pointer_move(x: f64, y: f64, buttons: u32) {
    let _ = (x, y, buttons);
}

#[wasm_bindgen]
pub fn wheel(dx: f64, dy: f64) {
    let _ = (dx, dy);
}

#[wasm_bindgen]
pub fn set_paused(paused: bool) {
    with_game(|g| g.paused = paused);
}

#[wasm_bindgen]
pub fn destroy() {}
