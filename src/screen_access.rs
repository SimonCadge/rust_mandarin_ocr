use std::{mem, time::{Instant, Duration}};

use abort_on_drop::ChildTask;
use bytemuck::{Pod, Zeroable};
use chinese_dictionary::tokenize;
use configparser::ini::Ini;
use html_parser::Node;
use tokio::sync::{watch, mpsc};
use wgpu::{BufferUsages, SurfaceConfiguration};
use wgpu_glyph::{GlyphBrush, ab_glyph, GlyphBrushBuilder, OwnedSection};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, Window}, dpi::{PhysicalSize, PhysicalPosition, Size},
};

use crate::{ocr, positioning_structs::{PresentableLine, PixelPoint, HocrWord}, supported_languages::SupportedLanguages};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 3],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3];
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

struct WindowState {
    window: Window,
    surface: wgpu::Surface,
    config: SurfaceConfiguration,
    size: PhysicalSize<u32>
}

impl WindowState {
    fn resize(&mut self, device: &wgpu::Device, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(device, &self.config);
            if self.window.inner_size() != new_size { //When we have changed the size in code
                self.window.set_inner_size(new_size);
            }
        }
    }

    fn set_visible(&mut self, is_visible: bool) {
        self.window.set_visible(is_visible);
    }
}

struct State {
    main_window_state: WindowState,
    popup_window_state: WindowState,
    device: wgpu::Device,
    queue: wgpu::Queue,
    staging_belt: wgpu::util::StagingBelt,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    popup_text: Option<OwnedSection>,
    glyph_brush: GlyphBrush<()>,
    _ocr_thread: ChildTask<()>,
    ocr_job_timer: Option<Instant>,
    ocr_send_channel: watch::Sender<(i32, i32, u32, u32)>,
    ocr_receive_channel: mpsc::Receiver<String>,
    ocr_text: Option<Vec<PresentableLine>>,
    config_parser: Ini,
    language: SupportedLanguages,
}

