use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use egui_wgpu::wgpu::util::DeviceExt;

use crate::animation::{AnimationSet, Skeleton, MAX_JOINTS};
use crate::entity_bounds;
use crate::math::{Mat4, Vec3};

const TARGET_PLAYER_HEIGHT: f32 = 1.8;
const TARGET_NPC_HEIGHT: f32 = 1.6;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ModelUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
    pub tint: [f32; 4],
    pub _padding: [f32; 4],
}

pub struct ModelDraw {
    pub target: ModelTarget,
    pub model: Mat4,
    pub tint: [f32; 4],
    pub entity_id: Option<openmmo_common::EntityId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelTarget {
    Player,
    Npc(openmmo_common::NpcId),
}

pub struct GlbModel {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub width: f32,
    pub height: f32,
    _texture: wgpu::Texture,
    _sampler: wgpu::Sampler,
}

struct MeshData {
    vertices: Vec<ModelVertex>,
    indices: Vec<u32>,
    texture: image::DynamicImage,
}

/// Back-compat alias for the player mesh loader.
pub type PlayerModel = GlbModel;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SkinnedModelVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub joints: [u32; 4],
    pub weights: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SkinnedUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
    pub tint: [f32; 4],
    pub bones: [[[f32; 4]; 4]; MAX_JOINTS],
}

pub struct SkinnedGlbModel {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub width: f32,
    pub height: f32,
    /// Scale/center bind-pose vertices to the NPC footprint without mutating mesh data.
    pub footprint_matrix: Mat4,
    pub skeleton: Skeleton,
    pub animations: AnimationSet,
    _texture: wgpu::Texture,
    _sampler: wgpu::Sampler,
}

pub enum NpcModel {
    Static(GlbModel),
    Skinned(SkinnedGlbModel),
}

impl NpcModel {
    pub fn width(&self) -> f32 {
        match self {
            Self::Static(m) => m.width,
            Self::Skinned(m) => m.width,
        }
    }

    pub fn height(&self) -> f32 {
        match self {
            Self::Static(m) => m.height,
            Self::Skinned(m) => m.height,
        }
    }

    pub fn is_skinned(&self) -> bool {
        matches!(self, Self::Skinned(_))
    }
}

const DEFAULT_PLAYER_MODEL: &str = "Pinguin_001.glb";
pub const MUTANT_TIGER_MODEL: &str = "Tiger_001.glb";

/// NPC id 1 (meadow crawler / Mutant Tiger) uses this model.
pub const MUTANT_TIGER_NPC_ID: openmmo_common::NpcId = openmmo_common::NpcId(1);

/// Resolve the player GLB path from env, workspace layout, cwd, or executable location.
pub fn resolve_player_model_path() -> Option<PathBuf> {
    if let Ok(env) = std::env::var("OPENMMO_PLAYER_MODEL") {
        let path = PathBuf::from(&env);
        if path.is_file() {
            return Some(path);
        }
        eprintln!("OpenMMO: OPENMMO_PLAYER_MODEL points to missing file: {env}");
    }

    resolve_model_path(DEFAULT_PLAYER_MODEL)
}

pub fn resolve_model_path(filename: &str) -> Option<PathBuf> {
    for candidate in model_candidates(filename) {
        if candidate.is_file() {
            return candidate.canonicalize().ok().or(Some(candidate));
        }
    }
    None
}

/// Resolve an NPC content model name to a GLB path (`frog` -> `frog.glb` / `Frog.glb`).
pub fn resolve_npc_glb_path(name: &str) -> Option<PathBuf> {
    let capitalized = {
        let mut chars = name.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    };
    for filename in [
        format!("{name}.glb"),
        format!("{capitalized}.glb"),
    ] {
        if let Some(path) = resolve_model_path(&filename) {
            return Some(path);
        }
    }
    None
}

fn model_candidates(filename: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/models")
            .join(filename),
    );

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("assets/models").join(filename));
        let mut dir = Some(cwd.as_path());
        while let Some(d) = dir {
            candidates.push(d.join("assets/models").join(filename));
            dir = d.parent();
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("assets/models").join(filename));
            candidates.push(dir.join("../../assets/models").join(filename));
        }
    }

    candidates
}

