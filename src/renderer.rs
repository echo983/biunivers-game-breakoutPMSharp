use std::mem::size_of;

use wgpu::util::DeviceExt;
use web_sys::HtmlCanvasElement;

use crate::mesh::{
    generate_skateboard, generate_unit_box, generate_unit_hemisphere, generate_unit_sphere, Mesh,
    Vertex,
};

const WGSL: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
    light: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct ObjectUniform {
    model: mat4x4<f32>,
    color: vec4<f32>,
};

@group(1) @binding(0) var<uniform> object: ObjectUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = object.model * vec4<f32>(in.position, 1.0);
    out.clip_position = camera.view_proj * world_pos;
    let model3 = mat3x3<f32>(object.model[0].xyz, object.model[1].xyz, object.model[2].xyz);
    out.world_normal = model3 * in.normal;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let l = normalize(-camera.light.xyz);
    let diff = max(dot(n, l), 0.0);
    let ambient = camera.light.w;
    let shade = ambient + (1.0 - ambient) * diff;
    return vec4<f32>(object.color.rgb * shade, object.color.a);
}
"#;

const BALL_COLOR: [f32; 4] = [1.0, 0.81, 0.36, 1.0];
const WALL_COLOR: [f32; 4] = [0.35, 0.42, 0.55, 1.0];
const GROUND_THICKNESS: f32 = 8.0;
const DEPTH: f32 = 120.0;

// 球拍类型（与 lib.rs 保持一致）：0=滑板(cube→skateboard) 1=橄榄球(sphere) 2=碗(hemisphere)
pub const PADDLE_SKATE: u8 = 0;
pub const PADDLE_RUGBY: u8 = 1;
pub const PADDLE_BOWL: u8 = 2;
const PADDLE_COLORS: [[f32; 4]; 3] = [
    [1.0, 0.55, 0.2, 1.0],   // 滑板：橙
    [0.72, 0.45, 0.25, 1.0], // 橄榄球：棕
    [0.3, 0.6, 0.9, 0.6],    // 碗：蓝，半透明——球进入碗内时可见
];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    light: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ObjectUniform {
    model: [[f32; 4]; 4],
    color: [f32; 4],
}

struct MeshBuffers {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

struct Object {
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// 一块砖的渲染数据：对象 + 颜色/尺寸（供 resize 时重新定位）。
struct BrickRender {
    object: Object,
    color: [f32; 4],
    half_w: f32,
    half_h: f32,
}

pub struct Renderer {
    canvas: HtmlCanvasElement,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    object_layout: wgpu::BindGroupLayout,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    sphere: MeshBuffers,
    cube: MeshBuffers,
    hemisphere: MeshBuffers,
    skateboard: MeshBuffers,
    ball: Object,
    paddle: Object,
    paddle_kind: u8,
    ceiling: Object,
    left_wall: Object,
    right_wall: Object,
    bricks: Vec<Option<BrickRender>>,
    camera: CameraUniform,
    css_width: f32,
    css_height: f32,
}

impl Renderer {
    pub async fn new(canvas: HtmlCanvasElement, css_width: f32, css_height: f32) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

