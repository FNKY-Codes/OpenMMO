use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use openmmo_common::{RegionDef, TilePos, WorldEntity};

use crate::camera::Camera;

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

pub struct Renderer {
    window: std::sync::Arc<egui_winit::winit::window::Window>,
    gpu: GpuState,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    camera: Camera,
}

impl Renderer {
    pub async fn new(window: std::sync::Arc<egui_winit::winit::window::Window>) -> Self {
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
            label: Some("World Pipeline"),
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
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
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

        Self {
            window,
            gpu: GpuState {
                device,
                queue,
                surface,
                config,
            },
            pipeline,
            vertex_buffer,
            uniform_buffer,
            uniform_bind_group,
            depth_texture,
            depth_view,
            camera: Camera::new(),
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

    pub fn begin_frame(
        &mut self,
    ) -> Result<(wgpu::SurfaceTexture, wgpu::TextureView, wgpu::CommandEncoder), wgpu::SurfaceError>
    {
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
        local_player: Option<openmmo_common::PlayerId>,
    ) {
        let vp = self
            .camera
            .view_projection(self.gpu.config.width, self.gpu.config.height);
        self.gpu.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&Uniforms {
                view_proj: vp.cols,
            }),
        );

        let mut vertices = Vec::new();

        if let Some(region) = region {
            self.build_region_tiles(region, &mut vertices);
        } else {
            self.build_test_map(&mut vertices);
        }

        for entity in entities {
            self.build_entity(entity, &mut vertices, local_player);
        }

        if !vertices.is_empty() {
            self.gpu.queue.write_buffer(
                &self.vertex_buffer,
                0,
                bytemuck::cast_slice(&vertices),
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

        if !vertices.is_empty() {
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..vertices.len() as u32, 0..1);
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
                let tile_type = region.tiles.get(idx).copied().unwrap_or(0);
                let (height, color) = match tile_type {
                    0 => (0.12, [0.25, 0.55, 0.28, 1.0]),
                    1 => (0.2, [0.45, 0.38, 0.25, 1.0]),
                    2 => (0.05, [0.2, 0.35, 0.65, 1.0]),
                    _ => (0.1, [0.35, 0.35, 0.35, 1.0]),
                };
                self.add_tile_block(TilePos::new(x as i32, y as i32), height, color, vertices);
            }
        }
    }

    fn build_entity(
        &self,
        entity: &WorldEntity,
        vertices: &mut Vec<Vertex>,
        local_player: Option<openmmo_common::PlayerId>,
    ) {
        match &entity.kind {
            openmmo_common::EntityKind::Player {
                player_id,
                position,
                ..
            } => {
                let color = if Some(*player_id) == local_player {
                    [0.2, 0.6, 1.0, 1.0]
                } else {
                    [0.9, 0.8, 0.2, 1.0]
                };
                self.add_entity_cube(*position, 0.9, 1.8, color, vertices);
            }
            openmmo_common::EntityKind::Npc {
                position, hp, ..
            } => {
                let color = if *hp > 0 {
                    [0.85, 0.2, 0.2, 1.0]
                } else {
                    [0.4, 0.4, 0.4, 0.6]
                };
                self.add_entity_cube(*position, 0.8, 1.6, color, vertices);
            }
            openmmo_common::EntityKind::Object { position, .. } => {
                self.add_entity_cube(*position, 0.7, 1.2, [0.5, 0.3, 0.15, 1.0], vertices);
            }
            openmmo_common::EntityKind::GroundItem { position, .. } => {
                self.add_entity_cube(*position, 0.35, 0.35, [1.0, 0.85, 0.0, 1.0], vertices);
            }
        }
    }

    fn tile_center(tile: TilePos) -> [f32; 3] {
        [
            tile.x as f32 + 0.5,
            0.0,
            tile.y as f32 + 0.5,
        ]
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
        );
    }

    fn add_entity_cube(
        &self,
        tile: TilePos,
        width: f32,
        height: f32,
        color: [f32; 4],
        vertices: &mut Vec<Vertex>,
    ) {
        let [cx, _, cz] = Self::tile_center(tile);
        add_box([cx, height * 0.5, cz], [width, height, width], color, vertices);
    }
}

fn add_box(center: [f32; 3], size: [f32; 3], color: [f32; 4], vertices: &mut Vec<Vertex>) {
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

    let faces: [([usize; 4], [f32; 3]); 6] = [
        ([0, 1, 2, 3], [0.0, 0.0, -1.0]),
        ([5, 4, 7, 6], [0.0, 0.0, 1.0]),
        ([4, 0, 3, 7], [-1.0, 0.0, 0.0]),
        ([1, 5, 6, 2], [1.0, 0.0, 0.0]),
        ([3, 2, 6, 7], [0.0, 1.0, 0.0]),
        ([4, 5, 1, 0], [0.0, -1.0, 0.0]),
    ];

    for (indices, normal) in faces {
        let shade = 0.65 + 0.35 * (normal[1].max(0.0) + normal[2].abs() * 0.15);
        let face_color = [
            (color[0] * shade).min(1.0),
            (color[1] * shade).min(1.0),
            (color[2] * shade).min(1.0),
            color[3],
        ];
        let [a, b, c, d] = indices;
        push_tri(corners[a], corners[b], corners[c], face_color, vertices);
        push_tri(corners[a], corners[c], corners[d], face_color, vertices);
    }
}

fn push_tri(
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    color: [f32; 4],
    vertices: &mut Vec<Vertex>,
) {
    for position in [a, b, c] {
        vertices.push(Vertex { position, color });
    }
}