// src/chat_hud.rs
use bevy::prelude::*;
use bevy::{
    input::{mouse::{MouseButton, MouseButtonInput}, ButtonState},
    window::WindowPosition,
};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use crate::overlay_grpc::ChatEvent;

/// Adds the chat overlay built with egui.
pub struct ChatHudPlugin;

#[derive(Resource, Default)]
struct WindowDrag {
    active: bool,
    start_cursor_logical: egui::Pos2,
    start_window_logical: Vec2,
}

#[derive(Resource, Default)]
struct ChatInput {
    buffer: String,
}

#[derive(Event)]
pub struct ChatSendEvent {
    pub text: String,
}

/// Simple ring‑buffer that stores the last N chat lines.
#[derive(Resource, Default)]
struct ChatLog(Vec<String>);

impl Plugin for ChatHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin { enable_multipass_for_primary_context: false })
            .init_resource::<ChatLog>()
            .init_resource::<ChatInput>()      // ← new
            .add_event::<ChatSendEvent>()      // ← new
            .add_systems(
                Update,
                (
                    collect_chat,
                    draw_hud,          // now includes input box
                    begin_window_drag,
                    flush_send_chat,   // push typed text into event stream
                ),
            );
    }
}

/* ------------------------------------------------------------------- */
/*   System 1 – collect Twitch/Discord lines into the ring‑buffer      */
/* ------------------------------------------------------------------- */
fn collect_chat(mut log: ResMut<ChatLog>, mut ev: EventReader<ChatEvent>) {
    for ChatEvent { author, body, .. } in ev.read() {
        log.0.push(format!("<{}> {}", author, body));
    }
    const MAX_LINES: usize = 200;
    if log.0.len() > MAX_LINES {
        let excess = log.0.len() - MAX_LINES;
        log.0.drain(0..excess);
    }
}

/* ------------------------------------------------------------------- */
/*   System 2 – render the log each frame with egui                    */
/* ------------------------------------------------------------------- */
fn draw_hud(
    mut ctxs: EguiContexts,
    log: Res<ChatLog>,
    mut input: ResMut<ChatInput>,
    mut sender: EventWriter<ChatSendEvent>,
) {
    use bevy_egui::egui::{self, Align, Layout};

    egui::CentralPanel::default()
        .frame(egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(18, 18, 18, 230),
            ..Default::default()
        })
        .show(ctxs.ctx_mut(), |ui| {
            /* ---- title bar ---- */
            egui::TopBottomPanel::top("hud_title_bar")
                .exact_height(28.0)
                .frame(egui::Frame::default().fill(egui::Color32::from_rgb(30, 30, 30)))
                .show_inside(ui, |top| {
                    top.with_layout(Layout::left_to_right(Align::Center), |bar| {
                        bar.label(egui::RichText::new("maowbot HUD").heading().color(egui::Color32::WHITE));
                    });
                });

            /* ---- main chat area ---- */
            egui::TopBottomPanel::bottom("chat_input_panel")
                .exact_height(30.0)
                .frame(egui::Frame::default().fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 220)))
                .show_inside(ui, |bot| {
                    let response = bot.add(
                        egui::TextEdit::singleline(&mut input.buffer)
                            .hint_text("Type a message…")
                            .desired_width(f32::INFINITY),
                    );
                    // Send on Enter
                    if response.lost_focus() && response.ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if !input.buffer.trim().is_empty() {
                            sender.send(ChatSendEvent { text: input.buffer.trim().to_owned() });
                            input.buffer.clear();
                        }
                        response.request_focus(); // keep focus
                    }
                });

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |scroll| {
                    for line in log.0.iter() {
                        scroll.label(line);
                    }
                });
        });
}



/// Draws a translucent rectangle + “HUD running” text each frame
fn debug_banner(mut ctxs: EguiContexts) {
    use bevy_egui::egui::{self, Align2};

    egui::Area::new("debug_banner".into())      // ← fixed
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctxs.ctx_mut(), |ui| {
            ui.visuals_mut().widgets.noninteractive.bg_fill =
                egui::Color32::from_rgba_unmultiplied(20, 20, 20, 200);
            ui.add_space(8.0);
            ui.label(egui::RichText::new("maowbot HUD running")
                .heading()
                .color(egui::Color32::WHITE));
            ui.add_space(8.0);
        });
}

/* ---------- drag_start ---------------------------------------- */
fn drag_start(
    mut drag: ResMut<WindowDrag>,
    windows: Query<&Window>,
    mut ev: EventReader<MouseButtonInput>,
    mut ctxs: EguiContexts,
) {
    let window = windows.single().unwrap();          // primary window

    for e in ev.read() {
        if e.button == MouseButton::Left && e.state == ButtonState::Pressed {
            if let Some(pos) = ctxs.ctx_mut().input(|i| i.pointer.latest_pos()) {
                if pos.y <= 28.0 {
                    drag.active = true;
                    drag.start_cursor_logical = pos;   // logical

                    // current window logical position
                    let logical = match window.position {
                        WindowPosition::At(v) =>
                            Vec2::new(v.x as f32, v.y as f32) / window.resolution.scale_factor(),
                        _ => Vec2::ZERO,
                    };
                    drag.start_window_logical = logical;
                }
            }
        }
    }
}

/* ---------- drag_update --------------------------------------- */
fn drag_update(
    mut drag: ResMut<WindowDrag>,
    mut windows: Query<&mut Window>,
    mut ctxs: EguiContexts,
) {
    if !drag.active { return; }

    if let Some(pos) = ctxs.ctx_mut().input(|i| i.pointer.latest_pos()) {
        let delta_e = pos - drag.start_cursor_logical;              // egui::Vec2
        let delta_b = Vec2::new(delta_e.x, delta_e.y);              // Bevy Vec2
        let mut win = windows.single_mut().unwrap();
        let scale   = win.resolution.scale_factor();
        let new_logical = drag.start_window_logical + delta_b;
        let new_px = IVec2::new(
            (new_logical.x * scale).round() as i32,
            (new_logical.y * scale).round() as i32,
        );
        if win.position != WindowPosition::At(new_px) {
            win.position = WindowPosition::At(new_px);
        }
    }
}


/* ---------- drag_end ------------------------------------------ */
fn drag_end(
    mut drag: ResMut<WindowDrag>,
    mut ev: EventReader<MouseButtonInput>,
) {
    for e in ev.read() {
        if e.button == MouseButton::Left && matches!(e.state, ButtonState::Released) {
            drag.active = false;
        }
    }
}

/// Start OS‑level drag when the user presses LMB in the title bar (≤ 28 px)
fn begin_window_drag(
    mut windows: Query<&mut Window>,
    mut ev: EventReader<MouseButtonInput>,
    mut ctxs: EguiContexts,
) {
    let mut window = windows.single_mut().unwrap();            // primary window

    for e in ev.read() {
        if e.button == MouseButton::Left && e.state == ButtonState::Pressed {
            if let Some(pos) = ctxs.ctx_mut().input(|i| i.pointer.latest_pos()) {
                if pos.y <= 28.0 {
                    window.start_drag_move();                  // <‑‑ one call
                }
            }
        }
    }
}

fn flush_send_chat(mut ev: EventReader<ChatSendEvent>) {
    for _ in ev.read() {
        // overlay_grpc will consume later; for now just drain
    }
}