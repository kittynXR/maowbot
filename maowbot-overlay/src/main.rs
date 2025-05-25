#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ffi;
mod chat;
mod overlay_grpc;
mod keyboard;

use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing_subscriber::EnvFilter;
use windows::Win32::Graphics::Direct3D11::*;
use windows::core::Interface;
use windows::Win32::Foundation::HMODULE;

use chat::{ChatState, ChatEvent, ChatCommand};
use keyboard::VirtualKeyboard;
use overlay_grpc::start_grpc_client;

pub enum AppEvent {
    Chat(ChatEvent),
    Shutdown,
}

struct OverlayApp {
    chat_state: Arc<Mutex<ChatState>>,
    event_rx: Receiver<AppEvent>,
    command_tx: Sender<ChatCommand>,
    is_dashboard: bool,
    gpu_context: GpuContext,
    keyboard: Option<VirtualKeyboard>,
    show_keyboard: bool,
    hip_tracker_index: Option<u32>,
}

struct GpuContext {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    width: u32,
    height: u32,
}

impl OverlayApp {
    fn new(is_dashboard: bool) -> Result<(Self, Sender<AppEvent>)> {
        // Initialize OpenVR
        let (key, name) = if is_dashboard {
            ("maowbot.overlay.dashboard", "maowbot Dashboard")
        } else {
            ("maowbot.overlay.hud", "maowbot HUD")
        };

        ffi::init()?;
        ffi::create_overlay(key, name, 1.0, is_dashboard)?;
        ffi::set_overlay_width(1.0);

        if !is_dashboard {
            unsafe { ffi::vr_center_in_front(1.5) };
        } else {
            ffi::show_dashboard(key);
        }

        // Create D3D11 device
        let gpu_context = Self::create_d3d11_device()?;

        // Initialize ImGui
        unsafe {
            ffi::imgui_init(
                gpu_context.device.as_raw() as *mut _,
                gpu_context.context.as_raw() as *mut _,
            );
        }

        // Create channels
        let (event_tx, event_rx) = bounded(100);
        let (command_tx, command_rx) = bounded(100);

        // Create shared state
        let chat_state = Arc::new(Mutex::new(ChatState::new()));

        // Start gRPC client
        let chat_state_clone = chat_state.clone();
        start_grpc_client(event_tx.clone(), command_rx, chat_state_clone);

        // Create virtual keyboard (optional, only for HUD mode)
        let keyboard = if !is_dashboard {
            match VirtualKeyboard::new() {
                Ok(mut kb) => {
                    if let Err(e) = kb.init_rendering(
                        gpu_context.device.as_raw() as *mut _,
                        gpu_context.context.as_raw() as *mut _,
                    ) {
                        tracing::warn!("Failed to init keyboard rendering: {}", e);
                        None
                    } else {
                        Some(kb)
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to create virtual keyboard: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok((
            Self {
                chat_state,
                event_rx,
                command_tx,
                is_dashboard,
                gpu_context,
                keyboard,
                show_keyboard: false,
                hip_tracker_index: None,
            },
            event_tx,
        ))
    }

    fn create_d3d11_device() -> Result<GpuContext> {
        use windows::Win32::Graphics::Direct3D::*;

        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        let mut feature_level = D3D_FEATURE_LEVEL_11_0;

        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut feature_level),
                Some(&mut context),
            )?;
        }

        Ok(GpuContext {
            device: device.unwrap(),
            context: context.unwrap(),
            width: 1024,
            height: 768,
        })
    }

    fn run(&mut self) -> Result<()> {
        let mut frame_count = 0u64;
        let mut last_fps_print = Instant::now();

        // Check for hip tracker periodically
        let mut last_hip_check = Instant::now();

        loop {
            // Wait for optimal VR frame timing
            unsafe { ffi::vr_wait_get_poses() };

            frame_count += 1;

            // Print FPS every second
            if last_fps_print.elapsed() > Duration::from_secs(1) {
                tracing::info!("FPS: {}", frame_count);
                frame_count = 0;
                last_fps_print = Instant::now();
            }

            // Process events
            while let Ok(event) = self.event_rx.try_recv() {
                match event {
                    AppEvent::Chat(chat_event) => {
                        let mut state = self.chat_state.lock().unwrap();
                        state.add_message(chat_event);
                    }
                    AppEvent::Shutdown => return Ok(()),
                }
            }

            // Check for hip tracker every 5 seconds
            if last_hip_check.elapsed() > Duration::from_secs(5) {
                self.hip_tracker_index = ffi::find_hip_tracker();
                last_hip_check = Instant::now();

                if let Some(idx) = self.hip_tracker_index {
                    tracing::info!("Found hip tracker at index {}", idx);
                }
            }

            // Update ImGui state from Rust
            {
                let mut state = self.chat_state.lock().unwrap();
                unsafe {
                    ffi::imgui_update_chat_state(
                        state.get_messages_ptr(),
                        state.get_messages_count(),
                        state.get_input_buffer_ptr_mut(),
                        state.get_input_buffer_capacity(),
                    );
                }
            }

            // Process controller input
            self.process_controller_input()?;

            // Update keyboard if visible
            if self.show_keyboard {
                if let Some(ref mut keyboard) = self.keyboard {
                    keyboard.position_at_hip(self.hip_tracker_index);

                    // Process keyboard input and check if we got text
                    if let Some(text) = keyboard.process_input()? {
                        // Send the text through chat
                        let _ = self.command_tx.send(ChatCommand::SendMessage(text));

                        // Hide keyboard after sending
                        self.show_keyboard = false;
                        keyboard.set_visible(false);
                    }
                }
            }

            // Render frame
            self.render_frame()?;

            // Handle input from ImGui
            let mut input_buffer = [0u8; 256];
            let sent = unsafe { ffi::imgui_get_sent_message(input_buffer.as_mut_ptr(), 256) };

            if sent {
                if let Ok(text) = std::str::from_utf8(&input_buffer) {
                    if let Some(text) = text.trim_end_matches('\0').trim().to_string().into() {
                        if !text.is_empty() {
                            let _ = self.command_tx.send(ChatCommand::SendMessage(text));
                        }
                    }
                }
            }

            // No manual sleep - let VR compositor handle timing
        }
    }

    fn process_controller_input(&mut self) -> Result<()> {
        ffi::update_controllers();

        let mut hit_any = false;
        let mut current_mouse_x = -100.0;
        let mut current_mouse_y = -100.0;
        let mut trigger_down = false;

        for controller_idx in 0..2 {
            if !unsafe { ffi::vr_get_controller_connected(controller_idx) } {
                // Clear laser state for disconnected controller
                unsafe { ffi::imgui_update_laser_state(controller_idx, false, 0.0, 0.0) };
                continue;
            }

            // Test laser intersection with main overlay
            let hit = unsafe { ffi::vr_test_laser_intersection_main(controller_idx) };

            if hit.hit {
                hit_any = true;

                // Convert UV to pixel coordinates
                // NOTE: OpenVR UV coordinates have Y=0 at bottom, but screen coordinates have Y=0 at top
                let x = hit.u * self.gpu_context.width as f32;
                let y = (1.0 - hit.v) * self.gpu_context.height as f32;  // Invert Y coordinate

                // Update laser state for rendering
                unsafe {
                    ffi::imgui_update_laser_state(controller_idx, true, x, y);
                }

                // Use the most recent hit position as mouse position
                current_mouse_x = x;
                current_mouse_y = y;

                // Check if trigger is currently pressed (not just pressed this frame)
                let trigger_value = unsafe { ffi::vr_get_controller_trigger_value(controller_idx) };
                if trigger_value > 0.5 {
                    trigger_down = true;
                }

                // Handle trigger press event for haptics
                if unsafe { ffi::vr_get_controller_trigger_pressed(controller_idx) } {
                    unsafe { ffi::vr_trigger_haptic_pulse(controller_idx, 1000) };
                }

                // Handle menu button for keyboard toggle (HUD only)
                if !self.is_dashboard && unsafe { ffi::vr_get_controller_menu_pressed(controller_idx) } {
                    self.show_keyboard = !self.show_keyboard;
                    if let Some(ref mut keyboard) = self.keyboard {
                        keyboard.set_visible(self.show_keyboard);
                    }
                }
            } else {
                // Clear laser state when not hitting
                unsafe { ffi::imgui_update_laser_state(controller_idx, false, 0.0, 0.0) };
            }
        }

        // Always update mouse position and button state
        unsafe {
            ffi::imgui_inject_mouse_pos(current_mouse_x, current_mouse_y);
            ffi::imgui_inject_mouse_button(0, trigger_down);
        }

        Ok(())
    }

    fn render_frame(&mut self) -> Result<()> {
        // Render keyboard first if visible
        if self.show_keyboard {
            if let Some(ref mut keyboard) = self.keyboard {
                keyboard.render()?;
            }
        }

        // Then render main overlay
        let ok = unsafe {
            ffi::imgui_render_and_submit(
                self.gpu_context.width,
                self.gpu_context.height,
                self.is_dashboard,
            )
        };

        if !ok {
            tracing::error!("Failed to submit frame to OpenVR");
        }

        // Signal we're done with this frame
        ffi::compositor_sync();

        Ok(())
    }
}

impl Drop for OverlayApp {
    fn drop(&mut self) {
        unsafe {
            ffi::imgui_shutdown();
            ffi::vr_shutdown();
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("maowbot_overlay=info".parse().unwrap()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let is_dashboard = args.iter().any(|a| a == "--dashboard");

    let (mut app, _event_tx) = OverlayApp::new(is_dashboard)?;

    tracing::info!("✔ Overlay started in {} mode",
        if is_dashboard { "dashboard" } else { "HUD" });

    app.run()
}