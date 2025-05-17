#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod overlay_grpc;
mod chat_hud;

use bevy::prelude::*;
use overlay_grpc::OverlayGrpcPlugin;
use chat_hud::ChatHudPlugin;

fn main() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let mut app = App::new();

        // 1️⃣ Standard desktop HUD window
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "maowbot HUD".into(),
                        transparent: true,
                        decorations: false,
                        present_mode: bevy::window::PresentMode::AutoNoVsync,
                        ..default()
                    }),
                    ..default()
                })
        )
            .insert_resource(ClearColor(Color::NONE));

        // 2️⃣ Your core logic
        app.add_plugins((OverlayGrpcPlugin, ChatHudPlugin));

        app.run();
    });
}
