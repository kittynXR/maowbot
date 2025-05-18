//! vr_layer.rs – single-texture, leak-free OpenVR overlay (Windows)

use bevy::prelude::*;
use tracing::{error, info};

use crate::ffi;
use windows::Win32::{
    Foundation::HMODULE,
    Graphics::{
        Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0},
        Direct3D11::*,
        Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM_SRGB, DXGI_SAMPLE_DESC},
    },
};
use windows::core::Interface;

#[derive(Resource)] struct FrameCounter(u64);
#[derive(Resource)] struct IsDashboard(bool);
#[derive(Resource)]
struct GpuOverlay {
    ctx:      ID3D11DeviceContext,
    textures: [ID3D11Texture2D; 2],          // ← double buffer
    srvs:     [ID3D11ShaderResourceView; 2],
    pixels:   Vec<u8>,
    index:    usize,                         // 0 / 1 toggle
    width:    u32,
    height:   u32,
}

pub struct VrOverlayPlugin { pub dashboard: bool }
impl Plugin for VrOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(IsDashboard(self.dashboard))
            .insert_resource(FrameCounter(0))
            .add_systems(Startup, setup)
            .add_systems(Update, submit_frame)
            .add_systems(Last, |mut e: EventReader<AppExit>| {
                if e.read().next().is_some() { unsafe { ffi::vr_shutdown() } }
            });
    }
}

fn setup(mut cmd: Commands, dash: Res<IsDashboard>) {
    /* ── OpenVR bootstrap ─────────────────────────────────────────── */
    let (key, name) = if dash.0 {
        ("maowbot.overlay.dashboard", "maowbot Dashboard")
    } else {
        ("maowbot.overlay.hud", "maowbot HUD")
    };
    ffi::init().expect("OpenVR init failed");
    ffi::create_overlay(key, name, 1.0, dash.0).unwrap();
    ffi::set_overlay_width(1.0);
    unsafe { ffi::vr_center_in_front(1.5) };

    unsafe{ ffi::vr_set_overlay_width_meters(1.0) };

    if dash.0 { ffi::show_dashboard(key); }
    info!("✔ OpenVR overlay ready");

    /* ── D3D11 device + texture + SRV ─────────────────────────────── */
    const W: u32 = 512;
    const H: u32 = 512;

    let mut device  : Option<ID3D11Device>        = None;
    let mut context : Option<ID3D11DeviceContext> = None;
    let mut feat    : D3D_FEATURE_LEVEL           = D3D_FEATURE_LEVEL_11_0;

    unsafe {
        D3D11CreateDevice(
            None,                           // adapter
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE(std::ptr::null_mut()),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,                           // pFeatureLevels array
            D3D11_SDK_VERSION,
            Some(&mut device),
            Some(&mut feat),
            Some(&mut context),
        )
            .expect("D3D11CreateDevice failed");
    }
    let device  = device .unwrap();
    let context = context.unwrap();

    /* texture ---------------------------------------------------------------- */
    let desc = D3D11_TEXTURE2D_DESC {
        Width: W,
        Height: H,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: D3D11_RESOURCE_MISC_SHARED.0 as u32,
    };

    let mut texture: Option<ID3D11Texture2D> = None;
    unsafe {
        device.CreateTexture2D(&desc, None, Some(&mut texture))
            .expect("CreateTexture2D");
    }
    let texture = texture.unwrap();

    /* shader-resource view --------------------------------------------------- */
    let mut srv: Option<ID3D11ShaderResourceView> = None;
    unsafe {
        device.CreateShaderResourceView(&texture, None, Some(&mut srv))
            .expect("CreateSRV");
    }
    let srv = srv.unwrap();

    let make_tex = |device: &ID3D11Device| -> (ID3D11Texture2D, ID3D11ShaderResourceView) {
        let mut t = None;
        unsafe { device.CreateTexture2D(&desc, None, Some(&mut t)).expect("tex") }
        let tex = t.unwrap();
        let mut v = None;
        unsafe { device.CreateShaderResourceView(&tex, None, Some(&mut v)).expect("srv") }
        (tex, v.unwrap())
    };

    let (t0, s0) = make_tex(&device);
    let (t1, s1) = make_tex(&device);

    cmd.insert_resource(GpuOverlay {
        ctx: context,
        textures: [t0, t1],
        srvs:     [s0, s1],
        pixels:   vec![0; (W * H * 4) as usize],
        index:    0,
        width:    W,
        height:   H,
    });
    
    
}

fn submit_frame(
    mut frame: ResMut<FrameCounter>,
    mut gpu:   ResMut<GpuOverlay>,
) {
    /* 0) block until compositor is ready ---------------------------------- */
    ffi::compositor_sync();

    /* 1) pick back buffer & fill CPU pixels -------------------------------- */
    let back = (gpu.index ^ 1) & 1;                // 0 ↔ 1 toggle
    for px in gpu.pixels.chunks_exact_mut(4) {
        px.copy_from_slice(&[255, 0, 255, 255]);
    }

    /* 2) upload pixels into the chosen texture ----------------------------- */
    unsafe {
        gpu.ctx.UpdateSubresource(
            &gpu.textures[back],
            0,
            None,
            gpu.pixels.as_ptr() as *const _,
            (gpu.width * 4) as u32,
            0,
        );
    }

    /* 3) submit that texture to SteamVR ------------------------------------ */
    let ok = unsafe {
        ffi::vr_submit_d3d11(gpu.textures[back].as_raw().cast())
    };

    unsafe {
        gpu.ctx.Flush();
    }

    if ok {
        // ffi::release_last_gpu_copy();
        gpu.index ^= 1;
        info!("frame {}", frame.0);
    } else {
        error!("✘ SetOverlayTexture failed (frame {})", frame.0);
    }

    frame.0 += 1;
}