pub fn load_player_model(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    path: &Path,
) -> Result<GlbModel> {
    let model = load_glb_model(device, queue, surface_format, path, 1.0, 1.0, TARGET_PLAYER_HEIGHT)?;
    entity_bounds::set_player_model_dims(model.width, model.height);
    Ok(model)
}

pub fn load_npc_model(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    npc_id: openmmo_common::NpcId,
    path: &Path,
    footprint: openmmo_common::NpcFootprint,
) -> Result<GlbModel> {
    let model = load_glb_model(
        device,
        queue,
        surface_format,
        path,
        footprint.width as f32,
        footprint.height as f32,
        TARGET_NPC_HEIGHT,
    )?;
    entity_bounds::set_npc_model_dims(npc_id, model.width, model.height);
    Ok(model)
}

pub fn load_skinned_npc_model(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    npc_id: openmmo_common::NpcId,
    path: &Path,
    footprint: openmmo_common::NpcFootprint,
) -> Result<SkinnedGlbModel> {
    let model = load_skinned_glb_model(
        device,
        queue,
        surface_format,
        path,
        footprint.width as f32,
        footprint.height as f32,
        TARGET_NPC_HEIGHT,
    )?;
    entity_bounds::set_npc_model_dims(npc_id, model.width, model.height);
    Ok(model)
}

pub fn load_skinned_glb_model(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    path: &Path,
    footprint_w: f32,
    footprint_h: f32,
    target_height: f32,
) -> Result<SkinnedGlbModel> {
    let (mesh, skeleton, animations) = load_skinned_mesh_data(path)?;
    let footprint_matrix =
        skinned_footprint_matrix(&mesh.vertices, footprint_w, footprint_h, target_height);
    let (width, height) = skinned_footprint_dims(&mesh.vertices, footprint_matrix);

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Skinned Model Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("model_skinned.wgsl").into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Skinned Model Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Skinned Model Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Skinned Model Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<SkinnedModelVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![
                    0 => Float32x3,
                    1 => Float32x3,
                    2 => Float32x2,
                    3 => Uint32x4,
                    4 => Float32x4,
                ],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Skinned Model Vertex Buffer"),
        contents: bytemuck::cast_slice(&mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Skinned Model Index Buffer"),
        contents: bytemuck::cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Skinned Model Uniform Buffer"),
        size: std::mem::size_of::<SkinnedUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let (texture, texture_view, sampler) = upload_texture(device, queue, &mesh.texture)?;

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Skinned Model Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
        ],
    });

    Ok(SkinnedGlbModel {
        pipeline,
        bind_group_layout,
        vertex_buffer,
        index_buffer,
        index_count: mesh.indices.len() as u32,
        uniform_buffer,
        bind_group,
        width,
        height,
        footprint_matrix,
        skeleton,
        animations,
        _texture: texture,
        _sampler: sampler,
    })
}

impl SkinnedGlbModel {
    pub fn draw_one(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        instance: &ModelDraw,
        bones: &[Mat4],
    ) {
        if self.index_count == 0 {
            return;
        }
        let mut bone_cols = [[[0.0f32; 4]; 4]; MAX_JOINTS];
        for (i, bone) in bones.iter().take(MAX_JOINTS).enumerate() {
            bone_cols[i] = bone.cols;
        }
        let model = instance.model.mul(self.footprint_matrix);
        let uniforms = SkinnedUniforms {
            view_proj: view_proj.cols,
            model: model.cols,
            tint: instance.tint,
            bones: bone_cols,
        };
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&uniforms),
        );
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

struct SkinnedMeshData {
    vertices: Vec<SkinnedModelVertex>,
    indices: Vec<u32>,
    texture: image::DynamicImage,
}

