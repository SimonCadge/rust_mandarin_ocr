use std::{time::Instant, io};

use html_parser::{Dom, Node};
use pollster::block_on;
use tokio::{task, runtime::{Runtime, self}};
use wgpu_glyph::{GlyphBrush, ab_glyph, GlyphBrushBuilder, Section, Text};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, Window}, dpi::PhysicalPosition,
};
use screenshots::Screen;

use crate::ocr;

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    staging_belt: wgpu::util::StagingBelt,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,
    glyph_brush: GlyphBrush<()>,
    tokio_runtime: Runtime,
    ocr_job: Option<task::JoinHandle<Result<String, io::Error>>>,
    ocr_text: Option<Vec<BboxWord>>,
}

struct BboxWord {
    text: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    is_highlighted: bool,
}

impl BboxWord {
    fn is_within_bounds(&mut self, position: &PhysicalPosition<f64>) -> bool {
        let cursor_x: f32 = position.x as f32;
        let cursor_y: f32 = position.y as f32;
        return cursor_x > self.x && cursor_x <= (self.x + self.width)
            && cursor_y > self.y && cursor_y <= (self.y + self.height);
    }
}

impl State {
    // Creating some of the wgpu types requires async code
    async fn new(window: Window) -> Self {
        let size = window.inner_size();

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
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
        .enumerate_adapters(wgpu::Backends::all())
        .filter(|adapter| {
            // Check if this adapter supports our surface
            adapter.is_surface_supported(&surface)
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

        let surface_caps = surface.get_capabilities(&adapter);

        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps.formats.iter()
            .copied()
            .filter(|f| f.describe().srgb)
            .next()
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        // Prepare glyph_brush
        let inconsolata = ab_glyph::FontArc::try_from_slice(include_bytes!(
            "Inconsolata-Regular.ttf"
        )).unwrap();

        let mut glyph_brush = GlyphBrushBuilder::using_font(inconsolata)
            .build(&device, surface_format);

        Self {
            window,
            surface,
            device,
            queue,
            staging_belt: wgpu::util::StagingBelt::new(1024),
            config,
            size,
            glyph_brush,
            tokio_runtime: runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .thread_name("ocr_worker")
                .build()
                .unwrap(),
            ocr_job: None,
            ocr_text: None,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        false
    }

    fn update(&mut self) {
        if let Some(running_job) = &self.ocr_job {
            running_job.abort();
            self.ocr_job = None;
            self.ocr_text = None;
        }
        let window_size = self.window.inner_size();
        let window_inner_position = self.window.inner_position().unwrap();
        self.ocr_job = Some(self.tokio_runtime.spawn(async move {
            let start_time = Instant::now();
            let screen = Screen::from_point(window_inner_position.x, window_inner_position.y).unwrap();
            let display_position = screen.display_info;
            let image = screen.capture_area(window_inner_position.x - display_position.x, window_inner_position.y - display_position.y, window_size.width, window_size.height).unwrap();
            let buffer = image.buffer();
            println!("Screenshot took {} ms", start_time.elapsed().as_millis());
            return Ok(ocr::execute_ocr(buffer));
        }));
    }

    fn check_running_job(&mut self) {
        if self.ocr_job.is_some() {
            let running_job = self.ocr_job.as_mut().unwrap();
            if running_job.is_finished() {
                let ocr_text = block_on(running_job).unwrap().unwrap();
                println!("{}", ocr_text);
                self.ocr_job = None;
                self.ocr_text = Some(nodes_to_words(&html_parser::Dom::parse(&ocr_text).unwrap().children));
                self.render().unwrap();
            }
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
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
        }

        if let Some(words) = &self.ocr_text {
            for word in words {
                self.glyph_brush.queue(Section {
                    screen_position: (word.x, word.y),
                    bounds: (word.width, word.height),
                    text: vec![Text::new(&word.text)
                        .with_color(if word.is_highlighted { [1.0, 1.0, 1.0, 1.0] } else { [0.0, 0.0, 0.0, 1.0] })
                        .with_scale(word.height)],
                    ..Section::default()
                });
            }

            self.glyph_brush.draw_queued(&self.device, &mut self.staging_belt, &mut encoder, &view, self.size.width, self.size.height).unwrap();
        }
    
        self.staging_belt.finish();
        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.staging_belt.recall();
    
        Ok(())
    }

    fn handle_cursor(&mut self, cursor_position: &PhysicalPosition<f64>) {
        if let Some(bbox_words) = &mut self.ocr_text {
            for bbox_word in bbox_words {
                if bbox_word.is_within_bounds(cursor_position) {
                    bbox_word.is_highlighted = true;
                } else {
                    bbox_word.is_highlighted = false;
                }
            }
            self.render().unwrap();
        }
    }

    fn handle_click(&self) {
        if let Some(bbox_words) = &self.ocr_text {
            for bbox_word in bbox_words {
                if bbox_word.is_highlighted {
                    println!("{}", bbox_word.text);
                }
            }
        }
    }
    
}

fn nodes_to_words(nodes: &Vec<Node>) -> Vec<BboxWord> {
    let mut words: Vec<BboxWord> = Vec::new();
    for node in nodes {
        if let html_parser::Node::Element(element) = node {
            if element.classes.contains(&"ocrx_word".to_string()) { // is individual word
                let title = element.attributes["title"].clone().unwrap();
                let mut parts = title.split(" ");
                parts.next();
                let x = parse_bbox_f32(parts.next().unwrap());
                let y = parse_bbox_f32(parts.next().unwrap());
                let width = parse_bbox_f32(parts.next().unwrap()) - x;
                let height = parse_bbox_f32(parts.next().unwrap()) - y;
                let text = get_text_child(&element.children);
                let word = BboxWord {
                    text,
                    x,
                    y,
                    width,
                    height,
                    is_highlighted: false
                };
                words.push(word);
            } else {
                words.append(&mut nodes_to_words(&node.element().unwrap().children));
            }
        }
    }
    return words;
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
    return parsed / 5.0; //OCR image was upscaled 5x before processing
}

pub async fn screen_entry() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().with_transparent(true).build(&event_loop).unwrap();

    let mut window_state = State::new(window).await;

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window_state.window().id() => if !window_state.input(event) {
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
                    } => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size) => {
                        window_state.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        window_state.resize(**new_inner_size);
                    }
                    WindowEvent::Moved(_) => {
                        window_state.window().request_redraw();
                    }
                    WindowEvent::CursorMoved { device_id: _, position, modifiers: _ } => {
                        window_state.handle_cursor(position);
                    }
                    WindowEvent::MouseInput { device_id: _, state, button: _, modifiers: _ } => {
                        if let ElementState::Released = state {
                            window_state.handle_click();
                        }
                    }
                    _ => {}
                }
            }
            Event::RedrawRequested(window_id) if window_id == window_state.window().id() => {
                window_state.update();
                match window_state.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => window_state.resize(window_state.size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                window_state.check_running_job();
                // state.window().request_redraw();
            }
            _ => {}
        }
    });
}

// https://sotrh.github.io/learn-wgpu/beginner/tutorial2-surface/#render