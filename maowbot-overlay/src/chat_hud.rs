// src/chat_hud.rs

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use crate::overlay_grpc::ChatEvent;

/// Plugin to display incoming chat and an input box.
pub struct ChatHudPlugin;

#[derive(Resource, Default)]
struct ChatLog {
    lines: Vec<String>,
}

#[derive(Resource, Default)]
struct ChatInput {
    buffer: String,
}

#[derive(Event)]
pub struct ChatSendEvent {
    pub text: String,
}

impl Plugin for ChatHudPlugin {
    fn build(&self, app: &mut App) {
        app
            // Add egui for our overlay UI
            .add_plugins(EguiPlugin { enable_multipass_for_primary_context: false })
            // Initialize resources
            .init_resource::<ChatLog>()
            .init_resource::<ChatInput>()
            // Register our outbound event
            .add_event::<ChatSendEvent>()
            // Add the three systems
            .add_systems(Update, (collect_chat, draw_hud, flush_send_chat));
    }
}

/// System 1: collect ChatEvent from gRPC into the ChatLog.
fn collect_chat(mut log: ResMut<ChatLog>, mut events: EventReader<ChatEvent>) {
    for ChatEvent { author, body, .. } in events.read() {
        log.lines.push(format!("<{}> {}", author, body));
    }
    // Keep only the last 200 lines
    const MAX: usize = 200;
    if log.lines.len() > MAX {
        let excess = log.lines.len() - MAX;
        log.lines.drain(0..excess);
    }
}

/// System 2: render the HUD (title bar, scrollable chat, input box).
fn draw_hud(
    mut egui_ctx: EguiContexts,
    log: Res<ChatLog>,
    mut input: ResMut<ChatInput>,
    mut sender: EventWriter<ChatSendEvent>,
) {
    use egui::{Align, Layout};

    egui::CentralPanel::default()
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(18, 18, 18, 230),
            ..Default::default()
        })
        .show(egui_ctx.ctx_mut(), |ui| {
            // ── Title Bar ───────────────────────────
            egui::TopBottomPanel::top("title_bar")
                .exact_height(28.0)
                .frame(egui::Frame::default().fill(egui::Color32::from_rgb(30, 30, 30)))
                .show_inside(ui, |bar| {
                    bar.with_layout(Layout::left_to_right(Align::Center), |bar| {
                        bar.label(
                            egui::RichText::new("maowbot HUD")
                                .heading()
                                .color(egui::Color32::WHITE),
                        );
                    });
                });

            // ── Chat Log ────────────────────────────
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |scroll| {
                    for line in &log.lines {
                        scroll.label(line);
                    }
                });

            // ── Input Box ──────────────────────────
            egui::TopBottomPanel::bottom("input_box")
                .exact_height(30.0)
                .frame(egui::Frame::default().fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 220)))
                .show_inside(ui, |bar| {
                    let response = bar.add(
                        egui::TextEdit::singleline(&mut input.buffer)
                            .hint_text("Type a message…")
                            .desired_width(f32::INFINITY),
                    );
                    // On Enter, emit ChatSendEvent
                    if response.lost_focus() && response.ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let text = input.buffer.trim();
                        if !text.is_empty() {
                            sender.write(ChatSendEvent { text: text.to_owned() });
                            input.buffer.clear();
                        }
                        response.request_focus();
                    }
                });
        });
}

/// System 3: drain our own ChatSendEvent so no unhandled events remain.
fn flush_send_chat(mut events: EventReader<ChatSendEvent>) {
    for _ in events.read() {
        // no-op; overlay_grpc will pick them up
    }
}
