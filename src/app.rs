use crate::image::{Image, ImageFormat};
use crate::renderer::{Parameters, Renderer};
use crate::scene::Scene;
use pixels::{Pixels, SurfaceTexture};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use egui_winit::egui;

pub struct App {
    renderer: Renderer,
    scene: Option<Scene>,
    image: Image,
    render_thread: Option<std::thread::JoinHandle<()>>,
    frame_buffer: Arc<Mutex<Option<Vec<u8>>>>,
    is_rendering: Arc<Mutex<bool>>,
    obj_path: String,
    camera_yaw: f32,
    camera_pitch: f32,
    camera_roll: f32,
    camera_distance: f32,
    camera_target: crate::vector::Vec3f,
    default_camera_distance: f32,
    default_camera_target: crate::vector::Vec3f,
    camera_dirty: bool,
    egui_state: Option<egui_winit::State>,
    egui_ctx: egui::Context,
    egui_renderer: Option<egui_wgpu::Renderer>,
    auto_rotate: bool,
    rotation_speed: f32,
    last_ui_change: Option<Instant>,
    ui_pointer_down: bool,

    invert_y: bool,
    orbit_sensitivity: f32,
    zoom_sensitivity: f32,
    render_while_dragging: bool,

    mouse_last_pos: Option<(f64, f64)>,
    mouse_left_down: bool,
    mouse_right_down: bool,
}

