use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use openmmo_common::{RegionDef, TilePos, WorldEntity, tile_to_screen};

use crate::camera::Camera;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tile Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Tile Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
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
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
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
            camera: Camera::new(),
        }
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
            self.gpu.surface.configure(&self.gpu.device, &self.gpu.config);
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
                        r: 0.08,
                        g: 0.12,
                        b: 0.08,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        if !vertices.is_empty() {
            render_pass.set_pipeline(&self.pipeline);
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
                    [0.2, 0.45, 0.2, 1.0]
                } else {
                    [0.15, 0.38, 0.15, 1.0]
                };
                self.add_iso_tile(TilePos::new(x, y), color, vertices);
            }
        }
    }

    fn build_region_tiles(&self, region: &RegionDef, vertices: &mut Vec<Vertex>) {
        for y in 0..region.height {
            for x in 0..region.width {
                let idx = (y * region.width + x) as usize;
                let tile_type = region.tiles.get(idx).copied().unwrap_or(0);
                let color = match tile_type {
                    0 => [0.2, 0.45, 0.2, 1.0],
                    1 => [0.35, 0.3, 0.2, 1.0],
                    2 => [0.15, 0.25, 0.5, 1.0],
                    _ => [0.25, 0.25, 0.25, 1.0],
                };
                self.add_iso_tile(TilePos::new(x as i32, y as i32), color, vertices);
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
                self.add_entity_marker(*position, color, vertices);
            }
            openmmo_common::EntityKind::Npc {
                position, hp, ..
            } => {
                let color = if *hp > 0 {
                    [0.8, 0.2, 0.2, 1.0]
                } else {
                    [0.4, 0.4, 0.4, 0.5]
                };
                self.add_entity_marker(*position, color, vertices);
            }
            openmmo_common::EntityKind::Object { position, .. } => {
                self.add_entity_marker(*position, [0.4, 0.25, 0.1, 1.0], vertices);
            }
            openmmo_common::EntityKind::GroundItem { position, .. } => {
                self.add_entity_marker(*position, [1.0, 0.85, 0.0, 1.0], vertices);
            }
        }
    }

    fn add_iso_tile(&self, tile: TilePos, color: [f32; 4], vertices: &mut Vec<Vertex>) {
        let (cx, cy) = tile_to_screen(tile, self.camera.x, self.camera.y);
        let hw = 32.0 * self.camera.zoom;
        let hh = 16.0 * self.camera.zoom;

        let corners = [
            [cx, cy - hh],
            [cx + hw, cy],
            [cx, cy + hh],
            [cx - hw, cy],
        ];

        let indices = [[0, 1, 2], [0, 2, 3]];
        for tri in indices {
            for &i in &tri {
                vertices.push(Vertex {
                    position: corners[i],
                    color,
                });
            }
        }
    }

    fn add_entity_marker(&self, tile: TilePos, color: [f32; 4], vertices: &mut Vec<Vertex>) {
        let (cx, cy) = tile_to_screen(tile, self.camera.x, self.camera.y);
        let s = 8.0 * self.camera.zoom;
        let verts = [
            [cx - s, cy - s],
            [cx + s, cy - s],
            [cx + s, cy + s],
            [cx - s, cy - s],
            [cx + s, cy + s],
            [cx - s, cy + s],
        ];
        for v in verts {
            vertices.push(Vertex {
                position: v,
                color,
            });
        }
    }
}