fn load_skinned_mesh_data(path: &Path) -> Result<(SkinnedMeshData, Skeleton, AnimationSet)> {
    let (document, buffers, images) = gltf::import(path).context("failed to import glb")?;
    let (skeleton, animations) =
        crate::animation::load_animation_from_document(&document, &buffers)?;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut found = false;

    for node in document.nodes() {
        let Some(mesh) = node.mesh() else { continue };
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let Some(joints_iter) = reader.read_joints(0) else {
                continue;
            };
            let weights_iter = reader
                .read_weights(0)
                .context("skinned primitive missing weights")?;
            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .context("skinned primitive missing positions")?
                .collect();
            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|iter| iter.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            let uvs: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|iter| iter.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
            let joints: Vec<[u16; 4]> = joints_iter.into_u16().collect();
            let weights: Vec<[f32; 4]> = weights_iter.into_f32().collect();

            let base = vertices.len() as u32;
            for i in 0..positions.len() {
                let mut w = weights[i];
                let sum = w[0] + w[1] + w[2] + w[3];
                if sum > 1e-8 {
                    w = [w[0] / sum, w[1] / sum, w[2] / sum, w[3] / sum];
                }
                vertices.push(SkinnedModelVertex {
                    position: positions[i],
                    normal: normals[i],
                    uv: uvs[i],
                    joints: [
                        joints[i][0] as u32,
                        joints[i][1] as u32,
                        joints[i][2] as u32,
                        joints[i][3] as u32,
                    ],
                    weights: w,
                });
            }

            if let Some(iter) = reader.read_indices() {
                for idx in iter.into_u32() {
                    indices.push(base + idx);
                }
            } else {
                for i in 0..positions.len() as u32 {
                    indices.push(base + i);
                }
            }
            found = true;
            break;
        }
        if found {
            break;
        }
    }

    if !found || vertices.is_empty() {
        anyhow::bail!("glb contains no skinned mesh geometry");
    }

    let texture = if let Some(image) = images.into_iter().next() {
        gltf_image_to_dynamic(&image).unwrap_or_else(|err| {
            tracing::warn!(%err, "failed to convert skinned model texture; using material color");
            material_fallback_texture(&document)
        })
    } else {
        material_fallback_texture(&document)
    };

    Ok((
        SkinnedMeshData {
            vertices,
            indices,
            texture,
        },
        skeleton,
        animations,
    ))
}

fn skinned_bind_pose_bounds(vertices: &[SkinnedModelVertex]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(v.position[axis]);
            max[axis] = max[axis].max(v.position[axis]);
        }
    }
    (min, max)
}

fn skinned_footprint_matrix(
    vertices: &[SkinnedModelVertex],
    _footprint_w: f32,
    _footprint_h: f32,
    target_height: f32,
) -> Mat4 {
    let (min, max) = skinned_bind_pose_bounds(vertices);
    let width_x = (max[0] - min[0]).max(0.01);
    let height_y = (max[1] - min[1]).max(0.01);
    let depth_z = (max[2] - min[2]).max(0.01);
    let max_dim = width_x.max(height_y).max(depth_z);
    let scale = target_height / max_dim;

    let center_x = (min[0] + max[0]) * 0.5;
    let center_z = (min[2] + max[2]) * 0.5;
    let base_y = min[1];
    let translate_to_origin = Mat4::translation(-center_x, -base_y, -center_z);
    let uniform_scale = Mat4::scale(scale, scale, scale);
    uniform_scale.mul(translate_to_origin)
}

fn skinned_footprint_dims(vertices: &[SkinnedModelVertex], matrix: Mat4) -> (f32, f32) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in vertices {
        let (p, w) = matrix.transform_point(Vec3::new(v.position[0], v.position[1], v.position[2]));
        let inv_w = if w.abs() > 1e-8 { 1.0 / w } else { 1.0 };
        let pos = [p.x * inv_w, p.y * inv_w, p.z * inv_w];
        for axis in 0..3 {
            min[axis] = min[axis].min(pos[axis]);
            max[axis] = max[axis].max(pos[axis]);
        }
    }
    let height = (max[1] - min[1]).max(0.01);
    let width = (max[0] - min[0]).max(max[2] - min[2]).max(0.01);
    (width, height)
}