impl App {
    pub fn new(
        width: usize,
        height: usize,
        samples: usize,
        max_bounces: usize,
        debug_mode: bool,
        obj_path: String,
    ) -> Self {
        let renderer = Renderer::new(Parameters {
            samples,
            max_ray_depth: max_bounces,
            debug_mode,
            camera_pos: crate::vector::Vec3f::new(72.0, 72.0, 180.0),
            camera_target: crate::vector::Vec3f::new(72.0, 72.0, 0.0),
            camera_up: crate::vector::Vec3f::new(0.0, 1.0, 0.0),
        });
        let image = Image::new(ImageFormat::PPM, width, height);

        let initial_buffer = vec![0u8; width * height * 4];

        let egui_ctx = egui::Context::default();

        Self {
            renderer,
            scene: None,
            image,
            render_thread: None,
            frame_buffer: Arc::new(Mutex::new(Some(initial_buffer))),
            is_rendering: Arc::new(Mutex::new(false)),
            obj_path,
            camera_yaw: 0.0,
            camera_pitch: 0.0,
            camera_roll: 0.0,
            camera_distance: 180.0,
            camera_target: crate::vector::Vec3f::new(72.0, 72.0, 0.0),
            default_camera_distance: 180.0,
            default_camera_target: crate::vector::Vec3f::new(72.0, 72.0, 0.0),
            camera_dirty: true,
            egui_state: None,
            egui_ctx,
            egui_renderer: None,
            auto_rotate: false,
            rotation_speed: 0.02,
            last_ui_change: None,
            ui_pointer_down: false,

            invert_y: false,
            orbit_sensitivity: 0.008,
            zoom_sensitivity: 0.10,
            render_while_dragging: false,

            mouse_last_pos: None,
            mouse_left_down: false,
            mouse_right_down: false,
        }
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("Epstein Island Renderer in Rust")
            .with_inner_size(LogicalSize::new(
                self.image.width as u32,
                self.image.height as u32,
            ))
            .with_resizable(false)
            .build(&event_loop)
            .unwrap();

        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let mut pixels = Pixels::new(
            self.image.width as u32,
            self.image.height as u32,
            surface_texture,
        )
        .unwrap();

        self.egui_state = Some(egui_winit::State::new(&event_loop));
        self.egui_renderer = Some(egui_wgpu::Renderer::new(
            pixels.device(),
            pixels.render_texture_format(),
            None,
            1,
        ));

        println!("Loading scene...");
        self.scene = Scene::load(&self.obj_path);
        if self.scene.is_some() {
            if let Some(scene) = &self.scene {
                if let Some(root) = scene.bvh.nodes.get(0) {
                    let center = (root.bounds_min + root.bounds_max) * 0.5;
                    let extent = root.bounds_max - root.bounds_min;
                    let diag = extent.length();

                    self.camera_target = center;
                    self.default_camera_target = center;

                    let dist = (diag * 1.4).clamp(10.0, 1000.0);
                    self.camera_distance = dist;
                    self.default_camera_distance = dist;

                    self.camera_yaw = 0.0;
                    self.camera_pitch = 0.0;
                    self.camera_roll = 0.0;
                    self.camera_dirty = true;
                }
            }

            println!("Scene loaded, starting render...");
            self.start_render();
        } else {
            println!("Failed to load scene!");
        }

        let mut last_update = Instant::now();

        event_loop.run(move |event, _, control_flow| {
            let mut egui_consumed = false;
            
            match &event {
                Event::WindowEvent { event: win_event, .. } => {
                    if let Some(ref mut state) = self.egui_state {
                        let response = state.on_event(&self.egui_ctx, win_event);
                        egui_consumed = response.consumed;
                        if response.consumed || response.repaint {
                            window.request_redraw();
                        }
                    }
                }
                _ => {}
            }

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent {
                    event: WindowEvent::KeyboardInput { input, .. },
                    ..
                } => {
                    if let Some(keycode) = input.virtual_keycode {
                        if input.state == winit::event::ElementState::Pressed {
                            match keycode {
                                winit::event::VirtualKeyCode::Escape => {
                                    *control_flow = ControlFlow::Exit;
                                }
                                winit::event::VirtualKeyCode::Q => {
                                    if !*self.is_rendering.lock().unwrap() {
                                        self.camera_distance -= 10.0;
                                        self.camera_distance = self.camera_distance.clamp(10.0, 1000.0);
                                        self.camera_dirty = true;
                                        self.last_ui_change = Some(Instant::now());
                                    }
                                }
                                winit::event::VirtualKeyCode::E => {
                                    if !*self.is_rendering.lock().unwrap() {
                                        self.camera_distance += 10.0;
                                        self.camera_distance = self.camera_distance.clamp(10.0, 1000.0);
                                        self.camera_dirty = true;
                                        self.last_ui_change = Some(Instant::now());
                                    }
                                }
                                winit::event::VirtualKeyCode::Space => {
                                    self.auto_rotate = !self.auto_rotate;
                                    println!("Auto-rotate: {}", self.auto_rotate);
                                    window.request_redraw();
                                }
                                winit::event::VirtualKeyCode::R => {
                                    if !*self.is_rendering.lock().unwrap() {
                                        self.camera_dirty = true;
                                    }
                                }
                                winit::event::VirtualKeyCode::P => {
                                    if let Some(buffer) = self.frame_buffer.lock().unwrap().as_ref()
                                    {
                                        let mut rgb_buffer =
                                            vec![0u8; self.image.width * self.image.height * 3];
                                        for i in 0..self.image.width * self.image.height {
                                            rgb_buffer[i * 3] = buffer[i * 4];
                                            rgb_buffer[i * 3 + 1] = buffer[i * 4 + 1];
                                            rgb_buffer[i * 3 + 2] = buffer[i * 4 + 2];
                                        }
                                        let mut image = Image::new(
                                            ImageFormat::PPM,
                                            self.image.width,
                                            self.image.height,
                                        );
                                        image.bytes = rgb_buffer;
                                        image.write_to_path("output.ppm");
                                        println!("Image saved to output.ppm");
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::MouseInput { state, button, .. },
                    ..
                } => {
                    if egui_consumed {
                        return;
                    }

                    match (button, state) {
                        (MouseButton::Left, ElementState::Pressed) => {
                            self.mouse_left_down = true;
                            self.mouse_last_pos = None;
                        }
                        (MouseButton::Left, ElementState::Released) => {
                            self.mouse_left_down = false;
                            self.mouse_last_pos = None;
                            window.request_redraw();
                        }
                        (MouseButton::Right, ElementState::Pressed) => {
                            self.mouse_right_down = true;
                            self.mouse_last_pos = None;
                        }
                        (MouseButton::Right, ElementState::Released) => {
                            self.mouse_right_down = false;
                            self.mouse_last_pos = None;
                            window.request_redraw();
                        }
                        _ => {}
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::CursorMoved { position, .. },
                    ..
                } => {
                    if egui_consumed {
                        return;
                    }

                    let (x, y) = (position.x, position.y);
                    let _ = self.mouse_last_pos;
                    self.mouse_last_pos = Some((x, y));
                }
                Event::WindowEvent {
                    event: WindowEvent::MouseWheel { delta, .. },
                    ..
                } => {
                    if egui_consumed {
                        return;
                    }

                    let scroll_y = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y as f32,
                        MouseScrollDelta::PixelDelta(p) => (p.y as f32) / 100.0,
                    };

                    let zoom_factor = 1.0 - (scroll_y * self.zoom_sensitivity);
                    self.camera_distance = (self.camera_distance * zoom_factor).clamp(10.0, 1000.0);
                    self.camera_dirty = true;
                    self.last_ui_change = Some(Instant::now());
                    window.request_redraw();
                }
                Event::RedrawRequested(_) => {
                    let now = Instant::now();
                    let delta = now.duration_since(last_update).as_secs_f32();
                    last_update = now;

                    if self.auto_rotate && !*self.is_rendering.lock().unwrap() {
                        self.camera_yaw += self.rotation_speed * delta * 60.0;
                        self.camera_dirty = true;
                        self.last_ui_change = Some(Instant::now());
                    }

                    if self.camera_dirty && !*self.is_rendering.lock().unwrap() {

                        let mut started_render = false;

                        if !(self.ui_pointer_down && !self.render_while_dragging) {
                            let should_render = match self.last_ui_change {
                                Some(t) => t.elapsed().as_millis() >= 150,
                                None => true,
                            };

                            if should_render {
                                self.camera_dirty = false;
                                self.last_ui_change = None;
                                self.start_render();
                                started_render = true;
                            }
                        }

                        if !started_render {
                            window.request_redraw();
                        }
                    }

                    let mut egui_paint: Option<(Vec<egui::epaint::ClippedPrimitive>, egui::TexturesDelta)> = None;

                    if let Some(ref mut state) = self.egui_state {
                        let raw_input = state.take_egui_input(&window);

                        self.ui_pointer_down = self.mouse_left_down || self.mouse_right_down;
                        
                        let was_rendering = *self.is_rendering.lock().unwrap();
                        let mut camera_yaw = self.camera_yaw;
                        let mut camera_pitch = self.camera_pitch;
                        let mut camera_roll = self.camera_roll;
                        let mut camera_distance = self.camera_distance;
                        let mut rotation_speed = self.rotation_speed;
                        let mut auto_rotate = self.auto_rotate;
                        let mut invert_y = self.invert_y;
                        let mut orbit_sensitivity = self.orbit_sensitivity;
                        let mut zoom_sensitivity = self.zoom_sensitivity;
                        let mut render_while_dragging = self.render_while_dragging;
                        let mut camera_changed = false;
                        let mut settings_changed = false;
                        
                        let output = self.egui_ctx.run(raw_input, |ctx| {
                            egui::Window::new("Camera Controls")
                                .default_pos(egui::pos2(10.0, 10.0))
                                .resizable(false)
                                .show(ctx, |ui| {
                                    ui.heading("Camera Controls");
                                    ui.separator();

                                    ui.label("Rotation (degrees):");
                                    let mut rot_x = camera_pitch.to_degrees();
                                    let mut rot_y = camera_yaw.to_degrees();
                                    let mut rot_z = camera_roll.to_degrees();

                                    if ui.add(egui::Slider::new(&mut rot_x, -89.0..=89.0).text("X"))
                                        .changed() {
                                        camera_pitch = rot_x.to_radians();
                                        camera_changed = true;
                                    }
                                    if ui.add(egui::Slider::new(&mut rot_y, -180.0..=180.0).text("Y"))
                                        .changed() {
                                        camera_yaw = rot_y.to_radians();
                                        camera_changed = true;
                                    }
                                    if ui.add(egui::Slider::new(&mut rot_z, -180.0..=180.0).text("Z"))
                                        .changed() {
                                        camera_roll = rot_z.to_radians();
                                        camera_changed = true;
                                    }

                                    ui.separator();
                                    ui.label("Zoom:");
                                    ui.horizontal(|ui| {
                                        if ui.button("Zoom In").clicked() {
                                            camera_distance = (camera_distance * 0.9).clamp(10.0, 1000.0);
                                            camera_changed = true;
                                        }
                                        if ui.button("Zoom Out").clicked() {
                                            camera_distance = (camera_distance * 1.1).clamp(10.0, 1000.0);
                                            camera_changed = true;
                                        }
                                    });
                                    let min_dist = (self.default_camera_distance * 0.25).clamp(10.0, 1000.0);
                                    let max_dist = (self.default_camera_distance * 3.0).clamp(10.0, 1000.0);
                                    if ui
                                        .add(egui::Slider::new(&mut camera_distance, min_dist..=max_dist).text("Distance"))
                                        .changed()
                                    {
                                        camera_changed = true;
                                    }

                                    if ui.button("Reset").clicked() {
                                        camera_pitch = 0.0;
                                        camera_yaw = 0.0;
                                        camera_roll = 0.0;
                                        camera_distance = self.default_camera_distance;
                                        camera_changed = true;
                                    }
                                    
                                    ui.separator();
                                    
                                    if was_rendering {
                                        ui.label("Rendering...");
                                    } else {
                                        ui.label("Ready");
                                    }
                                });
                        });
                        
                        if camera_changed {
                            self.camera_yaw = camera_yaw;
                            self.camera_pitch = camera_pitch.clamp(-1.55, 1.55);
                            self.camera_roll = camera_roll;
                            self.camera_distance = camera_distance;
                            self.camera_distance = self.camera_distance.clamp(10.0, 1000.0);
                            self.camera_dirty = true;
                            self.last_ui_change = Some(Instant::now());
                        }
                        if settings_changed {
                            self.rotation_speed = rotation_speed;
                            self.auto_rotate = auto_rotate;
                            self.invert_y = invert_y;
                            self.orbit_sensitivity = orbit_sensitivity;
                            self.zoom_sensitivity = zoom_sensitivity;
                            self.render_while_dragging = render_while_dragging;
                        }

                        state.handle_platform_output(&window, &self.egui_ctx, output.platform_output);

                        let clipped_primitives = self.egui_ctx.tessellate(output.shapes);
                        egui_paint = Some((clipped_primitives, output.textures_delta));

                        if output.repaint_after.is_zero() {
                            window.request_redraw();
                        }
                    }

                    if self.update(&mut pixels) {
                        window.request_redraw();
                    }

                    let window_size = window.inner_size();
                    
                    pixels.render_with(|encoder, render_target, context| {
                        context.scaling_renderer.render(encoder, render_target);
                        
                        if let (Some(egui_renderer), Some((clipped_primitives, textures_delta))) = (&mut self.egui_renderer, egui_paint.as_ref()) {
                            let screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
                                size_in_pixels: [window_size.width, window_size.height],
                                pixels_per_point: window.scale_factor() as f32,
                            };

                            for (id, image_delta) in &textures_delta.set {
                                egui_renderer.update_texture(pixels.device(), pixels.queue(), *id, image_delta);
                            }
                            
                            egui_renderer.update_buffers(
                                pixels.device(),
                                pixels.queue(),
                                encoder,
                                &clipped_primitives,
                                &screen_descriptor
                            );
                            
                            {
                                let mut rpass = encoder.begin_render_pass(&pixels::wgpu::RenderPassDescriptor {
                                    label: Some("egui render pass"),
                                    color_attachments: &[Some(pixels::wgpu::RenderPassColorAttachment {
                                        view: render_target,
                                        resolve_target: None,
                                        ops: pixels::wgpu::Operations {
                                            load: pixels::wgpu::LoadOp::Load,
                                            store: true,
                                        },
                                    })],
                                    depth_stencil_attachment: None,
                                });
                                
                                egui_renderer.render(&mut rpass, &clipped_primitives, &screen_descriptor);
                            }

                            for id in &textures_delta.free {
                                egui_renderer.free_texture(id);
                            }
                        }
                        
                        Ok(())
                    }).unwrap();
                }
                Event::MainEventsCleared => {
                    if self.auto_rotate || *self.is_rendering.lock().unwrap() {
                        window.request_redraw();
                        std::thread::sleep(std::time::Duration::from_millis(16));
                    } else if self.camera_dirty {
                        window.request_redraw();
                    } else {
                        *control_flow = ControlFlow::Wait;
                    }
                }
                _ => {}
            }
        });
    }

    fn start_render(&mut self) {
        let mut is_rendering = self.is_rendering.lock().unwrap();
        if *is_rendering {
            return;
        }
        *is_rendering = true;
        drop(is_rendering);

        let scene = match &self.scene {
            Some(s) => s.clone(),
            None => return,
        };

        // Yaw=0 faces +Z (front), Pitch=0 level.
        let dir = crate::vector::Vec3f::new(
            self.camera_yaw.sin() * self.camera_pitch.cos(),
            self.camera_pitch.sin(),
            self.camera_yaw.cos() * self.camera_pitch.cos(),
        );
        let camera_pos = self.camera_target + (dir * self.camera_distance);
        let camera_target = self.camera_target;

        // Build an up vector with roll applied around the forward axis.
        let forward = (camera_target - camera_pos).normalized();
        let world_up = crate::vector::Vec3f::new(0.0, 1.0, 0.0);
        let right = crate::vector::Vec3f::cross(forward, world_up).normalized();
        let up_no_roll = crate::vector::Vec3f::cross(right, forward).normalized();
        let camera_up = (up_no_roll * self.camera_roll.cos()) + (right * self.camera_roll.sin());

        let mut renderer = self.renderer.clone();
        renderer.parameters.camera_pos = camera_pos;
        renderer.parameters.camera_target = camera_target;
        renderer.parameters.camera_up = camera_up;
        
        let width = self.image.width;
        let height = self.image.height;
        let frame_buffer = self.frame_buffer.clone();
        let is_rendering = self.is_rendering.clone();

        self.render_thread = Some(std::thread::spawn(move || {
            let start = Instant::now();
            let mut image = Image::new(ImageFormat::PPM, width, height);
            renderer.render_to_image(&scene, &mut image);

            let mut rgba_buffer = vec![0u8; width * height * 4];
            for i in 0..width * height {
                rgba_buffer[i * 4] = image.bytes[i * 3];
                rgba_buffer[i * 4 + 1] = image.bytes[i * 3 + 1];
                rgba_buffer[i * 4 + 2] = image.bytes[i * 3 + 2];
                rgba_buffer[i * 4 + 3] = 255;
            }

            *frame_buffer.lock().unwrap() = Some(rgba_buffer);
            *is_rendering.lock().unwrap() = false;

            println!("Rendering completed in {} ms", start.elapsed().as_millis());
        }));
    }

    fn update(&mut self, pixels: &mut Pixels) -> bool {
        if let Some(buffer) = self.frame_buffer.lock().unwrap().as_ref() {
            let frame = pixels.frame_mut();
            frame.copy_from_slice(&buffer);
            return *self.is_rendering.lock().unwrap();
        }
        false
    }
}

