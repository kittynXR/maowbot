#![cfg_attr(not(debug_assertions),windows_subsystem="windows")]

mod ffi;
mod vr_layer;
mod chat_hud;
mod overlay_grpc;

use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy::winit::{UpdateMode, WinitSettings};
use tracing_subscriber::EnvFilter;
use vr_layer::VrOverlayPlugin;
use overlay_grpc::OverlayGrpcPlugin;
use chat_hud::ChatHudPlugin;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("maowbot_overlay=info".parse().unwrap()))
        .init();

    let args:Vec<String>=std::env::args().collect();
    let dashboard=args.iter().any(|a|a=="--dashboard");

    let mut app=App::new();
    app.add_plugins(
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1.0, 1.0).into(),
                transparent: true,
                decorations: false,
                visible: true,          // ‹— **must stay true**
                position: WindowPosition::At(IVec2::new(-10_000, -10_000)), // park off-screen
                ..default()
            }),
            exit_condition: ExitCondition::DontExit,
            ..default()
        }),
    )
    .add_plugins((
        VrOverlayPlugin { dashboard },   // ← runs overlay setup / submit_frame
        ChatHudPlugin,                   // ← egui chat UI
        OverlayGrpcPlugin,               // ← gRPC bridge
    ));

    app.insert_resource(WinitSettings {
        focused_mode:   UpdateMode::Continuous,
        unfocused_mode: UpdateMode::Continuous,
    });
    
    app.run();
}
