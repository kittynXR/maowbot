use libc::{c_char, c_void};
use std::ffi::CString;

#[repr(C)]
pub struct VREvent {
    pub(crate) _data: [u8; 64],
}

#[repr(C)]
pub struct LaserHit {
    pub hit: bool,
    pub u: f32,
    pub v: f32,
    pub distance: f32,
}

#[repr(C)]
pub struct HmdMatrix34 {
    pub m: [[f32; 4]; 3],
}

#[repr(C)]
pub struct OverlaySettingsFFI {
    pub show_chat: bool,
    pub chat_opacity: f32,
    pub chat_position_x: f32,
    pub chat_position_y: f32,
    pub chat_width: f32,
    pub chat_height: f32,
    pub show_alerts: bool,
    pub alert_duration: f32,
}

#[repr(C)]
pub struct DashboardState {
    pub show_settings: bool,
    pub current_tab: i32,
}

pub type VROverlayHandle = u64;

extern "C" {
    // OpenVR functions
    pub fn vr_init_overlay() -> bool;
    pub fn vr_shutdown();
    pub fn vr_create_overlays() -> bool;
    pub fn vr_create_overlay(
        key: *const c_char,
        name: *const c_char,
        width_m: f32,
        dashboard: bool,
    ) -> bool;
    pub fn vr_create_overlay_raw(
        key: *const c_char,
        name: *const c_char,
        width_m: f32,
        visible: bool,
    ) -> VROverlayHandle;
    pub fn vr_destroy_overlay(handle: VROverlayHandle);
    pub fn vr_set_sort_order(order: u32);
    pub fn vr_overlay_poll(ev: *mut VREvent) -> bool;
    pub fn vr_center_in_front(distance: f32);
    pub fn vr_set_overlay_transform_tracked_device_relative(
        handle: VROverlayHandle,
        device_index: u32,
        transform: *const HmdMatrix34,
    );
    pub fn vr_show_dashboard(key: *const c_char);
    pub fn vr_compositor_sync();
    pub fn vr_set_overlay_width_meters(m: f32);

    pub fn vr_show_overlay(handle: VROverlayHandle);
    pub fn vr_hide_overlay(handle: VROverlayHandle);
    // Controller functions
    pub fn vr_update_controllers();
    pub fn vr_get_controller_connected(controller_idx: i32) -> bool;
    pub fn vr_get_controller_trigger_pressed(controller_idx: i32) -> bool;
    pub fn vr_get_controller_trigger_released(controller_idx: i32) -> bool;
    pub fn vr_test_laser_intersection(controller_idx: i32, handle: VROverlayHandle) -> LaserHit;
    pub fn vr_test_laser_intersection_main(controller_idx: i32) -> LaserHit;
    pub fn vr_trigger_haptic_pulse(controller_idx: i32, duration_us: u16);
    pub fn vr_find_hip_tracker() -> u32;

    pub fn vr_get_controller_menu_pressed(controller_idx: i32) -> bool;
    pub fn vr_keyboard_init_rendering(device: *mut c_void, context: *mut c_void) -> bool;
    pub fn vr_keyboard_render(
        handle: VROverlayHandle,
        selected_x: f32,
        selected_y: f32,
        current_text: *const c_char,
    ) -> bool;
    
    // ImGui functions
    pub fn imgui_init(device: *mut c_void, context: *mut c_void);
    pub fn imgui_shutdown();
    pub fn imgui_render_and_submit(width: u32, height: u32, is_dashboard: bool) -> bool;
    pub fn imgui_render_hud(width: u32, height: u32) -> bool;
    pub fn imgui_render_dashboard(width: u32, height: u32) -> bool;
    pub fn imgui_update_chat_state(
        messages_ptr: *const u8,
        messages_count: usize,
        input_buffer: *mut u8,
        input_capacity: usize,
    );
    pub fn imgui_get_sent_message(buffer: *mut u8, capacity: usize) -> bool;
    pub fn imgui_inject_mouse_pos(x: f32, y: f32);
    pub fn imgui_inject_mouse_button(button: i32, down: bool);
    pub fn imgui_update_laser_state(controller_idx: i32, hit: bool, x: f32, y: f32);
    pub fn imgui_get_input_focused() -> bool;
    pub fn vr_get_controller_trigger_value(controller_idx: i32) -> f32;
    pub fn vr_wait_get_poses();
    
    // Dashboard settings functions
    pub fn imgui_update_dashboard_state(state: *const DashboardState);
    pub fn imgui_update_overlay_settings(settings: *const OverlaySettingsFFI);
    pub fn imgui_get_dashboard_state(state: *mut DashboardState) -> bool;
}

// Safe wrappers
pub fn init() -> anyhow::Result<()> {
    unsafe {
        if vr_init_overlay() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("OpenVR init failed"))
        }
    }
}

pub fn create_overlay(key: &str, name: &str, width: f32, dashboard: bool) -> anyhow::Result<()> {
    let k = CString::new(key)?;
    let n = CString::new(name)?;
    unsafe {
        if vr_create_overlay(k.as_ptr(), n.as_ptr(), width, dashboard) {
            Ok(())
        } else {
            Err(anyhow::anyhow!("overlay create failed"))
        }
    }
}

pub fn create_overlay_raw(key: &str, name: &str, width: f32, visible: bool) -> anyhow::Result<VROverlayHandle> {
    let k = CString::new(key)?;
    let n = CString::new(name)?;
    unsafe {
        let handle = vr_create_overlay_raw(k.as_ptr(), n.as_ptr(), width, visible);
        if handle != 0 {
            Ok(handle)
        } else {
            Err(anyhow::anyhow!("overlay create failed"))
        }
    }
}

pub fn show_dashboard(key: &str) {
    let c = CString::new(key).unwrap();
    unsafe { vr_show_dashboard(c.as_ptr()) };
}

#[inline(always)]
pub fn set_overlay_width(meters: f32) {
    unsafe { vr_set_overlay_width_meters(meters) }
}

#[inline(always)]
pub fn compositor_sync() {
    unsafe { vr_compositor_sync() }
}

#[inline(always)]
pub fn update_controllers() {
    unsafe { vr_update_controllers() }
}

#[inline(always)]
pub fn find_hip_tracker() -> Option<u32> {
    unsafe {
        let idx = vr_find_hip_tracker();
        if idx != u32::MAX {
            Some(idx)
        } else {
            None
        }
    }
}

pub const K_UNTRACKED_DEVICE_INDEX_HMD: u32 = 0;