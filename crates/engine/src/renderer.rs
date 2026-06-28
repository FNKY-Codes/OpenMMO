use std::collections::HashMap;
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use openmmo_common::{ContentPack, EntityId, NpcFootprint, NpcId, RegionDef, TilePos, WorldEntity};

use crate::animation::{AnimationPlayer, DeathCorpse, FROG_ATTACK, FROG_IDLE, FROG_JUMP};
use crate::camera::Camera;
use crate::entity_bounds;
use crate::math::Vec3;
use crate::mesh::{Mesh, ModelCache};
use crate::model::{
    entity_model_matrix, load_npc_model, load_player_model, load_skinned_npc_model,
    resolve_model_path, resolve_npc_glb_path, GlbModel, ModelDraw, ModelTarget, NpcModel,
    MUTANT_TIGER_MODEL, MUTANT_TIGER_NPC_ID,
};
use crate::movement_interp::EntityMovementInterp;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
}

pub struct GpuState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
}

struct TransparentDraw {
    solid_start: u32,
    solid_len: u32,
    outline_start: u32,
    outline_len: u32,
    sort_key: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverTarget {
    Entity(EntityId),
    Tile(TilePos),
}

const HOVER_OUTLINE_COLOR: [f32; 4] = [0.4, 0.9, 0.3, 0.95];

#[derive(Clone, Copy, PartialEq, Eq)]
enum TileCacheKey {
    TestMap,
    Region(openmmo_common::RegionId),
}

pub struct Renderer {
    window: std::sync::Arc<egui_winit::winit::window::Window>,
    gpu: GpuState,
    pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,
    outline_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    camera: Camera,
    tile_cache_key: Option<TileCacheKey>,
    tile_vertex_count: u32,
    cached_tile_vertices: Vec<Vertex>,
    tiles_need_upload: bool,
    scratch_opaque_entity_vertices: Vec<Vertex>,
    scratch_opaque_outline_vertices: Vec<Vertex>,
    scratch_transparent_entity_vertices: Vec<Vertex>,
    scratch_transparent_outline_vertices: Vec<Vertex>,
    scratch_transparent_draws: Vec<TransparentDraw>,
    scratch_upload: Vec<Vertex>,
    player_model: Option<GlbModel>,
    npc_models: HashMap<NpcId, NpcModel>,
    scratch_model_draws: Vec<ModelDraw>,
}

impl Renderer {
    pub fn is_skinned_npc(&self, npc_id: NpcId) -> bool {
        self.npc_models
            .get(&npc_id)
            .is_some_and(NpcModel::is_skinned)
    }
    pub async fn new(
        window: std::sync::Arc<egui_winit::winit::window::Window>,
        player_model_path: Option<&std::path::Path>,
        content: &ContentPack,
    ) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("OpenMMO Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let (depth_texture, depth_view) =
            Self::create_depth_texture(&device, config.width, config.height);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("World Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Uniform Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Opaque World Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
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

        let transparent_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Transparent World Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
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
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let outline_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Outline Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: (std::mem::size_of::<Vertex>() * 60000) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let player_model = player_model_path.and_then(|path| {
            match load_player_model(&device, &queue, format, path) {
                Ok(model) => {
                    eprintln!("OpenMMO: loaded player model from {}", path.display());
                    tracing::info!(?path, "loaded player model");
                    Some(model)
                }
                Err(err) => {
                    eprintln!(
                        "OpenMMO: failed to load player model from {}: {err}",
                        path.display()
                    );
                    tracing::warn!(?path, %err, "failed to load player model");
                    None
                }
            }
        });

        let mut npc_models = HashMap::new();
        if let Some(path) = resolve_model_path(MUTANT_TIGER_MODEL) {
            let footprint = content
                .npc(MUTANT_TIGER_NPC_ID)
                .map(NpcFootprint::from_def)
                .unwrap_or_default();
            match load_npc_model(
                &device,
                &queue,
                format,
                MUTANT_TIGER_NPC_ID,
                &path,
                footprint,
            ) {
                Ok(model) => {
                    eprintln!("OpenMMO: loaded Mutant Tiger model from {}", path.display());
                    npc_models.insert(MUTANT_TIGER_NPC_ID, NpcModel::Static(model));
                }
                Err(err) => {
                    eprintln!(
                        "OpenMMO: failed to load Mutant Tiger model from {}: {err}",
                        path.display()
                    );
                }
            }
        }

        for npc_def in &content.npcs {
            if npc_def.id == MUTANT_TIGER_NPC_ID {
                continue;
            }
            let Some(model_name) = npc_def.model.as_deref() else {
                continue;
            };
            let Some(path) = resolve_npc_glb_path(model_name) else {
                continue;
            };
            let footprint = NpcFootprint::from_def(npc_def);
            match load_skinned_npc_model(
                &device,
                &queue,
                format,
                npc_def.id,
                &path,
                footprint,
            ) {
                Ok(model) => {
                    eprintln!(
                        "OpenMMO: loaded skinned NPC model for {} from {}",
                        npc_def.name,
                        path.display()
                    );
                    npc_models.insert(npc_def.id, NpcModel::Skinned(model));
                }
                Err(err) => {
                    eprintln!(
                        "OpenMMO: failed to load skinned model for {} from {}: {err}",
                        npc_def.name,
                        path.display()
                    );
                }
            }
        }

        Self {
            window,
            gpu: GpuState {
                device,
                queue,
                surface,
                config,
            },
            pipeline,
            transparent_pipeline,
            outline_pipeline,
            vertex_buffer,
            uniform_buffer,
            uniform_bind_group,
            depth_texture,
            depth_view,
            camera: Camera::new(),
            tile_cache_key: None,
            tile_vertex_count: 0,
            cached_tile_vertices: Vec::with_capacity(8192),
            tiles_need_upload: false,
            scratch_opaque_entity_vertices: Vec::with_capacity(4096),
            scratch_opaque_outline_vertices: Vec::with_capacity(2048),
            scratch_transparent_entity_vertices: Vec::with_capacity(512),
            scratch_transparent_outline_vertices: Vec::with_capacity(512),
            scratch_transparent_draws: Vec::with_capacity(64),
            scratch_upload: Vec::with_capacity(8192),
            player_model,
            npc_models,
            scratch_model_draws: Vec::with_capacity(32),
        }
    }

    fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    pub fn window(&self) -> &egui_winit::winit::window::Window {
        &self.window
    }

    pub fn gpu(&self) -> &GpuState {
        &self.gpu
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.gpu.config.width = width;
            self.gpu.config.height = height;
            self.gpu
                .surface
                .configure(&self.gpu.device, &self.gpu.config);
            let (depth_texture, depth_view) =
                Self::create_depth_texture(&self.gpu.device, width, height);
            self.depth_texture = depth_texture;
            self.depth_view = depth_view;
        }
    }

    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    pub fn invalidate_tile_cache(&mut self) {
        self.tile_cache_key = None;
        self.tile_vertex_count = 0;
        self.cached_tile_vertices.clear();
        self.tiles_need_upload = false;
    }