#[cfg(test)]
fn skin_vertex_linear(bones: &[Mat4], joints: [u32; 4], weights: [f32; 4], position: [f32; 3]) -> [f32; 3] {
    let mut out = Mat4::identity();
    for i in 0..4 {
        let bone = bones
            .get(joints[i] as usize)
            .copied()
            .unwrap_or(Mat4::identity());
        let w = weights[i];
        for c in 0..4 {
            for r in 0..4 {
                out.cols[c][r] += bone.cols[c][r] * w;
            }
        }
    }
    let (p, w) = out.transform_point(Vec3::new(position[0], position[1], position[2]));
    let inv_w = if w.abs() > 1e-8 { 1.0 / w } else { 1.0 };
    [p.x * inv_w, p.y * inv_w, p.z * inv_w]
}

pub fn load_glb_model(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    path: &Path,
    footprint_w: f32,
    footprint_h: f32,
    target_height: f32,
) -> Result<GlbModel> {
    let mesh = load_mesh_data(path, footprint_w, footprint_h, target_height)?;
    let (width, height) = compute_bounds(&mesh.vertices);

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Player Model Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("model_shader.wgsl").into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Player Model Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Player Model Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Player Model Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<ModelVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![
                    0 => Float32x3,
                    1 => Float32x3,
                    2 => Float32x2,
                ],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Player Model Vertex Buffer"),
        contents: bytemuck::cast_slice(&mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Player Model Index Buffer"),
        contents: bytemuck::cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Player Model Uniform Buffer"),
        size: std::mem::size_of::<ModelUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let (texture, texture_view, sampler) = upload_texture(device, queue, &mesh.texture)?;

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Player Model Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
        ],
    });

    Ok(GlbModel {
        pipeline,
        bind_group_layout,
        vertex_buffer,
        index_buffer,
        index_count: mesh.indices.len() as u32,
        uniform_buffer,
        bind_group,
        width,
        height,
        _texture: texture,
        _sampler: sampler,
    })
}

impl GlbModel {
    pub fn draw_instances(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        instances: &[ModelDraw],
    ) {
        if instances.is_empty() || self.index_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        for instance in instances {
            self.draw_one(render_pass, queue, view_proj, instance);
        }
    }

    pub fn draw_one(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        instance: &ModelDraw,
    ) {
        if self.index_count == 0 {
            return;
        }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        let uniforms = ModelUniforms {
            view_proj: view_proj.cols,
            model: instance.model.cols,
            tint: instance.tint,
            _padding: [0.0; 4],
        };
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&uniforms),
        );
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

/// Y-axis rotation offset if the GLB mesh forward axis differs from +Z.
const MODEL_YAW_OFFSET: f32 = 0.0;

pub fn entity_model_matrix(base: [f32; 3], yaw: f32) -> Mat4 {
    let [cx, surface_y, cz] = base;
    Mat4::translation(cx, surface_y, cz).mul(Mat4::rotation_y(yaw + MODEL_YAW_OFFSET))
}

pub fn player_model_matrix(base: [f32; 3], yaw: f32) -> Mat4 {
    entity_model_matrix(base, yaw)
}

fn load_mesh_data(path: &Path, footprint_w: f32, footprint_h: f32, target_height: f32) -> Result<MeshData> {
    let (document, buffers, images) = gltf::import(path).context("failed to import glb")?;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for scene in document.scenes() {
        for node in scene.nodes() {
            append_node(node, &buffers, Mat4::identity(), &mut vertices, &mut indices);
        }
    }

    if vertices.is_empty() {
        anyhow::bail!("glb contains no mesh geometry");
    }

    let texture = if let Some(image) = images.into_iter().next() {
        gltf_image_to_dynamic(&image).unwrap_or_else(|err| {
            tracing::warn!(%err, "failed to convert player model texture; using material color");
            material_fallback_texture(&document)
        })
    } else {
        material_fallback_texture(&document)
    };

    normalize_mesh_to_footprint(&mut vertices, footprint_w, footprint_h, target_height);

    Ok(MeshData {
        vertices,
        indices,
        texture,
    })
}

