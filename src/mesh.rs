#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

pub fn generate_unit_sphere(stacks: u32, sectors: u32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for i in 0..=stacks {
        let phi = std::f32::consts::PI * i as f32 / stacks as f32;
        let (sin_phi, cos_phi) = phi.sin_cos();
        for j in 0..=sectors {
            let theta = std::f32::consts::TAU * j as f32 / sectors as f32;
            let (sin_theta, cos_theta) = theta.sin_cos();
            let position = [sin_phi * cos_theta, sin_phi * sin_theta, cos_phi];
            vertices.push(Vertex {
                position,
                normal: position,
            });
        }
    }

    for i in 0..stacks {
        for j in 0..sectors {
            let a = i * (sectors + 1) + j;
            let b = a + sectors + 1;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }

    Mesh { vertices, indices }
}

pub fn generate_unit_box() -> Mesh {
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([0.0, 0.0, 1.0], [[-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5]]),
        ([0.0, 0.0, -1.0], [[-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [-0.5, 0.5, -0.5]]),
        ([1.0, 0.0, 0.0], [[0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5]]),
        ([-1.0, 0.0, 0.0], [[-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5]]),
        ([0.0, 1.0, 0.0], [[-0.5, 0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5]]),
        ([0.0, -1.0, 0.0], [[-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5]]),
    ];

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for (normal, corners) in faces.iter() {
        let base = vertices.len() as u32;
        for corner in corners.iter() {
            vertices.push(Vertex {
                position: *corner,
                normal: *normal,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Mesh { vertices, indices }
}

/// 生成开口朝上的凹面半球壳（碗）：碗口在 y=0 平面，底部向下凸出至 y=-1。
/// 法线取反向（指向球心），让凹腔内表面受光可见。
pub fn generate_unit_hemisphere(stacks: u32, sectors: u32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for i in 0..=stacks {
        let phi = std::f32::consts::FRAC_PI_2 * i as f32 / stacks as f32;
        let (sin_phi, cos_phi) = phi.sin_cos();
        for j in 0..=sectors {
            let theta = std::f32::consts::TAU * j as f32 / sectors as f32;
            let (sin_theta, cos_theta) = theta.sin_cos();
            // y = -cos(phi)：i=0 时底部 (-1)，i=stacks 时碗口 (0)
            let position = [sin_phi * cos_theta, -cos_phi, sin_phi * sin_theta];
            vertices.push(Vertex {
                position,
                normal: [-position[0], -position[1], -position[2]],
            });
        }
    }

    for i in 0..stacks {
        for j in 0..sectors {
            let a = i * (sectors + 1) + j;
            let b = a + sectors + 1;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }

    Mesh { vertices, indices }
}
/// 生成滑板组合网格：带翘头/翘尾的板面 + 4 个轮子，合并为单一 Mesh（共享同一变换）。
/// 单位空间：x∈[-1,1] 沿板长，z 向上，y 为厚度方向。
pub fn generate_skateboard() -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // ── 板面：沿 x 扫掠矩形，两端翘起（nose/tail kickup）──
    const N: usize = 20;
    const TY: f32 = 0.22; // 半厚度（深度方向）
    const TZ: f32 = 0.25; // 半厚度（垂直方向）
    const KICK_START: f32 = 0.55; // 翘起起始位置
    const KICK_RISE: f32 = 0.42; // 翘起高度

    let mut stations: Vec<Vec<[f32; 3]>> = Vec::with_capacity(N);
    for i in 0..N {
        let x = -1.0 + 2.0 * i as f32 / (N as f32 - 1.0);
        let ax = x.abs();
        let kick = if ax > KICK_START { (ax - KICK_START) / (1.0 - KICK_START) } else { 0.0 };
        let zc = kick * kick * KICK_RISE;
        // 板面中心线切向（在 x-z 平面），决定截面朝向
        let dz = if ax > KICK_START {
            2.0 * (ax - KICK_START) / (1.0 - KICK_START) * KICK_RISE * x.signum()
        } else {
            0.0
        };
        let tlen = (1.0f32 + dz * dz).sqrt();
        let tx = 1.0 / tlen;
        let tz = dz / tlen;
        // 截面局部 up 向量（垂直于切向）
        let ux = -tz;
        let uz = tx;
        let c = [x, 0.0, zc];
        // 四个角：±TY（深度）、±TZ（沿 up 方向）
        let corners = [
            [c[0] + ux * TZ, TY, c[2] + uz * TZ], // +y +z
            [c[0] + ux * TZ, -TY, c[2] + uz * TZ], // -y +z
            [c[0] - ux * TZ, -TY, c[2] - uz * TZ], // -y -z
            [c[0] - ux * TZ, TY, c[2] - uz * TZ], // +y -z
        ];
        stations.push(corners.to_vec());
    }
    // 连接相邻站，生成侧面/顶/底面
    let mut push_quad = |vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]| {
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ad = [d[0] - a[0], d[1] - a[1], d[2] - a[2]];
        let n = [
            ab[1] * ad[2] - ab[2] * ad[1],
            ab[2] * ad[0] - ab[0] * ad[2],
            ab[0] * ad[1] - ab[1] * ad[0],
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
        let n = [n[0] / len, n[1] / len, n[2] / len];
        let base = vertices.len() as u32;
        vertices.push(Vertex { position: a, normal: n });
        vertices.push(Vertex { position: b, normal: n });
        vertices.push(Vertex { position: c, normal: n });
        vertices.push(Vertex { position: d, normal: n });
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    for i in 0..N - 1 {
        let s0 = &stations[i];
        let s1 = &stations[i + 1];
        // 顶面（+z 角 0/1）
        push_quad(&mut vertices, &mut indices, s0[0], s1[0], s1[1], s0[1]);
        // 底面（-z 角 2/3）
        push_quad(&mut vertices, &mut indices, s0[3], s1[3], s1[2], s0[2]);
        // +y 侧面（角 0/3）
        push_quad(&mut vertices, &mut indices, s0[0], s0[3], s1[3], s1[0]);
        // -y 侧面（角 1/2）
        push_quad(&mut vertices, &mut indices, s0[1], s1[1], s1[2], s0[2]);
    }
    // 两端封口（首/末站四边形）
    for (idx, order) in [(0usize, [0usize, 1, 2, 3]), (N - 1, [0, 3, 2, 1])] {
        let s = &stations[idx];
        let quad = order.map(|k| s[k]);
        push_quad(&mut vertices, &mut indices, quad[0], quad[1], quad[2], quad[3]);
    }

    // ── 轮子：4 个小球体，位于板两端下方 ──
    let wheel = generate_unit_sphere(10, 14);
    for wx in [-0.66f32, 0.66] {
        for wy in [-0.30f32, 0.30] {
            // 轮心：板面下方
            let cx = wx;
            let cy = wy;
            let cz = -0.46;
            let base = vertices.len() as u32;
            for v in &wheel.vertices {
                let r = 0.16f32;
                let p = v.position;
                vertices.push(Vertex {
                    position: [cx + p[0] * r, cy + p[1] * r, cz + p[2] * r],
                    normal: [p[0], p[1], p[2]],
                });
            }
            for idx in &wheel.indices {
                indices.push(base + *idx);
            }
        }
    }

    Mesh { vertices, indices }
}