    fn ensure_vertex_buffer_capacity(&mut self, required_vertices: usize) -> bool {
        let vertex_size = std::mem::size_of::<Vertex>();
        let current_capacity = self.vertex_buffer.size() as usize / vertex_size;
        if required_vertices <= current_capacity {
            return false;
        }
        let new_capacity = required_vertices.next_power_of_two().max(60_000);
        self.vertex_buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: (vertex_size * new_capacity) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.tiles_need_upload = true;
        true
    }

    fn upload_tile_vertices(&mut self) {
        if self.cached_tile_vertices.is_empty() {
            return;
        }
        self.gpu.queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(&self.cached_tile_vertices),
        );
        self.tiles_need_upload = false;
    }

    pub fn begin_frame(
        &mut self,
    ) -> Result<
        (
            wgpu::SurfaceTexture,
            wgpu::TextureView,
            wgpu::CommandEncoder,
        ),
        wgpu::SurfaceError,
    > {
        let output = self.gpu.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Encoder"),
            });
        Ok((output, view, encoder))
    }

    pub fn render_world_pass(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        region: Option<&RegionDef>,
        entities: &[WorldEntity],
        content: &ContentPack,
        model_cache: &mut ModelCache,
        local_player: Option<openmmo_common::PlayerId>,
        movement_interp: &EntityMovementInterp,
        hover: Option<HoverTarget>,
        npc_animations: &mut HashMap<EntityId, AnimationPlayer>,
        death_corpses: &mut [DeathCorpse],
        dt: f32,
    ) {
        let vp = self
            .camera
            .view_projection(self.gpu.config.width, self.gpu.config.height);
        self.gpu.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&Uniforms { view_proj: vp.cols }),
        );

        let tile_key = if let Some(region) = region {
            TileCacheKey::Region(region.id)
        } else {
            TileCacheKey::TestMap
        };

        if self.tile_cache_key != Some(tile_key) {
            let mut tile_vertices = Vec::new();
            if let Some(region) = region {
                self.build_region_tiles(region, &mut tile_vertices);
            } else {
                self.build_test_map(&mut tile_vertices);
            }
            self.tile_vertex_count = tile_vertices.len() as u32;
            self.cached_tile_vertices = tile_vertices;
            self.tile_cache_key = Some(tile_key);
            self.tiles_need_upload = true;
        }

        let eye = self.camera.eye_position();
        let mut opaque_entity_vertices = std::mem::take(&mut self.scratch_opaque_entity_vertices);
        let mut opaque_outline_vertices = std::mem::take(&mut self.scratch_opaque_outline_vertices);
        let mut transparent_entity_vertices =
            std::mem::take(&mut self.scratch_transparent_entity_vertices);
        let mut transparent_outline_vertices =
            std::mem::take(&mut self.scratch_transparent_outline_vertices);
        let mut transparent_draws = std::mem::take(&mut self.scratch_transparent_draws);
        let mut model_draws = std::mem::take(&mut self.scratch_model_draws);
        opaque_entity_vertices.clear();
        opaque_outline_vertices.clear();
        transparent_entity_vertices.clear();
        transparent_outline_vertices.clear();
        transparent_draws.clear();
        model_draws.clear();

        let now = Instant::now();
        let mut local_entity = None;
        for entity in entities {
            let is_local = matches!(
                &entity.kind,
                openmmo_common::EntityKind::Player { player_id, .. }
                    if Some(*player_id) == local_player
            );
            if is_local {
                local_entity = Some(entity);
            } else {
                self.build_entity(
                    entity,
                    region,
                    eye,
                    now,
                    movement_interp,
                    content,
                    model_cache,
                    &mut opaque_entity_vertices,
                    &mut opaque_outline_vertices,
                    &mut transparent_entity_vertices,
                    &mut transparent_outline_vertices,
                    &mut transparent_draws,
                    &mut model_draws,
                    local_player,
                );
            }
        }
        if let Some(entity) = local_entity {
            self.build_entity(
                entity,
                region,
                eye,
                now,
                movement_interp,
                content,
                model_cache,
                &mut opaque_entity_vertices,
                &mut opaque_outline_vertices,
                &mut transparent_entity_vertices,
                &mut transparent_outline_vertices,
                &mut transparent_draws,
                &mut model_draws,
                local_player,
            );
        }

        let tile_count = self.tile_vertex_count;
        let opaque_solid_base = tile_count;
        let opaque_solid_count = opaque_entity_vertices.len() as u32;
        let opaque_outline_base = opaque_solid_base + opaque_solid_count;
        let opaque_outline_count = opaque_outline_vertices.len() as u32;
        let transparent_solid_base = opaque_outline_base + opaque_outline_count;
        let transparent_solid_count = transparent_entity_vertices.len() as u32;
        let transparent_outline_base = transparent_solid_base + transparent_solid_count;

        self.scratch_upload.clear();
        self.scratch_upload.extend(&opaque_entity_vertices);
        self.scratch_upload.extend(&opaque_outline_vertices);
        self.scratch_upload.extend(&transparent_entity_vertices);
        self.scratch_upload.extend(&transparent_outline_vertices);

        let mut hover_outline_vertices = Vec::new();
        if let Some(hover_target) = hover {
            self.build_hover_outline(
                hover_target,
                region,
                entities,
                movement_interp,
                content,
                now,
                &mut hover_outline_vertices,
            );
        }
        self.scratch_upload.extend(&hover_outline_vertices);
        let hover_outline_base =
            transparent_outline_base + transparent_outline_vertices.len() as u32;
        let hover_outline_count = hover_outline_vertices.len() as u32;

        self.scratch_opaque_entity_vertices = opaque_entity_vertices;
        self.scratch_opaque_outline_vertices = opaque_outline_vertices;
        self.scratch_transparent_entity_vertices = transparent_entity_vertices;
        self.scratch_transparent_outline_vertices = transparent_outline_vertices;
        self.scratch_transparent_draws = transparent_draws;
        self.scratch_model_draws = model_draws;

        let total_vertices =
            self.tile_vertex_count as usize + self.scratch_upload.len();
        self.ensure_vertex_buffer_capacity(total_vertices);
        if self.tiles_need_upload {
            self.upload_tile_vertices();
        }

        if !self.scratch_upload.is_empty() {
            let byte_offset = tile_count as u64 * std::mem::size_of::<Vertex>() as u64;
            self.gpu.queue.write_buffer(
                &self.vertex_buffer,
                byte_offset,
                bytemuck::cast_slice(&self.scratch_upload),
            );
        }

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("World Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.45,
                        g: 0.65,
                        b: 0.85,
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
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        let has_opaque = opaque_outline_base > 0;
        if has_opaque {
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..opaque_outline_base, 0..1);
        }

        for draw in &self.scratch_model_draws {
            match draw.target {
                ModelTarget::Player => {
                    if let Some(model) = self.player_model.as_ref() {
                        model.draw_one(&mut render_pass, &self.gpu.queue, vp, draw);
                    }
                }
                ModelTarget::Npc(npc_id) => {
                    let Some(npc_model) = self.npc_models.get_mut(&npc_id) else {
                        continue;
                    };
                    match npc_model {
                        NpcModel::Static(model) => {
                            model.draw_one(&mut render_pass, &self.gpu.queue, vp, draw);
                        }
                        NpcModel::Skinned(model) => {
                            let Some(entity_id) = draw.entity_id else {
                                continue;
                            };
                            let player = npc_animations.entry(entity_id).or_insert_with(|| {
                                AnimationPlayer::new(FROG_IDLE)
                            });
                            update_skinned_npc_clip(
                                player,
                                movement_interp,
                                entity_id,
                                now,
                            );
                            player.advance(dt, &model.animations);
                            let bones = player.bone_matrices(&model.skeleton, &model.animations);
                            model.draw_one(
                                &mut render_pass,
                                &self.gpu.queue,
                                vp,
                                draw,
                                &bones,
                            );
                        }
                    }
                }
            }
        }

        for corpse in death_corpses.iter_mut() {
            let Some(NpcModel::Skinned(model)) = self.npc_models.get_mut(&corpse.npc_id) else {
                continue;
            };
            corpse.player.advance(dt, &model.animations);
            let bones = corpse
                .player
                .bone_matrices(&model.skeleton, &model.animations);
            let draw = ModelDraw {
                target: ModelTarget::Npc(corpse.npc_id),
                model: entity_model_matrix(corpse.base, corpse.yaw),
                tint: [1.0, 1.0, 1.0, 1.0],
                entity_id: None,
            };
            model.draw_one(&mut render_pass, &self.gpu.queue, vp, &draw, &bones);
        }

        if opaque_outline_count > 0 {
            render_pass.set_pipeline(&self.outline_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(
                opaque_outline_base..opaque_outline_base + opaque_outline_count,
                0..1,
            );
        }

        self.scratch_transparent_draws.sort_by(|a, b| {
            b.sort_key
                .partial_cmp(&a.sort_key)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for draw in &self.scratch_transparent_draws {
            if draw.solid_len > 0 {
                render_pass.set_pipeline(&self.transparent_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(
                    transparent_solid_base + draw.solid_start
                        ..transparent_solid_base + draw.solid_start + draw.solid_len,
                    0..1,
                );
            }
            if draw.outline_len > 0 {
                render_pass.set_pipeline(&self.outline_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(
                    transparent_outline_base + draw.outline_start
                        ..transparent_outline_base + draw.outline_start + draw.outline_len,
                    0..1,
                );
            }
        }

        if hover_outline_count > 0 {
            render_pass.set_pipeline(&self.outline_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(
                hover_outline_base..hover_outline_base + hover_outline_count,
                0..1,
            );
        }
    }

    fn build_hover_outline(
        &self,
        hover: HoverTarget,
        region: Option<&RegionDef>,
        entities: &[WorldEntity],
        movement_interp: &EntityMovementInterp,
        content: &ContentPack,
        now: Instant,
        vertices: &mut Vec<Vertex>,
    ) {
        match hover {
            HoverTarget::Entity(entity_id) => {
                let entity = entities.iter().find(|e| e.entity_id == entity_id);
                if let Some(entity) = entity {
                    let footprint = entity_footprint(content, entity);
                    let visual_base = movement_interp
                        .visual_center(entity.entity_id, now, region, footprint)
                        .or_else(|| {
                            entity_bounds::entity_tile(entity).map(|tile| {
                                let surface_y = Self::tile_surface_height(tile, region);
                                footprint.unwrap_or_default().world_center(tile, surface_y)
                            })
                        });
                    if let Some([cx, surface_y, cz]) = visual_base {
                        let (width, height) = entity_bounds::entity_cube_dims(&entity.kind);
                        let fp = footprint.unwrap_or_default();
                        let scale = 1.08;
                        let center = [cx, surface_y + height * 0.5, cz];
                        let size = [
                            fp.width as f32 * scale,
                            height * scale,
                            fp.height as f32 * scale,
                        ];
                        add_box_edge_lines(center, size, HOVER_OUTLINE_COLOR, vertices, true);
                        let _ = width;
                    }
                }
            }
            HoverTarget::Tile(tile) => {
                let [cx, _, cz] = Self::tile_center(tile);
                let surface_y = Self::tile_surface_height(tile, region);
                add_box_edge_lines(
                    [cx, surface_y + 0.03, cz],
                    [1.05, 0.06, 1.05],
                    HOVER_OUTLINE_COLOR,
                    vertices,
                    false,
                );
            }
        }
    }

    pub fn finish_frame(&self, encoder: wgpu::CommandEncoder, output: wgpu::SurfaceTexture) {
        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn build_test_map(&self, vertices: &mut Vec<Vertex>) {
        for x in -10..10 {
            for y in -10..10 {
                let color = if (x + y) % 2 == 0 {
                    [0.25, 0.55, 0.28, 1.0]
                } else {
                    [0.2, 0.48, 0.22, 1.0]
                };
                self.add_tile_block(TilePos::new(x, y), 0.15, color, vertices);
            }
        }
    }

    fn build_region_tiles(&self, region: &RegionDef, vertices: &mut Vec<Vertex>) {
        for y in 0..region.height {
            for x in 0..region.width {
                let idx = (y * region.width + x) as usize;
                let tile_pos = TilePos::new(x as i32, y as i32);
                let tile_type = region.tiles.get(idx).copied().unwrap_or(0);
                let is_portal = region
                    .transitions
                    .iter()
                    .any(|t| t.position.x == tile_pos.x && t.position.y == tile_pos.y);
                let (height, color) = if is_portal {
                    (0.18, [0.58, 0.22, 0.78, 1.0])
                } else {
                    match tile_type {
                        0 => (0.12, [0.25, 0.55, 0.28, 1.0]),
                        1 => (0.2, [0.45, 0.38, 0.25, 1.0]),
                        2 => (0.05, [0.2, 0.35, 0.65, 1.0]),
                        _ => (0.1, [0.35, 0.35, 0.35, 1.0]),
                    }
                };
                self.add_tile_block(tile_pos, height, color, vertices);
            }
        }
    }

    fn build_entity(
        &self,
        entity: &WorldEntity,
        region: Option<&RegionDef>,
        eye: Vec3,
        now: Instant,
        movement_interp: &EntityMovementInterp,
        content: &ContentPack,
        model_cache: &mut ModelCache,
        opaque_entity_vertices: &mut Vec<Vertex>,
        opaque_outline_vertices: &mut Vec<Vertex>,
        transparent_entity_vertices: &mut Vec<Vertex>,
        transparent_outline_vertices: &mut Vec<Vertex>,
        transparent_draws: &mut Vec<TransparentDraw>,
        model_draws: &mut Vec<ModelDraw>,
        local_player: Option<openmmo_common::PlayerId>,
    ) {
        let footprint = entity_footprint(content, entity);
        let visual_base = movement_interp
            .visual_center(entity.entity_id, now, region, footprint)
            .or_else(|| {
                entity_bounds::entity_tile(entity).map(|tile| {
                    let surface_y = Self::tile_surface_height(tile, region);
                    footprint.unwrap_or_default().world_center(tile, surface_y)
                })
            });

        let yaw = movement_interp
            .visual_facing_yaw(entity.entity_id, now)
            .unwrap_or(0.0);

        match &entity.kind {
            openmmo_common::EntityKind::Player { player_id, .. } => {
                let Some([cx, surface_y, cz]) = visual_base else {
                    return;
                };
                if self.player_model.is_some() {
                    let is_local = Some(*player_id) == local_player;
                    let tint = if is_local {
                        [1.0, 1.0, 1.0, 1.0]
                    } else {
                        [0.95, 0.9, 0.75, 1.0]
                    };
                    model_draws.push(ModelDraw {
                        target: ModelTarget::Player,
                        model: entity_model_matrix([cx, surface_y, cz], yaw),
                        tint,
                        entity_id: None,
                    });
                    return;
                }
                let (width, height) = entity_bounds::entity_cube_dims(&entity.kind);
                let color = if Some(*player_id) == local_player {
                    [0.2, 0.6, 1.0, 1.0]
                } else {
                    [0.9, 0.8, 0.2, 1.0]
                };
                self.add_entity_cube(
                    [cx, surface_y, cz],
                    eye,
                    width,
                    height,
                    color,
                    opaque_entity_vertices,
                    opaque_outline_vertices,
                    transparent_entity_vertices,
                    transparent_outline_vertices,
                    transparent_draws,
                );
            }
            openmmo_common::EntityKind::Npc { npc_id, hp, .. } => {
                let Some([cx, surface_y, cz]) = visual_base else {
                    return;
                };
                if self.npc_models.contains_key(npc_id) && *hp > 0 {
                    model_draws.push(ModelDraw {
                        target: ModelTarget::Npc(*npc_id),
                        model: entity_model_matrix([cx, surface_y, cz], yaw),
                        tint: [1.0, 1.0, 1.0, 1.0],
                        entity_id: Some(entity.entity_id),
                    });
                    return;
                }
                if self.is_skinned_npc(*npc_id) {
                    return;
                }
                let alive = *hp > 0;
                let fallback_color = if alive {
                    [0.85, 0.2, 0.2, 1.0]
                } else {
                    [0.4, 0.4, 0.4, 0.6]
                };
                if let Some(def) = content.npc(*npc_id) {
                    if let Some(model_name) = def.model.as_deref() {
                        if let Some(mesh) = model_cache.get(model_name) {
                            self.add_entity_mesh(
                                [cx, surface_y, cz],
                                eye,
                                mesh,
                                alive,
                                fallback_color,
                                opaque_entity_vertices,
                                transparent_entity_vertices,
                                transparent_draws,
                            );
                            return;
                        }
                    }
                }
                let (width, height) = entity_bounds::entity_cube_dims(&entity.kind);
                self.add_entity_cube(
                    [cx, surface_y, cz],
                    eye,
                    width,
                    height,
                    fallback_color,
                    opaque_entity_vertices,
                    opaque_outline_vertices,
                    transparent_entity_vertices,
                    transparent_outline_vertices,
                    transparent_draws,
                );
            }
            openmmo_common::EntityKind::Boss { hp, .. } => {
                let Some([cx, surface_y, cz]) = visual_base else {
                    return;
                };
                let (width, height) = entity_bounds::entity_cube_dims(&entity.kind);
                let color = if *hp > 0 {
                    [0.6, 0.1, 0.8, 1.0]
                } else {
                    [0.3, 0.3, 0.3, 0.6]
                };
                self.add_entity_cube(
                    [cx, surface_y, cz],
                    eye,
                    width,
                    height,
                    color,
                    opaque_entity_vertices,
                    opaque_outline_vertices,
                    transparent_entity_vertices,
                    transparent_outline_vertices,
                    transparent_draws,
                );
            }
            openmmo_common::EntityKind::Object { .. } => {
                let Some([cx, surface_y, cz]) = visual_base else {
                    return;
                };
                let (width, height) = entity_bounds::entity_cube_dims(&entity.kind);
                self.add_entity_cube(
                    [cx, surface_y, cz],
                    eye,
                    width,
                    height,
                    [0.5, 0.3, 0.15, 1.0],
                    opaque_entity_vertices,
                    opaque_outline_vertices,
                    transparent_entity_vertices,
                    transparent_outline_vertices,
                    transparent_draws,
                );
            }
            openmmo_common::EntityKind::GroundItem { .. } => {
                let Some([cx, surface_y, cz]) = visual_base else {
                    return;
                };
                let (width, height) = entity_bounds::entity_cube_dims(&entity.kind);
                self.add_entity_cube(
                    [cx, surface_y, cz],
                    eye,
                    width,
                    height,
                    [1.0, 0.85, 0.0, 1.0],
                    opaque_entity_vertices,
                    opaque_outline_vertices,
                    transparent_entity_vertices,
                    transparent_outline_vertices,
                    transparent_draws,
                );
            }
        }
    }

    fn tile_surface_height(tile: TilePos, region: Option<&RegionDef>) -> f32 {
        entity_bounds::tile_surface_height(tile, region)
    }

    fn tile_center(tile: TilePos) -> [f32; 3] {
        [tile.x as f32 + 0.5, 0.0, tile.y as f32 + 0.5]
    }

    fn add_tile_block(
        &self,
        tile: TilePos,
        height: f32,
        color: [f32; 4],
        vertices: &mut Vec<Vertex>,
    ) {
        let [cx, _, cz] = Self::tile_center(tile);
        add_box(
            [cx, height * 0.5, cz],
            [1.0, height.max(0.05), 1.0],
            color,
            vertices,
            false,
        );
    }

    fn add_entity_cube(
        &self,
        base: [f32; 3],
        eye: Vec3,
        width: f32,
        height: f32,
        color: [f32; 4],
        opaque_entity_vertices: &mut Vec<Vertex>,
        opaque_outline_vertices: &mut Vec<Vertex>,
        transparent_entity_vertices: &mut Vec<Vertex>,
        transparent_outline_vertices: &mut Vec<Vertex>,
        transparent_draws: &mut Vec<TransparentDraw>,
    ) {
        let [cx, surface_y, cz] = base;
        let center = [cx, surface_y + height * 0.5, cz];
        let size = [width, height, width];
        let transparent = color[3] < 1.0;

        let (entity_vertices, outline_vertices) = if transparent {
            (transparent_entity_vertices, transparent_outline_vertices)
        } else {
            (opaque_entity_vertices, opaque_outline_vertices)
        };

        let solid_start = entity_vertices.len() as u32;
        add_box(center, size, color, entity_vertices, true);
        let solid_len = entity_vertices.len() as u32 - solid_start;

        let outline_start = outline_vertices.len() as u32;
        add_box_edge_lines(center, size, [0.0, 0.0, 0.0, 1.0], outline_vertices, true);
        let outline_len = outline_vertices.len() as u32 - outline_start;

        if transparent {
            let dx = center[0] - eye.x;
            let dy = center[1] - eye.y;
            let dz = center[2] - eye.z;
            transparent_draws.push(TransparentDraw {
                solid_start,
                solid_len,
                outline_start,
                outline_len,
                sort_key: dx * dx + dy * dy + dz * dz,
            });
        }
    }

    fn add_entity_mesh(
        &self,
        base: [f32; 3],
        eye: Vec3,
        mesh: &Mesh,
        alive: bool,
        dead_color: [f32; 4],
        opaque_entity_vertices: &mut Vec<Vertex>,
        transparent_entity_vertices: &mut Vec<Vertex>,
        transparent_draws: &mut Vec<TransparentDraw>,
    ) {
        let [cx, surface_y, cz] = base;
        let transparent = !alive;
        let entity_vertices = if transparent {
            transparent_entity_vertices
        } else {
            opaque_entity_vertices
        };

        let solid_start = entity_vertices.len() as u32;
        for face in &mesh.faces {
            let color = if alive { face.color } else { dead_color };
            for pos in face.positions {
                entity_vertices.push(Vertex {
                    position: [cx + pos[0], surface_y + pos[1], cz + pos[2]],
                    color,
                });
            }
        }
        let solid_len = entity_vertices.len() as u32 - solid_start;

        if transparent {
            let center_y = surface_y + mesh.height * 0.5;
            let dx = cx - eye.x;
            let dy = center_y - eye.y;
            let dz = cz - eye.z;
            transparent_draws.push(TransparentDraw {
                solid_start,
                solid_len,
                outline_start: 0,
                outline_len: 0,
                sort_key: dx * dx + dy * dy + dz * dz,
            });
        }
    }
}

fn add_box(
    center: [f32; 3],
    size: [f32; 3],
    color: [f32; 4],
    vertices: &mut Vec<Vertex>,
    skip_bottom: bool,
) {
    let [cx, cy, cz] = center;
    let [sx, sy, sz] = size;
    let hx = sx * 0.5;
    let hy = sy * 0.5;
    let hz = sz * 0.5;

    let corners = [
        [cx - hx, cy - hy, cz - hz],
        [cx + hx, cy - hy, cz - hz],
        [cx + hx, cy + hy, cz - hz],
        [cx - hx, cy + hy, cz - hz],
        [cx - hx, cy - hy, cz + hz],
        [cx + hx, cy - hy, cz + hz],
        [cx + hx, cy + hy, cz + hz],
        [cx - hx, cy + hy, cz + hz],
    ];

    // Outward CCW in world space; pairs with +f projection and FrontFace::Ccw culling.
    let faces: [([usize; 4], [f32; 3]); 6] = [
        ([0, 3, 2, 1], [0.0, 0.0, -1.0]),
        ([5, 6, 7, 4], [0.0, 0.0, 1.0]),
        ([4, 7, 3, 0], [-1.0, 0.0, 0.0]),
        ([1, 2, 6, 5], [1.0, 0.0, 0.0]),
        ([3, 7, 6, 2], [0.0, 1.0, 0.0]),
        ([4, 0, 1, 5], [0.0, -1.0, 0.0]),
    ];

    for (face_idx, (indices, normal)) in faces.iter().enumerate() {
        if skip_bottom && face_idx == 5 {
            continue;
        }
        let shade = 0.65 + 0.35 * (normal[1].max(0.0) + normal[2].abs() * 0.15);
        let face_color = [
            (color[0] * shade).min(1.0),
            (color[1] * shade).min(1.0),
            (color[2] * shade).min(1.0),
            color[3],
        ];
        let [a, b, c, d] = *indices;
        push_tri(corners[a], corners[b], corners[c], face_color, vertices);
        push_tri(corners[a], corners[c], corners[d], face_color, vertices);
    }
}

fn add_box_edge_lines(
    center: [f32; 3],
    size: [f32; 3],
    color: [f32; 4],
    vertices: &mut Vec<Vertex>,
    skip_bottom: bool,
) {
    let [cx, cy, cz] = center;
    let [sx, sy, sz] = size;
    let hx = sx * 0.5;
    let hy = sy * 0.5;
    let hz = sz * 0.5;

    let corners = [
        [cx - hx, cy - hy, cz - hz],
        [cx + hx, cy - hy, cz - hz],
        [cx + hx, cy + hy, cz - hz],
        [cx - hx, cy + hy, cz - hz],
        [cx - hx, cy - hy, cz + hz],
        [cx + hx, cy - hy, cz + hz],
        [cx + hx, cy + hy, cz + hz],
        [cx - hx, cy + hy, cz + hz],
    ];

    let edges: &[[usize; 2]] = if skip_bottom {
        &[
            [3, 2],
            [2, 6],
            [6, 7],
            [7, 3],
            [0, 3],
            [1, 2],
            [5, 6],
            [4, 7],
        ]
    } else {
        &[
            [0, 1],
            [1, 5],
            [5, 4],
            [4, 0],
            [3, 2],
            [2, 6],
            [6, 7],
            [7, 3],
            [0, 3],
            [1, 2],
            [5, 6],
            [4, 7],
        ]
    };

    for &[a, b] in edges {
        push_line(corners[a], corners[b], color, vertices);
    }
}

fn push_line(a: [f32; 3], b: [f32; 3], color: [f32; 4], vertices: &mut Vec<Vertex>) {
    vertices.push(Vertex { position: a, color });
    vertices.push(Vertex { position: b, color });
}

fn push_tri(a: [f32; 3], b: [f32; 3], c: [f32; 3], color: [f32; 4], vertices: &mut Vec<Vertex>) {
    for position in [a, b, c] {
        vertices.push(Vertex { position, color });
    }
}

fn update_skinned_npc_clip(
    player: &mut AnimationPlayer,
    movement_interp: &EntityMovementInterp,
    entity_id: EntityId,
    now: Instant,
) {
    if player.is_playing(FROG_ATTACK) && !player.finished {
        return;
    }
    if movement_interp.is_moving(entity_id, now) {
        if !player.is_playing(FROG_JUMP) {
            player.play(FROG_JUMP, true);
        }
    } else if player.current_clip() != Some(FROG_IDLE) {
        player.play(FROG_IDLE, true);
    }
}

fn entity_footprint(content: &ContentPack, entity: &WorldEntity) -> Option<NpcFootprint> {
    match &entity.kind {
        openmmo_common::EntityKind::Npc { npc_id, .. } => {
            content.npc(*npc_id).map(NpcFootprint::from_def)
        }
        _ => None,
    }
}