impl State {
    // Creating some of the wgpu types requires async code
    async fn new(main_window: Window, popup_window: Window, mut config_parser: Ini) -> Self {
        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });
        
        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let main_window_surface = unsafe { instance.create_surface(&main_window) }.unwrap();
        let popup_window_surface = unsafe { instance.create_surface(&popup_window) }.unwrap();
        
        let adapter = instance
        .enumerate_adapters(wgpu::Backends::all())
        .filter(|adapter| {
            // Check if this adapter supports our surface
            adapter.is_surface_supported(&main_window_surface)
        })
        .next()
        .unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
                // WebGL doesn't support all of wgpu's features, so if
                // we're building for the web we'll have to disable some.
                limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                label: None,
            },
            None, // Trace path
        ).await.unwrap();

        let surface_caps = main_window_surface.get_capabilities(&adapter);

        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps.formats.iter()
            .copied()
            .filter(|f| f.describe().srgb)
            .next()
            .unwrap_or(surface_caps.formats[0]);

        let main_window_state = configure_main_window(main_window, surface_format, &surface_caps, main_window_surface, &device);
        popup_window.set_visible(false);
        let popup_window_state = configure_popup_window(popup_window, surface_format, &surface_caps, popup_window_surface, &device);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { 
            label: Some("Shader"), 
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()), 
        });

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    Vertex::desc()
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: main_window_state.config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState { 
                topology: wgpu::PrimitiveTopology::TriangleList, 
                strip_index_format: None, 
                front_face: wgpu::FrontFace::Cw, 
                cull_mode: Some(wgpu::Face::Back), 
                unclipped_depth: false, 
                polygon_mode: wgpu::PolygonMode::Fill, 
                conservative: false },
            depth_stencil: None,
            multisample: wgpu::MultisampleState { 
                count: 1, 
                mask: !0, 
                alpha_to_coverage_enabled: false 
            },
            multiview: None,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: 10000 * mem::size_of::<Vertex>() as u64, //Assuming we never need more than 1000 vertices
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Index Buffer"),
            size: 10000 * mem::size_of::<u16>() as u64,
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Prepare glyph_brush
        let simhei = ab_glyph::FontArc::try_from_slice(include_bytes!(
            "SimHei.ttf"
        )).unwrap();

        let glyph_brush = GlyphBrushBuilder::using_font(simhei.clone())
            .build(&device, surface_format);

        let (main_thread_send_channel, worker_thread_receive_channel) = watch::channel((0, 0, 0, 0));
        let (worker_thread_send_channel, main_thread_receive_channel) = mpsc::channel(1);

        let language = serde_json::from_str::<SupportedLanguages>(
            &config_parser.get("other", "language").or(Some("\"ChiTra\"".to_string())).unwrap()
        ).expect("Expected language ChiTra or ChiSim");
        config_parser.set("other", "language", Some(serde_json::to_string(&language).unwrap()));
        
        let _ocr_thread = ChildTask::from(tokio::task::spawn_blocking(move || {
            ocr::build_ocr_worker(worker_thread_receive_channel, worker_thread_send_channel, language);
        }));

        Self {
            main_window_state,
            popup_window_state,
            device,
            queue,
            staging_belt: wgpu::util::StagingBelt::new(1024),
            render_pipeline,
            vertex_buffer,
            index_buffer,
            glyph_brush,
            _ocr_thread,
            ocr_job_timer: None,
            ocr_send_channel: main_thread_send_channel,
            ocr_receive_channel: main_thread_receive_channel,
            ocr_text: None,
            popup_text: None,
            config_parser,
            language
        }
    }

    fn input(&mut self, _event: &WindowEvent) -> bool {
        false
    }

    fn schedule_ocr_job(&mut self) {
        if self.ocr_text.is_some() {
            self.ocr_text = None;
        }
        self.ocr_job_timer = Instant::now().checked_add(Duration::from_millis(200));
    }

    fn check_running_job(&mut self) {
        if let Ok(ocr_text) = self.ocr_receive_channel.try_recv() {
            self.ocr_text = Some(self.nodes_to_lines(&html_parser::Dom::parse(&ocr_text).unwrap().children));
            self.render_main_window().unwrap();
        }
    }

    fn render_main_window(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.main_window_state.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);

            if let Some(lines) = &self.ocr_text {
                let mut vertices: Vec<Vertex> = Vec::with_capacity(10000 * mem::size_of::<Vertex>());
                let mut indices: Vec<u32> = Vec::with_capacity(10000 * mem::size_of::<u32>());
                let mut offset = 0;
                let mut num_indices = 0;
                let screen_size = PixelPoint::new(self.main_window_state.config.width as f32, self.main_window_state.config.height as f32);
                for line in lines {
                    let (mut line_vertices, mut line_indices) = line.generate_bounding_vertices(screen_size, offset);
                    offset += line_vertices.len() as u32;
                    vertices.append(&mut line_vertices);
                    num_indices += line_indices.len() as u32;
                    indices.append(&mut line_indices);
                }
                self.queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
                self.queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));

                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..num_indices, 0, 0..1);
            }
        }

        if let Some(lines) = &self.ocr_text {
            for line in lines {
                let section = line.get_section();
                self.glyph_brush.queue(section);
            }
            self.glyph_brush.draw_queued(&self.device, &mut self.staging_belt, &mut encoder, &view, self.main_window_state.size.width, self.main_window_state.size.height).unwrap();
        }
    
        self.staging_belt.finish();
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.staging_belt.recall();
    
        Ok(())
    }
    
    fn render_popup_window(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.popup_window_state.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
        }
        
        if let Some(text) = &self.popup_text {
            self.glyph_brush.queue(text);
            self.glyph_brush.draw_queued(&self.device, &mut self.staging_belt, &mut encoder, &view, self.popup_window_state.size.width, self.popup_window_state.size.height).unwrap();
        }

        self.staging_belt.finish();
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.staging_belt.recall();
    
        Ok(())
    }

    fn handle_cursor(&mut self, cursor_position: &PixelPoint) {
        if let Some(bbox_lines) = &mut self.ocr_text {
            for line in bbox_lines {
                line.handle_cursor(cursor_position);
            }
            self.render_main_window().unwrap();
        }
    }

    fn handle_click(&mut self) {
        let mut something_clicked = false;
        if let Some(lines) = &self.ocr_text {
            for line in lines {
                for word in line.get_words() {
                    if word.is_highlighted() {
                        let (text_section, bounds) = word.generate_translation_section(&mut self.glyph_brush, &self.language);
                        if let Some(bounds) = bounds {
                            self.popup_text = Some(text_section);
                            let new_size = PhysicalSize { 
                                width: (bounds.max.x - bounds.min.x) as u32, 
                                height: (bounds.max.y - bounds.min.y) as u32 
                            };
                            self.popup_window_state.resize(&self.device, new_size);
                            self.popup_window_state.set_visible(true);
                            let main_window_position = self.main_window_state.window.inner_position().unwrap();
                            let popup_new_position = PhysicalPosition {
                                x: main_window_position.x as u32 + word.get_min().get_x() as u32 - (new_size.width / 2) + (line.get_scale().x as u32 / 2),
                                y: main_window_position.y as u32 + word.get_min().get_y() as u32 - new_size.height - 10,
                            };
                            self.popup_window_state.window.set_outer_position(popup_new_position);
                            self.popup_window_state.window.set_window_level(winit::window::WindowLevel::AlwaysOnTop);
                            self.popup_window_state.window.request_redraw();
                            something_clicked = true;
                        }
                    }
                }
            }
        }
        if !something_clicked {
            self.popup_text = None;
            self.popup_window_state.set_visible(false);
            self.popup_window_state.window.request_redraw();
        }
    }

    fn nodes_to_lines(&mut self, nodes: &Vec<Node>) -> Vec<PresentableLine> {
        let mut lines: Vec<PresentableLine> = Vec::new();
        for node in nodes {
            if let html_parser::Node::Element(element) = node {
                if element.classes.contains(&"ocr_line".to_string()) { // is individual line
                    let num_words = element.children.len();
                    let mut words = Vec::with_capacity(num_words);
                    for word in &element.children {
                        if let html_parser::Node::Element(word_element) = word {
                            let title = word_element.attributes["title"].clone().unwrap();
                            let mut parts = title.split(" ");
                            parts.next();
                            let x = parse_bbox_f32(parts.next().unwrap()); //Bounds reported by tesseract, unfortunately they're very inaccurate
                            let y = parse_bbox_f32(parts.next().unwrap()); //The sizes are accurate but the positions aren't
                            let x2 = parse_bbox_f32(parts.next().unwrap());
                            let y2 = parse_bbox_f32(parts.next().unwrap());
                            parts.next();
                            let confidence = parts.next().unwrap().parse::<f32>().unwrap();
                            let text = get_text_child(&word_element.children);
                            let word = HocrWord::new(
                                text,
                                PixelPoint::new(x, y),
                                PixelPoint::new(x2, y2),
                                confidence,
                            );
                            words.push(word);
                        }
                    }
                    let raw_text: String = words.iter().map(|hocr_word| hocr_word.get_text().to_string()).collect();
                    let tokenized_text = tokenize(&raw_text);
                    let mut tokenized_words = Vec::with_capacity(tokenized_text.len());
                    let mut i = 0;
                    for token in tokenized_text {
                        let first_char = token.as_bytes()[0];
                        if let Some((index, _word)) = words.iter().map(|hocr_word| hocr_word.get_text()).enumerate().skip(i).find(|(_i, word)| word.as_bytes()[0] == first_char) {
                            for y in i .. index {
                                tokenized_words.push(words[y].clone());
                            }
                            i = index;
                            let len = token.chars().count();
                            tokenized_words.push(words[i+1 .. i+len].iter().fold(words[i].clone(), |lhs, rhs| lhs + rhs));
                            i += len;
                        }
                    }if !tokenized_words.is_empty() {
                        let line = PresentableLine::from_hocr(tokenized_words, &mut self.glyph_brush);
                        lines.push(line);
                    }
                } else { // call recursively until we reach individual words
                    lines.append(&mut self.nodes_to_lines(&node.element().unwrap().children));
                }
            }
        }
        return lines;
    }
    
}

