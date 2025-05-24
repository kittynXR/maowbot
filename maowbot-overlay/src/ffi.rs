use libc::{c_char, c_void};
use std::ffi::CString;

#[repr(C)]
pub struct VREvent {
    pub(crate) _data: [u8; 64],
}

extern "C" {
    // OpenVR functions (unchanged)
    pub fn vr_init_overlay() -> bool;
    pub fn vr_shutdown();
    pub fn vr_create_overlay(
        key: *const c_char,
        name: *const c_char,
        width_m: f32,
        dashboard: bool,
    ) -> bool;
    pub fn vr_set_sort_order(order: u32);
    pub fn vr_overlay_poll(ev: *mut VREvent) -> bool;
    pub fn vr_center_in_front(distance: f32);
    pub fn vr_show_dashboard(key: *const c_char);
    pub fn vr_compositor_sync();
    pub fn vr_set_overlay_width_meters(m: f32);

    // ImGui functions (new)
    pub fn imgui_init(device: *mut c_void, context: *mut c_void);
    pub fn imgui_shutdown();
    pub fn imgui_render_and_submit(width: u32, height: u32, is_dashboard: bool) -> bool;
    pub fn imgui_update_chat_state(
        messages_ptr: *const u8,
        messages_count: usize,
        input_buffer: *mut u8,
        input_capacity: usize,
    );
    pub fn imgui_get_sent_message(buffer: *mut u8, capacity: usize) -> bool;
}

// Safe wrappers (unchanged)
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