fn gltf_image_to_dynamic(image: &gltf::image::Data) -> Result<image::DynamicImage> {
    use gltf::image::Format;

    let width = image.width.max(1);
    let height = image.height.max(1);
    match image.format {
        Format::R8G8B8A8 => {
            if image.pixels.len() != (width * height * 4) as usize {
                anyhow::bail!(
                    "unexpected R8G8B8A8 size: got {} expected {}",
                    image.pixels.len(),
                    width * height * 4
                );
            }
            let rgba = image::RgbaImage::from_raw(width, height, image.pixels.clone())
                .context("failed to build RGBA image")?;
            Ok(image::DynamicImage::ImageRgba8(rgba))
        }
        Format::R8G8B8 => {
            let mut rgba = image::RgbaImage::new(width, height);
            for (i, chunk) in image.pixels.chunks_exact(3).enumerate() {
                let x = (i as u32) % width;
                let y = (i as u32) / width;
                rgba.put_pixel(
                    x,
                    y,
                    image::Rgba([chunk[0], chunk[1], chunk[2], 255]),
                );
            }
            Ok(image::DynamicImage::ImageRgba8(rgba))
        }
        other => anyhow::bail!("unsupported gltf image format: {other:?}"),
    }
}

fn material_fallback_texture(document: &gltf::Document) -> image::DynamicImage {
    let factor = document
        .materials()
        .next()
        .map(|mat| mat.pbr_metallic_roughness().base_color_factor())
        .unwrap_or([0.8, 0.8, 0.85, 1.0]);
    let r = (factor[0] * 255.0) as u8;
    let g = (factor[1] * 255.0) as u8;
    let b = (factor[2] * 255.0) as u8;
    let a = (factor[3] * 255.0) as u8;
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        1,
        1,
        image::Rgba([r, g, b, a]),
    ))
}

fn append_node(
    node: gltf::Node,
    buffers: &[gltf::buffer::Data],
    parent: Mat4,
    vertices: &mut Vec<ModelVertex>,
    indices: &mut Vec<u32>,
) {
    let local = Mat4::from_gltf(node.transform().matrix());
    let transform = parent.mul(local);

    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            append_primitive(primitive, buffers, transform, vertices, indices);
        }
    }

    for child in node.children() {
        append_node(child, buffers, transform, vertices, indices);
    }
}

fn append_primitive(
    primitive: gltf::Primitive,
    buffers: &[gltf::buffer::Data],
    transform: Mat4,
    vertices: &mut Vec<ModelVertex>,
    indices: &mut Vec<u32>,
) {
    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
    let positions: Vec<[f32; 3]> = reader
        .read_positions()
        .context("mesh primitive missing positions")
        .unwrap()
        .collect();
    let normals: Vec<[f32; 3]> = reader
        .read_normals()
        .map(|iter| iter.collect())
        .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
    let uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(0)
        .map(|iter| iter.into_f32().collect())
        .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

    let base = vertices.len() as u32;
    for i in 0..positions.len() {
        let pos = transform_point(transform, positions[i]);
        let normal = transform_direction(transform, normals[i]);
        vertices.push(ModelVertex {
            position: pos,
            normal,
            uv: uvs[i],
        });
    }

    if let Some(iter) = reader.read_indices() {
        for idx in iter.into_u32() {
            indices.push(base + idx);
        }
    } else {
        for i in 0..positions.len() as u32 {
            indices.push(base + i);
        }
    }
}

fn transform_point(m: Mat4, p: [f32; 3]) -> [f32; 3] {
    let x = m.cols[0][0] * p[0] + m.cols[1][0] * p[1] + m.cols[2][0] * p[2] + m.cols[3][0];
    let y = m.cols[0][1] * p[0] + m.cols[1][1] * p[1] + m.cols[2][1] * p[2] + m.cols[3][1];
    let z = m.cols[0][2] * p[0] + m.cols[1][2] * p[1] + m.cols[2][2] * p[2] + m.cols[3][2];
    [x, y, z]
}