fn configure_main_window(window: Window, surface_format: wgpu::TextureFormat, surface_caps: &wgpu::SurfaceCapabilities, surface: wgpu::Surface, device: &wgpu::Device) -> WindowState {
    let size = window.inner_size();
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        format: surface_format,
        width: size.width,
        height: size.height,
        present_mode: surface_caps.present_modes[0],
        alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
        view_formats: vec![],
    };
    surface.configure(device, &config);
    WindowState {
        window,
        surface,
        config,
        size
    }
}

fn configure_popup_window(window: Window, surface_format: wgpu::TextureFormat, surface_caps: &wgpu::SurfaceCapabilities, surface: wgpu::Surface, device: &wgpu::Device) -> WindowState {
    let size = window.inner_size();
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        format: surface_format,
        width: size.width,
        height: size.height,
        present_mode: surface_caps.present_modes[0],
        alpha_mode: wgpu::CompositeAlphaMode::Opaque,
        view_formats: vec![],
    };
    surface.configure(device, &config);
    WindowState {
        window,
        surface,
        config,
        size
    }
}

fn get_text_child(nodes: &Vec<Node>) -> String {
    for node in nodes {
        if let html_parser::Node::Text(text) = node {
            return text.to_string();
        } else if let html_parser::Node::Element(element) = node {
            return get_text_child(&element.children);
        }
    }
    return "".to_string();
}

