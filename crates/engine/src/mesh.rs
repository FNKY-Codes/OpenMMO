use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;

#[derive(Debug, Clone)]
pub struct Mesh {
    pub faces: Vec<MeshFace>,
    pub height: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct MeshFace {
    pub positions: [[f32; 3]; 3],
    pub color: [f32; 4],
}

#[derive(Debug, Default)]
pub struct ModelCache {
    models: HashMap<String, Mesh>,
    assets_dir: PathBuf,
}

impl ModelCache {
    pub fn new(assets_dir: PathBuf) -> Self {
        Self {
            models: HashMap::new(),
            assets_dir,
        }
    }

    pub fn get(&mut self, name: &str) -> Option<&Mesh> {
        if !self.models.contains_key(name) {
            if let Ok(mesh) = load_obj_model(&self.assets_dir.join("models").join(format!("{name}.obj"))) {
                self.models.insert(name.to_string(), mesh);
            }
        }
        self.models.get(name)
    }
}

pub fn default_assets_dir() -> PathBuf {
    std::env::var("OPENMMO_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("assets"))
}

fn load_obj_model(path: &Path) -> anyhow::Result<Mesh> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read OBJ {}", path.display()))?;
    let mtl_path = path.with_extension("mtl");
    let materials = if mtl_path.exists() {
        load_mtl(&mtl_path)?
    } else {
        HashMap::new()
    };

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut faces: Vec<MeshFace> = Vec::new();
    let mut current_color = [0.7, 0.7, 0.7, 1.0];

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(tag) = parts.next() else { continue };
        match tag {
            "v" => {
                let x: f32 = parts.next().context("vertex x")?.parse()?;
                let y: f32 = parts.next().context("vertex y")?.parse()?;
                let z: f32 = parts.next().context("vertex z")?.parse()?;
                positions.push([x, y, z]);
            }
            "vn" => {
                let x: f32 = parts.next().context("normal x")?.parse()?;
                let y: f32 = parts.next().context("normal y")?.parse()?;
                let z: f32 = parts.next().context("normal z")?.parse()?;
                normals.push([x, y, z]);
            }
            "usemtl" => {
                if let Some(name) = parts.next() {
                    current_color = materials
                        .get(name)
                        .copied()
                        .unwrap_or([0.7, 0.7, 0.7, 1.0]);
                }
            }
            "f" => {
                let mut face_verts: Vec<([f32; 3], [f32; 3])> = Vec::new();
                for corner in parts {
                    let (vi, ni) = parse_face_corner(corner)?;
                    let pos = positions
                        .get(vi.saturating_sub(1))
                        .copied()
                        .context("vertex index out of range")?;
                    let normal = ni
                        .and_then(|idx| normals.get(idx.saturating_sub(1)).copied())
                        .unwrap_or([0.0, 1.0, 0.0]);
                    face_verts.push((pos, normal));
                }
                if face_verts.len() >= 3 {
                    for i in 1..face_verts.len() - 1 {
                        faces.push(tri_from_corners(
                            face_verts[0],
                            face_verts[i],
                            face_verts[i + 1],
                            current_color,
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    normalize_mesh(&mut faces);
    let height = faces
        .iter()
        .flat_map(|f| f.positions)
        .map(|p| p[1])
        .fold(0.0_f32, f32::max);

    Ok(Mesh { faces, height })
}

fn parse_face_corner(corner: &str) -> anyhow::Result<(usize, Option<usize>)> {
    let mut indices = corner.split('/');
    let vi: usize = indices
        .next()
        .context("face vertex")?
        .parse()
        .context("face vertex index")?;
    let _ti = indices.next();
    let ni = indices.next().and_then(|s| s.parse().ok());
    Ok((vi, ni))
}

fn tri_from_corners(
    a: ([f32; 3], [f32; 3]),
    b: ([f32; 3], [f32; 3]),
    c: ([f32; 3], [f32; 3]),
    base_color: [f32; 4],
) -> MeshFace {
    let shade = (normal_shade(a.1) + normal_shade(b.1) + normal_shade(c.1)) / 3.0;
    MeshFace {
        positions: [a.0, b.0, c.0],
        color: shade_color(base_color, shade),
    }
}

fn normal_shade(normal: [f32; 3]) -> f32 {
    0.55 + 0.45 * normal[1].max(0.0)
}

fn shade_color(base: [f32; 4], shade: f32) -> [f32; 4] {
    [
        (base[0] * shade).min(1.0),
        (base[1] * shade).min(1.0),
        (base[2] * shade).min(1.0),
        base[3],
    ]
}

fn normalize_mesh(faces: &mut [MeshFace]) {
    if faces.is_empty() {
        return;
    }
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for face in faces.iter() {
        for pos in &face.positions {
            for axis in 0..3 {
                min[axis] = min[axis].min(pos[axis]);
                max[axis] = max[axis].max(pos[axis]);
            }
        }
    }
    let size = [
        max[0] - min[0],
        max[1] - min[1],
        max[2] - min[2],
    ];
    let max_dim = size[0].max(size[1]).max(size[2]).max(0.001);
    let target_height = 1.2;
    let scale = target_height / max_dim;
    let center_x = (min[0] + max[0]) * 0.5;
    let center_z = (min[2] + max[2]) * 0.5;
    let base_y = min[1];

    for face in faces.iter_mut() {
        for pos in &mut face.positions {
            pos[0] = (pos[0] - center_x) * scale;
            pos[1] = (pos[1] - base_y) * scale;
            pos[2] = (pos[2] - center_z) * scale;
        }
    }
}

fn load_mtl(path: &Path) -> anyhow::Result<HashMap<String, [f32; 4]>> {
    let text = std::fs::read_to_string(path)?;
    let mut materials = HashMap::new();
    let mut current = String::new();
    let mut kd = [0.7, 0.7, 0.7];

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(tag) = parts.next() else { continue };
        match tag {
            "newmtl" => {
                if !current.is_empty() {
                    materials.insert(current.clone(), [kd[0], kd[1], kd[2], 1.0]);
                }
                current = parts.next().unwrap_or("default").to_string();
            }
            "Kd" => {
                kd[0] = parts.next().and_then(|v| v.parse().ok()).unwrap_or(kd[0]);
                kd[1] = parts.next().and_then(|v| v.parse().ok()).unwrap_or(kd[1]);
                kd[2] = parts.next().and_then(|v| v.parse().ok()).unwrap_or(kd[2]);
            }
            _ => {}
        }
    }
    if !current.is_empty() {
        materials.insert(current, [kd[0], kd[1], kd[2], 1.0]);
    }
    Ok(materials)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_frog_model() {
        let path = PathBuf::from("assets/models/frog.obj");
        if !path.exists() {
            return;
        }
        let mesh = load_obj_model(&path).expect("frog model");
        assert!(!mesh.faces.is_empty());
        assert!(mesh.height > 0.0);
    }
}
