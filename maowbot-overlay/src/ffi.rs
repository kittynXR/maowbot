use libc::{c_char,c_void};
use std::ffi::CString;

#[repr(C)]
pub struct VREvent { pub(crate) _data:[u8;64] }  // opaque for now

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum VROverlayError {
    None                       = 0,
    InvalidHandle              = 10,
    InvalidParameter           = 11,
    FileNotFound               = 12,
    // (add more codes here as needed)
    Unknown                    = 99_999,      // fallback
}

extern "C" {
    pub fn vr_init_overlay() -> bool;
    pub fn vr_shutdown();
    pub fn vr_create_overlay(
        key:*const c_char, name:*const c_char,
        width_m:f32, dashboard:bool
    ) -> bool;
    pub fn vr_set_sort_order(order: u32);
    pub fn vr_submit_raw(data:*const c_void,w:u32,h:u32,bpp:u32) -> bool;

    #[cfg(target_os="windows")]
    pub fn vr_submit_d3d11(tex: *mut c_void) -> bool;
    #[cfg(not(target_os="windows"))]
    pub fn vr_submit_vulkan(data:*mut c_void) -> bool;

    pub fn vr_submit_d3d11_err(
        tex: *mut core::ffi::c_void,
        out: *mut VROverlayError
    ) -> i32;

    pub fn vr_overlay_poll(ev:*mut VREvent) -> bool;

    pub fn vr_center_in_front(distance: f32);

    pub fn vr_show_dashboard(key: *const c_char);

    pub fn vr_compositor_sync();

    pub fn vr_clear_overlay_texture();

    pub fn vr_set_overlay_width_meters(m: f32);
}

/// Block until the compositor is ready for a new overlay frame.
#[inline(always)]
pub fn compositor_sync() {
    unsafe { vr_compositor_sync() }
}

#[inline(always)]
pub fn release_last_gpu_copy() {
    unsafe { vr_clear_overlay_texture() }
}

/* safe rust wrappers */
pub fn init() -> anyhow::Result<()> {
    unsafe { if vr_init_overlay() { Ok(()) } else { Err(anyhow::anyhow!("OpenVR init failed")) } }
}
pub fn create_overlay(key:&str,name:&str,width:f32,dashboard:bool) -> anyhow::Result<()> {
    let k=CString::new(key)?; let n=CString::new(name)?;
    unsafe { if vr_create_overlay(k.as_ptr(),n.as_ptr(),width,dashboard) {
        Ok(())
    } else { Err(anyhow::anyhow!("overlay create failed")) } }
}

pub fn show_dashboard(key:&str) {
    let c = CString::new(key).unwrap();
    unsafe { vr_show_dashboard(c.as_ptr()) };
}

#[inline(always)]
pub fn set_overlay_width(meters: f32) {
    unsafe { vr_set_overlay_width_meters(meters) }
}