        // 先请求 adapter/device，再创建 surface：失败时 canvas 未被占用，便于回退到 2D。
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .map_err(|e| format!("request_adapter: {e:?}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| format!("request_device: {e:?}"))?;

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| format!("create_surface: {e:?}"))?;

        let config = surface
            .get_default_config(&adapter, canvas.width().max(1), canvas.height().max(1))
            .ok_or_else(|| "no default surface configuration".to_string())?;
        surface.configure(&device, &config);

        let depth_view = create_depth_texture(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });

        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let object_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("object"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline"),
            bind_group_layouts: &[Some(&camera_layout), Some(&object_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    // Alpha 混合：不透明物体 alpha=1 时等价于覆盖；碗用 alpha<1 显示透明。
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bind_group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let sphere = create_mesh_buffers(&device, &generate_unit_sphere(24, 32));
        let cube = create_mesh_buffers(&device, &generate_unit_box());
        let hemisphere = create_mesh_buffers(&device, &generate_unit_hemisphere(24, 32));
        let skateboard = create_mesh_buffers(&device, &generate_skateboard());

        let ball = create_object(&device, &object_layout);
        let paddle = create_object(&device, &object_layout);
        let ceiling = create_object(&device, &object_layout);
        let left_wall = create_object(&device, &object_layout);
        let right_wall = create_object(&device, &object_layout);

        let mut renderer = Self {
            canvas,
            device,
            queue,
            surface,
            config,
            depth_view,
            pipeline,
            object_layout,
            camera_buffer,
            camera_bind_group,
            sphere,
            cube,
            hemisphere,
            skateboard,
            ball,
            paddle,
            paddle_kind: 0,
            ceiling,
            left_wall,
            right_wall,
            bricks: Vec::new(),
            camera: CameraUniform {
                view_proj: [[0.0; 4]; 4],
                light: [0.4, -0.6, 0.7, 0.35],
            },
            css_width,
            css_height,
        };

        renderer.update_camera();
        renderer.write_static_objects();

        Ok(renderer)
    }

    pub fn resize(&mut self, css_width: f32, css_height: f32) {
        self.css_width = css_width;
        self.css_height = css_height;
        self.config.width = self.canvas.width().max(1);
        self.config.height = self.canvas.height().max(1);
        self.surface.configure(&self.device, &self.config);
        self.depth_view = create_depth_texture(&self.device, &self.config);
        self.update_camera();
        self.write_static_objects();
    }

    pub fn update_ball(&mut self, x: f32, y: f32, radius: f32) {
        let model = glam::Mat4::from_scale_rotation_translation(
            glam::Vec3::splat(radius),
            glam::Quat::IDENTITY,
            glam::Vec3::new(x, 0.0, y),
        );
        let uniform = ObjectUniform {
            model: model.to_cols_array_2d(),
            color: BALL_COLOR,
        };
        self.queue
            .write_buffer(&self.ball.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub fn update_paddle(&mut self, x: f32, y: f32, kind: u8, half_w: f32, half_h: f32) {
        self.paddle_kind = kind;
        let model = match kind {
            // 橄榄球/碗：单位半径网格直接缩放为半宽/半高
            PADDLE_RUGBY | PADDLE_BOWL => glam::Mat4::from_scale_rotation_translation(
                glam::Vec3::new(half_w, DEPTH, half_h),
                glam::Quat::IDENTITY,
                glam::Vec3::new(x, 0.0, y),
            ),
            // 滑板：组合网格 x∈[-1,1]，板长=半宽、厚度=DEPTH、板厚=半高×2
            PADDLE_SKATE => glam::Mat4::from_scale_rotation_translation(
                glam::Vec3::new(half_w, DEPTH, half_h * 2.0),
                glam::Quat::IDENTITY,
                glam::Vec3::new(x, 0.0, y),
            ),
            _ => glam::Mat4::from_scale_rotation_translation(
                glam::Vec3::new(half_w * 2.0, DEPTH, half_h * 2.0),
                glam::Quat::IDENTITY,
                glam::Vec3::new(x, 0.0, y),
            ),
        };
        let uniform = ObjectUniform {
            model: model.to_cols_array_2d(),
            color: PADDLE_COLORS[kind as usize],
        };
        self.queue
            .write_buffer(&self.paddle.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    /// 清空全部砖块（重建关卡时调用）。
    pub fn clear_bricks(&mut self) {
        self.bricks.clear();
    }

    /// 添加一块砖（cube 网格），返回其索引供 hide_brick 使用。
    pub fn add_brick(&mut self, x: f32, y: f32, half_w: f32, half_h: f32, color: [f32; 4]) -> usize {
        let uniform = ObjectUniform {
            model: brick_model(x, y, half_w, half_h).to_cols_array_2d(),
            color,
        };
        let object = create_object_with_uniform(&self.device, &self.object_layout, &self.queue, &uniform);
        self.bricks.push(Some(BrickRender {
            object,
            color,
            half_w,
            half_h,
        }));
        self.bricks.len() - 1
    }

    /// 隐藏（破坏）某块砖：保留占位，绘制时跳过。
    pub fn hide_brick(&mut self, index: usize) {
        if let Some(slot) = self.bricks.get_mut(index) {
            *slot = None;
        }
    }

    /// 重新定位某块砖（窗口 resize 时按新布局刷新物理与渲染位置）。
    pub fn move_brick(&mut self, index: usize, x: f32, y: f32, half_w: f32, half_h: f32) {
        if let Some(Some(brick)) = self.bricks.get_mut(index) {
            brick.half_w = half_w;
            brick.half_h = half_h;
            let uniform = ObjectUniform {
                model: brick_model(x, y, half_w, half_h).to_cols_array_2d(),
                color: brick.color,
            };
            self.queue.write_buffer(
                &brick.object.uniform_buffer,
                0,
                bytemuck::bytes_of(&uniform),
            );
        }
    }

    fn update_camera(&mut self) {
        let aspect = if self.css_height > 0.0 {
            self.css_width / self.css_height
        } else {
            1.0
        };
        let fov_y = 50.0_f32.to_radians();
        let cam_dist = (self.css_width + self.css_height) * 0.6;

        // 取景：以游玩区（球拍 y≈50 到顶排砖块 y≈height-17）为中心。
        // target/eye 取 height 的 0.53/0.55，确保默认与最小窗口下砖块区均完整可见
        //（此前 0.35 会让顶排硬砖在最小窗口完全出屏，见 0.5.2 提交说明）。
        let eye = glam::Vec3::new(self.css_width * 0.5, -cam_dist, self.css_height * 0.55);
        let target = glam::Vec3::new(self.css_width * 0.5, 0.0, self.css_height * 0.53);
        let view = glam::camera::rh::view::look_at_mat4(eye, target, glam::Vec3::Z);
        let proj = glam::camera::rh::proj::directx::perspective(fov_y, aspect, 0.1, 2000.0);

        let light_dir = glam::Vec3::new(0.4, -0.6, 0.7).normalize();
        self.camera.view_proj = (proj * view).to_cols_array_2d();
        self.camera.light = [light_dir.x, light_dir.y, light_dir.z, 0.35];
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&self.camera));
    }

    fn write_static_objects(&mut self) {
        let w = self.css_width;
        let h = self.css_height;
        let depth = DEPTH;

        let ceiling_model = glam::Mat4::from_scale_rotation_translation(
            glam::Vec3::new(w + 2.0 * GROUND_THICKNESS, depth, GROUND_THICKNESS),
            glam::Quat::IDENTITY,
            glam::Vec3::new(w * 0.5, 0.0, h + GROUND_THICKNESS * 0.5),
        );
        let left_model = glam::Mat4::from_scale_rotation_translation(
            glam::Vec3::new(GROUND_THICKNESS, depth, h + 2.0 * GROUND_THICKNESS),
            glam::Quat::IDENTITY,
            glam::Vec3::new(-GROUND_THICKNESS * 0.5, 0.0, h * 0.5),
        );
        let right_model = glam::Mat4::from_scale_rotation_translation(
            glam::Vec3::new(GROUND_THICKNESS, depth, h + 2.0 * GROUND_THICKNESS),
            glam::Quat::IDENTITY,
            glam::Vec3::new(w + GROUND_THICKNESS * 0.5, 0.0, h * 0.5),
        );

        write_object(&self.queue, &mut self.ceiling, ceiling_model);
        write_object(&self.queue, &mut self.left_wall, left_model);
        write_object(&self.queue, &mut self.right_wall, right_model);
    }

    pub fn render(&mut self) {
        let tex = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => return,
        };

        let view = tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08,
                            g: 0.10,
                            b: 0.14,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            // 小球（球体）
            render_pass.set_bind_group(1, &self.ball.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.sphere.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.sphere.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.sphere.index_count, 0, 0..1);

            // 球拍（按类型选网格：滑板=立方体，橄榄球=球体，碗=半球）
            render_pass.set_bind_group(1, &self.paddle.bind_group, &[]);
            match self.paddle_kind {
                PADDLE_RUGBY => {
                    render_pass.set_vertex_buffer(0, self.sphere.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(self.sphere.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..self.sphere.index_count, 0, 0..1);
                }
                PADDLE_BOWL => {
                    render_pass.set_vertex_buffer(0, self.hemisphere.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(self.hemisphere.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..self.hemisphere.index_count, 0, 0..1);
                }
                // 滑板：组合网格（板面 + 翘头 + 轮子）
                PADDLE_SKATE => {
                    render_pass.set_vertex_buffer(0, self.skateboard.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(self.skateboard.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..self.skateboard.index_count, 0, 0..1);
                }
                // 未知类型兜底
                _ => {
                    render_pass.set_vertex_buffer(0, self.cube.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(self.cube.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..self.cube.index_count, 0, 0..1);
                }
            }

            // 天花板与侧墙（立方体）
            render_pass.set_vertex_buffer(0, self.cube.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.cube.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            for object in [&self.ceiling, &self.left_wall, &self.right_wall] {
                render_pass.set_bind_group(1, &object.bind_group, &[]);
                render_pass.draw_indexed(0..self.cube.index_count, 0, 0..1);
            }

            // 砖块（立方体，隐藏的跳过）
            for brick in self.bricks.iter().flatten() {
                render_pass.set_bind_group(1, &brick.object.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.cube.vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.cube.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..self.cube.index_count, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(tex);
    }
}

/// 砖块的模型矩阵：宽高以 px 计的扁平立方体。
fn brick_model(x: f32, y: f32, half_w: f32, half_h: f32) -> glam::Mat4 {
    glam::Mat4::from_scale_rotation_translation(
        glam::Vec3::new(half_w * 2.0, DEPTH, half_h * 2.0),
        glam::Quat::IDENTITY,
        glam::Vec3::new(x, 0.0, y),
    )
}

fn create_depth_texture(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24Plus,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn create_uniform_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("object_uniform"),
        size: size_of::<ObjectUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_object_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("object_bind_group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}

fn create_object(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Object {
    let uniform_buffer = create_uniform_buffer(device);
    let bind_group = create_object_bind_group(device, layout, &uniform_buffer);
    Object {
        uniform_buffer,
        bind_group,
    }
}

fn create_object_with_uniform(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    queue: &wgpu::Queue,
    uniform: &ObjectUniform,
) -> Object {
    let uniform_buffer = create_uniform_buffer(device);
    queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(uniform));
    let bind_group = create_object_bind_group(device, layout, &uniform_buffer);
    Object {
        uniform_buffer,
        bind_group,
    }
}

fn write_object(queue: &wgpu::Queue, object: &mut Object, model: glam::Mat4) {
    let uniform = ObjectUniform {
        model: model.to_cols_array_2d(),
        color: WALL_COLOR,
    };
    queue.write_buffer(&object.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
}

fn create_mesh_buffers(device: &wgpu::Device, mesh: &Mesh) -> MeshBuffers {
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vertices"),
        contents: bytemuck::cast_slice(&mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("indices"),
        contents: bytemuck::cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    MeshBuffers {
        vertex_buffer,
        index_buffer,
        index_count: mesh.indices.len() as u32,
    }
}
