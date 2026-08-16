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