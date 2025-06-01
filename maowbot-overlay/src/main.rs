#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod ffi;
mod keyboard;
mod imgui_renderer;

use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing_subscriber::EnvFilter;
#[cfg(windows)]
use windows::core::Interface;
use keyboard::VirtualKeyboard;
use maowbot_common_ui::{AppEvent, AppState, ChatEvent, SharedGrpcClient};
use imgui_renderer::ImGuiOverlayRenderer;
use maowbot_common_ui::events::ChatCommand;
use maowbot_common_ui::settings::{StreamOverlaySettings, UISettings, AudioSettings};

struct OverlayApp {
    state: AppState,
    event_rx: Receiver<AppEvent>,
    command_tx: Sender<ChatCommand>,
    is_dashboard: bool,
    gpu_context: GpuContext,
    keyboard: Option<VirtualKeyboard>,
    show_keyboard: bool,
    hip_tracker_index: Option<u32>,
    renderer: ImGuiOverlayRenderer,
    // Settings
    overlay_settings: StreamOverlaySettings,
    ui_settings: UISettings,
    audio_settings: AudioSettings,
    show_settings: bool,
}

#[cfg(windows)]
struct GpuContext {
    device: windows::Win32::Graphics::Direct3D11::ID3D11Device,
    context: windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    width: u32,
    height: u32,
}

#[cfg(not(windows))]
struct GpuContext {
    width: u32,
    height: u32,
}

impl OverlayApp {
    fn new() -> Result<(Self, Sender<AppEvent>)> {
        // Initialize OpenVR
        ffi::init()?;
        
        // Create both overlays
        unsafe {
            if !ffi::vr_create_overlays() {
                return Err(anyhow::anyhow!("Failed to create overlays"));
            }
        }
        
        // Position HUD overlay in front of user
        unsafe { ffi::vr_center_in_front(1.5) };

        // Create GPU context
        let gpu_context = Self::create_gpu_context()?;

        // Initialize ImGui
        #[cfg(windows)]
        unsafe {
            ffi::imgui_init(
                gpu_context.device.as_raw() as *mut _,
                gpu_context.context.as_raw() as *mut _,
            );
        }
        
        #[cfg(not(windows))]
        unsafe {
            ffi::imgui_init(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
        }

        // Create channels
        let (event_tx, event_rx) = bounded(100);
        let (command_tx, command_rx) = bounded(100);

        // Create shared state
        let state = AppState::new();

        // Start gRPC client
        SharedGrpcClient::start(
            "maowbot-overlay".to_string(),
            event_tx.clone(),
            command_rx,
        );

        // Create virtual keyboard for HUD mode
        let keyboard = match VirtualKeyboard::new() {
                Ok(mut kb) => {
                    #[cfg(windows)]
                    let init_result = kb.init_rendering(
                        gpu_context.device.as_raw() as *mut _,
                        gpu_context.context.as_raw() as *mut _,
                    );
                    
                    #[cfg(not(windows))]
                    let init_result = kb.init_rendering(
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    );
                    
                    if let Err(e) = init_result {
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
        };

        Ok((
            Self {
                state,
                event_rx,
                command_tx,
                is_dashboard: false,  // No longer needed - we have both overlays
                gpu_context,
                keyboard,
                show_keyboard: false,
                hip_tracker_index: None,
                renderer: ImGuiOverlayRenderer::new(false),  // HUD renderer
                overlay_settings: StreamOverlaySettings::default(),
                ui_settings: UISettings::default(),
                audio_settings: AudioSettings::default(),
                show_settings: false,
            },
            event_tx,
        ))
    }

    #[cfg(windows)]
    fn create_gpu_context() -> Result<GpuContext> {
        use windows::Win32::Graphics::Direct3D::*;
        use windows::Win32::Graphics::Direct3D11::*;
        use windows::core::Interface;
        use windows::Win32::Foundation::HMODULE;

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
    
    #[cfg(not(windows))]
    fn create_gpu_context() -> Result<GpuContext> {
        // On Linux, OpenGL context is managed by OpenVR/ImGui
        Ok(GpuContext {
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
                tracing::trace!("FPS: {}", frame_count);
                frame_count = 0;
                last_fps_print = Instant::now();
            }

            // Process events
            while let Ok(event) = self.event_rx.try_recv() {
                match event {
                    AppEvent::Chat(chat_event) => {
                        let mut state = self.state.chat_state.lock().unwrap();
                        state.add_message(chat_event);
                    }
                    AppEvent::Shutdown => return Ok(()),
                    _ => {}
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
            self.renderer.update_state(&self.state);
            
            // Always update overlay settings for dashboard
            self.renderer.update_dashboard_state(true, &self.overlay_settings);

            // Process controller input
            self.process_controller_input()?;

            // Check if input field was just focused
            let input_focused = unsafe { ffi::imgui_get_input_focused() };
            if input_focused && !self.show_keyboard {
                self.show_keyboard = true;
                if let Some(ref mut keyboard) = self.keyboard {
                    keyboard.set_visible(true);
                    tracing::info!("Showing keyboard due to input focus");
                }
            }

            // Update keyboard if visible
            if self.show_keyboard {
                if let Some(ref mut keyboard) = self.keyboard {
                    // Position keyboard based on hip tracker availability
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
            if let Some(message) = self.renderer.get_sent_message() {
                let _ = self.command_tx.send(ChatCommand::SendMessage(message));
            }

            // No manual sleep - let VR compositor handle timing
        }
    }

    fn process_controller_input(&mut self) -> Result<()> {
        ffi::update_controllers();

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
                // Convert UV to pixel coordinates
                let x = hit.u * self.gpu_context.width as f32;
                let y = (1.0 - hit.v) * self.gpu_context.height as f32;

                // Update laser state for rendering
                unsafe {
                    ffi::imgui_update_laser_state(controller_idx, true, x, y);
                }

                // Use the most recent hit position as mouse position
                current_mouse_x = x;
                current_mouse_y = y;

                // Check if trigger is currently pressed
                let trigger_value = unsafe { ffi::vr_get_controller_trigger_value(controller_idx) };
                if trigger_value > 0.5 {
                    trigger_down = true;
                }

                // Handle trigger press event for haptics
                if unsafe { ffi::vr_get_controller_trigger_pressed(controller_idx) } {
                    unsafe { ffi::vr_trigger_haptic_pulse(controller_idx, 1000) };
                }

                // Handle menu button for keyboard toggle
                if unsafe { ffi::vr_get_controller_menu_pressed(controller_idx) } {
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

        // Render HUD overlay (chat)
        let hud_ok = unsafe {
            ffi::imgui_render_hud(
                self.gpu_context.width,
                self.gpu_context.height,
            )
        };

        if !hud_ok {
            tracing::error!("Failed to submit HUD frame to OpenVR");
        }
        
        // Render Dashboard overlay (settings)
        let dashboard_ok = unsafe {
            ffi::imgui_render_dashboard(
                1280,  // Dashboard is larger
                960,
            )
        };

        if !dashboard_ok {
            tracing::error!("Failed to submit Dashboard frame to OpenVR");
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

    let (mut app, _event_tx) = OverlayApp::new()?;

    tracing::info!("✔ Overlay started with HUD and Dashboard");

    app.run()
}