#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ffi;
mod chat;
mod overlay_grpc;

use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing_subscriber::EnvFilter;
use windows::Win32::Graphics::Direct3D11::*;
use windows::core::Interface;
use windows::Win32::Foundation::HMODULE;
// Fixed import


use chat::{ChatState, ChatEvent, ChatCommand};
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

        Ok((
            Self {
                chat_state,
                event_rx,
                command_tx,
                is_dashboard,
                gpu_context,
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
        let mut last_frame = Instant::now();
        let frame_duration = Duration::from_millis(16); // ~60 FPS

        loop {
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

            // Frame timing
            let elapsed = last_frame.elapsed();
            if elapsed < frame_duration {
                std::thread::sleep(frame_duration - elapsed);
            }
            last_frame = Instant::now();
        }
    }

    fn render_frame(&mut self) -> Result<()> {
        // Sync with compositor
        ffi::compositor_sync();

        // Render ImGui and submit to OpenVR
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