fn transform_direction(m: Mat4, d: [f32; 3]) -> [f32; 3] {
    let x = m.cols[0][0] * d[0] + m.cols[1][0] * d[1] + m.cols[2][0] * d[2];
    let y = m.cols[0][1] * d[0] + m.cols[1][1] * d[1] + m.cols[2][1] * d[2];
    let z = m.cols[0][2] * d[0] + m.cols[1][2] * d[1] + m.cols[2][2] * d[2];
    let len = (x * x + y * y + z * z).sqrt();
    if len < 1e-8 {
        [0.0, 1.0, 0.0]
    } else {
        [x / len, y / len, z / len]
    }
}

fn compute_bounds(vertices: &[ModelVertex]) -> (f32, f32) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(v.position[axis]);
            max[axis] = max[axis].max(v.position[axis]);
        }
    }
    let height = (max[1] - min[1]).max(0.01);
    let width = (max[0] - min[0]).max(max[2] - min[2]).max(0.01);
    (width, height)
}

fn normalize_mesh_to_footprint(
    vertices: &mut [ModelVertex],
    footprint_w: f32,
    footprint_h: f32,
    target_height: f32,
) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in vertices.iter() {
        for axis in 0..3 {
            min[axis] = min[axis].min(v.position[axis]);
            max[axis] = max[axis].max(v.position[axis]);
        }
    }
    let width_x = (max[0] - min[0]).max(0.01);
    let height_y = (max[1] - min[1]).max(0.01);
    let depth_z = (max[2] - min[2]).max(0.01);

    for v in vertices.iter_mut() {
        v.position[0] = (v.position[0] - min[0]) / width_x * footprint_w;
        v.position[1] = (v.position[1] - min[1]) / height_y * target_height;
        v.position[2] = (v.position[2] - min[2]) / depth_z * footprint_h;
    }

    // Center the mesh on the footprint anchor so world placement at the footprint
    // center aligns the model with the occupied tiles (not offset toward +X/+Z).
    let half_w = footprint_w * 0.5;
    let half_h = footprint_h * 0.5;
    for v in vertices.iter_mut() {
        v.position[0] -= half_w;
        v.position[2] -= half_h;
    }
}

fn normalize_mesh(vertices: &mut [ModelVertex], target_height: f32) {
    normalize_mesh_to_footprint(vertices, target_height, target_height, target_height);
}

fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    image: &image::DynamicImage,
) -> Result<(wgpu::Texture, wgpu::TextureView, wgpu::Sampler)> {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Player Model Texture"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Player Model Sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    Ok((texture, view, sampler))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_mesh_to_footprint_centers_on_origin() {
        let mut vertices = vec![
            ModelVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
            },
            ModelVertex {
                position: [1.0, 2.0, 1.0],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 1.0],
            },
        ];
        normalize_mesh_to_footprint(&mut vertices, 2.0, 2.0, 1.8);

        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for v in &vertices {
            for axis in 0..3 {
                min[axis] = min[axis].min(v.position[axis]);
                max[axis] = max[axis].max(v.position[axis]);
            }
        }
        assert!((min[0] + 1.0).abs() < 1e-5);
        assert!((max[0] - 1.0).abs() < 1e-5);
        assert!((min[2] + 1.0).abs() < 1e-5);
        assert!((max[2] - 1.0).abs() < 1e-5);
        assert!((min[1]).abs() < 1e-5);
        assert!((max[1] - 1.8).abs() < 1e-5);
    }

    #[test]
    fn frog_bind_pose_skinning_preserves_vertices() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/models/Frog.glb");
        assert!(path.is_file(), "missing Frog.glb at {path:?}");
        let (mesh, skeleton, _) = load_skinned_mesh_data(&path).expect("load frog mesh");
        let bones = skeleton.bind_pose();
        let mut max_err = 0.0f32;
        for v in &mesh.vertices {
            let skinned = skin_vertex_linear(bones.as_slice(), v.joints, v.weights, v.position);
            for axis in 0..3 {
                max_err = max_err.max((skinned[axis] - v.position[axis]).abs());
            }
        }
        assert!(
            max_err < 0.05,
            "bind-pose skinning should preserve vertex positions (max err {max_err})"
        );
    }

    #[test]
    fn frog_footprint_matrix_preserves_proportions() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/models/Frog.glb");
        let (mesh, _, _) = load_skinned_mesh_data(&path).expect("load frog mesh");
        let target_height = 1.6;
        let matrix = skinned_footprint_matrix(&mesh.vertices, 1.0, 1.0, target_height);

        let (min_raw, max_raw) = skinned_bind_pose_bounds(&mesh.vertices);
        let raw_size = [
            max_raw[0] - min_raw[0],
            max_raw[1] - min_raw[1],
            max_raw[2] - min_raw[2],
        ];
        let raw_aspect_xz = raw_size[0] / raw_size[2];

        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for v in &mesh.vertices {
            let (p, w) = matrix.transform_point(Vec3::new(v.position[0], v.position[1], v.position[2]));
            let inv_w = if w.abs() > 1e-8 { 1.0 / w } else { 1.0 };
            let pos = [p.x * inv_w, p.y * inv_w, p.z * inv_w];
            for axis in 0..3 {
                min[axis] = min[axis].min(pos[axis]);
                max[axis] = max[axis].max(pos[axis]);
            }
        }
        let fitted_size = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
        let fitted_aspect_xz = fitted_size[0] / fitted_size[2];

        assert!(min[1].abs() < 1e-3, "feet should rest on y=0");
        let largest_dim = fitted_size[0].max(fitted_size[1]).max(fitted_size[2]);
        assert!(
            (largest_dim - target_height).abs() < 0.05,
            "largest dimension should match target height (got {largest_dim})"
        );
        assert!(
            (raw_aspect_xz - fitted_aspect_xz).abs() < 0.02,
            "uniform scale should preserve xz aspect ratio"
        );
    }

    #[test]
    fn compile_time_asset_path_exists() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/models")
            .join(DEFAULT_PLAYER_MODEL);
        assert!(path.is_file(), "missing penguin model at {path:?}");
    }

    #[test]
    fn resolve_player_model_path_finds_penguin() {
        let path = resolve_player_model_path().expect("penguin model should resolve");
        assert!(path.is_file(), "resolved path missing: {path:?}");
    }

    #[test]
    fn penguin_glb_reports_image_format() {
        let path = resolve_player_model_path().expect("penguin model should resolve");
        let (_doc, _bufs, images) = gltf::import(&path).expect("import");
        for (i, image) in images.iter().enumerate() {
            eprintln!(
                "image {i}: {}x{} format={:?} bytes={}",
                image.width,
                image.height,
                image.format,
                image.pixels.len()
            );
        }
    }

    #[test]
    fn resolve_tiger_model_path() {
        let path = resolve_model_path(MUTANT_TIGER_MODEL).expect("tiger model should resolve");
        assert!(path.is_file(), "resolved path missing: {path:?}");
    }

    #[test]
    fn tiger_glb_loads_geometry() {
        let path = resolve_model_path(MUTANT_TIGER_MODEL).expect("tiger model should resolve");
        let mesh = load_mesh_data(&path, 2.0, 2.0, TARGET_NPC_HEIGHT).expect("mesh load");
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        let (width, height) = compute_bounds(&mesh.vertices);
        assert!((width - 2.0).abs() < 0.05);
        assert!((height - TARGET_NPC_HEIGHT).abs() < 0.05);
        let depth = mesh.vertices.iter().map(|v| v.position[2]).fold(0.0f32, f32::max);
        assert!((depth - 2.0).abs() < 0.05);
    }

    #[test]
    fn penguin_glb_loads_geometry() {
        let path = resolve_player_model_path().expect("penguin model should resolve");
        let mesh = load_mesh_data(&path, 1.0, 1.0, TARGET_PLAYER_HEIGHT).expect("mesh load");
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        let (width, height) = compute_bounds(&mesh.vertices);
        assert!((height - TARGET_PLAYER_HEIGHT).abs() < 0.01);
        assert!(width > 0.0);
    }
}
