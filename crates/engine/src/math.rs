use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }

    pub fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }

    pub fn scale(self, s: f32) -> Self {
        Self {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len < 1e-8 {
            return Self::ZERO;
        }
        self.scale(1.0 / len)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Mat4 {
    pub cols: [[f32; 4]; 4],
}

impl Mat4 {
    pub fn identity() -> Self {
        Self {
            cols: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y / 2.0).tan();
        let nf = 1.0 / (near - far);
        Self {
            cols: [
                [f / aspect, 0.0, 0.0, 0.0],
                [0.0, f, 0.0, 0.0],
                [0.0, 0.0, (far + near) * nf, -1.0],
                [0.0, 0.0, 2.0 * far * near * nf, 0.0],
            ],
        }
    }

    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let f = target.sub(eye).normalize();
        let s = f
            .cross(up)
            .normalize();
        let u = s.cross(f);

        Self {
            cols: [
                [s.x, u.x, -f.x, 0.0],
                [s.y, u.y, -f.y, 0.0],
                [s.z, u.z, -f.z, 0.0],
                [
                    -s.x * eye.x - s.y * eye.y - s.z * eye.z,
                    -u.x * eye.x - u.y * eye.y - u.z * eye.z,
                    f.x * eye.x + f.y * eye.y + f.z * eye.z,
                    1.0,
                ],
            ],
        }
    }

    pub fn mul(self, other: Self) -> Self {
        let mut out = Self::identity();
        for c in 0..4 {
            for r in 0..4 {
                out.cols[c][r] = self.cols[0][r] * other.cols[c][0]
                    + self.cols[1][r] * other.cols[c][1]
                    + self.cols[2][r] * other.cols[c][2]
                    + self.cols[3][r] * other.cols[c][3];
            }
        }
        out
    }

    pub fn transform_point(self, p: Vec3) -> (Vec3, f32) {
        let x = self.cols[0][0] * p.x
            + self.cols[1][0] * p.y
            + self.cols[2][0] * p.z
            + self.cols[3][0];
        let y = self.cols[0][1] * p.x
            + self.cols[1][1] * p.y
            + self.cols[2][1] * p.z
            + self.cols[3][1];
        let z = self.cols[0][2] * p.x
            + self.cols[1][2] * p.y
            + self.cols[2][2] * p.z
            + self.cols[3][2];
        let w = self.cols[0][3] * p.x
            + self.cols[1][3] * p.y
            + self.cols[2][3] * p.z
            + self.cols[3][3];
        (Vec3::new(x, y, z), w)
    }

    pub fn inverse(self) -> Option<Self> {
        let m = self.cols;
        let mut inv = [[0.0f32; 4]; 4];

        inv[0][0] = m[1][1] * m[2][2] * m[3][3] - m[1][1] * m[2][3] * m[3][2]
            - m[2][1] * m[1][2] * m[3][3]
            + m[2][1] * m[1][3] * m[3][2]
            + m[3][1] * m[1][2] * m[2][3]
            - m[3][1] * m[1][3] * m[2][2];
        inv[0][1] = -m[0][1] * m[2][2] * m[3][3]
            + m[0][1] * m[2][3] * m[3][2]
            + m[2][1] * m[0][2] * m[3][3]
            - m[2][1] * m[0][3] * m[3][2]
            - m[3][1] * m[0][2] * m[2][3]
            + m[3][1] * m[0][3] * m[2][2];
        inv[0][2] = m[0][1] * m[1][2] * m[3][3]
            - m[0][1] * m[1][3] * m[3][2]
            - m[1][1] * m[0][2] * m[3][3]
            + m[1][1] * m[0][3] * m[3][2]
            + m[3][1] * m[0][2] * m[1][3]
            - m[3][1] * m[0][3] * m[1][2];
        inv[0][3] = -m[0][1] * m[1][2] * m[2][3]
            + m[0][1] * m[1][3] * m[2][2]
            + m[1][1] * m[0][2] * m[2][3]
            - m[1][1] * m[0][3] * m[2][2]
            - m[2][1] * m[0][2] * m[1][3]
            + m[2][1] * m[0][3] * m[1][2];
        inv[1][0] = -m[1][0] * m[2][2] * m[3][3]
            + m[1][0] * m[2][3] * m[3][2]
            + m[2][0] * m[1][2] * m[3][3]
            - m[2][0] * m[1][3] * m[3][2]
            - m[3][0] * m[1][2] * m[2][3]
            + m[3][0] * m[1][3] * m[2][2];
        inv[1][1] = m[0][0] * m[2][2] * m[3][3]
            - m[0][0] * m[2][3] * m[3][2]
            - m[2][0] * m[0][2] * m[3][3]
            + m[2][0] * m[0][3] * m[3][2]
            + m[3][0] * m[0][2] * m[2][3]
            - m[3][0] * m[0][3] * m[2][2];
        inv[1][2] = -m[0][0] * m[1][2] * m[3][3]
            + m[0][0] * m[1][3] * m[3][2]
            + m[1][0] * m[0][2] * m[3][3]
            - m[1][0] * m[0][3] * m[3][2]
            - m[3][0] * m[0][2] * m[1][3]
            + m[3][0] * m[0][3] * m[1][2];
        inv[1][3] = m[0][0] * m[1][2] * m[2][3]
            - m[0][0] * m[1][3] * m[2][2]
            - m[1][0] * m[0][2] * m[2][3]
            + m[1][0] * m[0][3] * m[2][2]
            + m[2][0] * m[0][2] * m[1][3]
            - m[2][0] * m[0][3] * m[1][2];
        inv[2][0] = m[1][0] * m[2][1] * m[3][3]
            - m[1][0] * m[2][3] * m[3][1]
            - m[2][0] * m[1][1] * m[3][3]
            + m[2][0] * m[1][3] * m[3][1]
            + m[3][0] * m[1][1] * m[2][3]
            - m[3][0] * m[1][3] * m[2][1];
        inv[2][1] = -m[0][0] * m[2][1] * m[3][3]
            + m[0][0] * m[2][3] * m[3][1]
            + m[2][0] * m[0][1] * m[3][3]
            - m[2][0] * m[0][3] * m[3][1]
            - m[3][0] * m[0][1] * m[2][3]
            + m[3][0] * m[0][3] * m[2][1];
        inv[2][2] = m[0][0] * m[1][1] * m[3][3]
            - m[0][0] * m[1][3] * m[3][1]
            - m[1][0] * m[0][1] * m[3][3]
            + m[1][0] * m[0][3] * m[3][1]
            + m[3][0] * m[0][1] * m[1][3]
            - m[3][0] * m[0][3] * m[1][1];
        inv[2][3] = -m[0][0] * m[1][1] * m[2][3]
            + m[0][0] * m[1][3] * m[2][1]
            + m[1][0] * m[0][1] * m[2][3]
            - m[1][0] * m[0][3] * m[2][1]
            - m[2][0] * m[0][1] * m[1][3]
            + m[2][0] * m[0][3] * m[1][1];
        inv[3][0] = -m[1][0] * m[2][1] * m[3][2]
            + m[1][0] * m[2][2] * m[3][1]
            + m[2][0] * m[1][1] * m[3][2]
            - m[2][0] * m[1][2] * m[3][1]
            - m[3][0] * m[1][1] * m[2][2]
            + m[3][0] * m[1][2] * m[2][1];
        inv[3][1] = m[0][0] * m[2][1] * m[3][2]
            - m[0][0] * m[2][2] * m[3][1]
            - m[2][0] * m[0][1] * m[3][2]
            + m[2][0] * m[0][2] * m[3][1]
            + m[3][0] * m[0][1] * m[2][2]
            - m[3][0] * m[0][2] * m[2][1];
        inv[3][2] = -m[0][0] * m[1][1] * m[3][2]
            + m[0][0] * m[1][2] * m[3][1]
            + m[1][0] * m[0][1] * m[3][2]
            - m[1][0] * m[0][2] * m[3][1]
            - m[3][0] * m[0][1] * m[1][2]
            + m[3][0] * m[0][2] * m[1][1];
        inv[3][3] = m[0][0] * m[1][1] * m[2][2]
            - m[0][0] * m[1][2] * m[2][1]
            - m[1][0] * m[0][1] * m[2][2]
            + m[1][0] * m[0][2] * m[2][1]
            + m[2][0] * m[0][1] * m[1][2]
            - m[2][0] * m[0][2] * m[1][1];

        let mut det = m[0][0] * inv[0][0] + m[0][1] * inv[1][0] + m[0][2] * inv[2][0] + m[0][3] * inv[3][0];
        if det.abs() < 1e-12 {
            return None;
        }
        det = 1.0 / det;
        for c in 0..4 {
            for r in 0..4 {
                inv[c][r] *= det;
            }
        }
        Some(Self { cols: inv })
    }
}

impl Vec3 {
    fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }
}

pub fn screen_to_world_ray(
    mouse_x: f32,
    mouse_y: f32,
    width: f32,
    height: f32,
    inv_view_proj: Mat4,
) -> (Vec3, Vec3) {
    let ndc_x = (mouse_x / width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (mouse_y / height) * 2.0;

    let (near, w_near) = inv_view_proj.transform_point(Vec3::new(ndc_x, ndc_y, -1.0));
    let (far, w_far) = inv_view_proj.transform_point(Vec3::new(ndc_x, ndc_y, 1.0));
    let near = near.scale(1.0 / w_near);
    let far = far.scale(1.0 / w_far);
    let dir = far.sub(near).normalize();
    (near, dir)
}

pub fn ray_plane_y_intersection(origin: Vec3, dir: Vec3) -> Option<Vec3> {
    if dir.y.abs() < 1e-6 {
        return None;
    }
    let t = -origin.y / dir.y;
    if t < 0.0 {
        return None;
    }
    Some(origin.add(dir.scale(t)))
}

pub const DEFAULT_FOV_Y: f32 = PI / 4.0;