fn parse_bbox_f32(string: &str) -> f32 {
    let parsed = string.chars().filter(|char| char.is_digit(10)).collect::<String>().parse::<f32>().unwrap();
    return parsed / 4.0; //OCR image was upscaled 4x before processing
}

pub async fn screen_entry() {
    env_logger::init();
    let mut config_parser = Ini::new();
    config_parser.load("config.ini").unwrap_or_default();
    let event_loop = EventLoop::new();
    let window_width = config_parser.getfloat("screen", "width").unwrap().or(Some(100.0)).unwrap();
    let window_height = config_parser.getfloat("screen", "height").unwrap().or(Some(50.0)).unwrap();
    let window_x = config_parser.getfloat("screen", "x_pos").unwrap().or(Some(100.0)).unwrap();
    let window_y = config_parser.getfloat("screen", "y_pos").unwrap().or(Some(100.0)).unwrap();
    let main_window = WindowBuilder::new()
        .with_transparent(true)
        .with_inner_size(PhysicalSize::new(window_width, window_height))
        .with_position(PhysicalPosition::new(window_x, window_y))
        .build(&event_loop).unwrap();
    let main_window_id = main_window.id();
    let popup_window = WindowBuilder::new().with_decorations(false).build(&event_loop).unwrap();
    let popup_window_id = popup_window.id();

    let mut window_state = State::new(main_window, popup_window, config_parser).await;

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window_state.main_window_state.window.id() => if !window_state.input(event) {
                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => {
                        window_state.config_parser.write("config.ini").unwrap();
                        *control_flow = ControlFlow::Exit}
                        ,
                    WindowEvent::Resized(physical_size) => {
                        window_state.main_window_state.resize(&window_state.device, *physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        window_state.main_window_state.resize(&window_state.device, **new_inner_size);
                    }
                    WindowEvent::Moved(_) => {
                        window_state.main_window_state.window.request_redraw();
                    }
                    WindowEvent::CursorMoved { device_id: _, position, modifiers: _ } => {
                        window_state.handle_cursor(&PixelPoint::from(position));
                    }
                    WindowEvent::MouseInput { device_id: _, state, button: _, modifiers: _ } => {
                        if let ElementState::Released = state {
                            window_state.handle_click();
                        }
                    }
                    _ => {}
                }
            }
            Event::RedrawRequested(window_id) => {
                match window_id {
                    _ if window_id == main_window_id => {
                        window_state.schedule_ocr_job();
                        match window_state.render_main_window() {
                            Ok(_) => {}
                            // Reconfigure the surface if lost
                            Err(wgpu::SurfaceError::Lost) => window_state.main_window_state.resize(&window_state.device, window_state.main_window_state.size),
                            // The system is out of memory, we should probably quit
                            Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                            // All other errors (Outdated, Timeout) should be resolved by the next frame
                            Err(e) => eprintln!("{:?}", e),
                        }
                    },
                    _ if window_id == popup_window_id => {
                        match window_state.render_popup_window() {
                            Ok(_) => {}
                            // Reconfigure the surface if lost
                            Err(wgpu::SurfaceError::Lost) => window_state.popup_window_state.resize(&window_state.device, window_state.popup_window_state.size),
                            // The system is out of memory, we should probably quit
                            Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                            // All other errors (Outdated, Timeout) should be resolved by the next frame
                            Err(e) => eprintln!("{:?}", e),
                        }
                    },
                    _ => {}
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                if let Some(trigger_time) = window_state.ocr_job_timer {
                    if trigger_time <= Instant::now() {
                        window_state.ocr_job_timer = None;
                        let window_size = window_state.main_window_state.window.inner_size();
                        window_state.config_parser.set("screen", "width", Some((window_size.width as f64).to_string()));
                        window_state.config_parser.set("screen", "height", Some((window_size.height as f64).to_string()));
                        let window_inner_position = window_state.main_window_state.window.inner_position().unwrap();
                        let window_outer_position = window_state.main_window_state.window.outer_position().unwrap();
                        window_state.config_parser.set("screen", "x_pos", Some((window_outer_position.x as f64).to_string()));
                        window_state.config_parser.set("screen", "y_pos", Some((window_outer_position.y as f64).to_string()));
                        window_state.ocr_send_channel.send((window_inner_position.x, window_inner_position.y, window_size.width, window_size.height)).unwrap();
                    }
                }
                window_state.check_running_job();
                // state.window().request_redraw();
            }
            _ => {}
        }
    });
    
}