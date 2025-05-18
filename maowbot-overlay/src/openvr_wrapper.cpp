// put the OpenVR headers path in build.rs include list
#include <openvr.h>
#ifdef _WIN32
  #include <d3d11.h>
  #pragma comment(lib,"d3d11.lib")
#endif

using namespace vr;
static VROverlayHandle_t g_handle = k_ulOverlayHandleInvalid;
static IVROverlay*       g_vro    = nullptr;

/*──────────────────────── basic lifecycle ───────────────────────────*/
extern "C" bool vr_init_overlay() {
    EVRInitError e = VRInitError_None;
    if (VR_Init(&e, VRApplication_Overlay) != nullptr && e == VRInitError_None) {
        g_vro = VROverlay();
        return true;
    }
    return false;
}
extern "C" void vr_shutdown() { if (g_handle) g_vro->DestroyOverlay(g_handle); VR_Shutdown(); }

/*──────────────────────── overlay setup ─────────────────────────────*/
extern "C" bool vr_create_overlay(const char* key,const char* name,
                                  float width_m,bool dashboard)
{
    EVROverlayError oe;
    if (dashboard) {
        VROverlayHandle_t thumb;
        oe = VROverlay()->CreateDashboardOverlay(key,name,&g_handle,&thumb); // dashboard API ✔
        if (oe!=VROverlayError_None) return false;
        VROverlay()->ShowDashboard(key);                                     // pop tab immediately ✔
    } else {
        oe = VROverlay()->CreateOverlay(key,name,&g_handle);
        if (oe!=VROverlayError_None) return false;
        VROverlay()->ShowOverlay(g_handle);
    }

    VROverlay()->SetOverlayWidthInMeters(g_handle,width_m);
    VROverlay()->SetOverlayInputMethod(g_handle,VROverlayInputMethod_Mouse);
    return true;
}

/*──────────────────────── texture submission ───────────────────────*/
extern "C" bool vr_submit_raw(const void* data,uint32_t w,uint32_t h,uint32_t bpp){
    if (g_handle==k_ulOverlayHandleInvalid) return false;
    return g_vro->SetOverlayRaw(g_handle,const_cast<void*>(data),w,h,bpp)==VROverlayError_None;
}

#ifdef _WIN32
// Direct3D 11 submission
extern "C" bool vr_submit_d3d11(void* tex)
{
    Texture_t t{};
    t.handle      = tex;
    t.eType       = TextureType_DirectX;
    t.eColorSpace = ColorSpace_Gamma;
    return VROverlay()->SetOverlayTexture(g_handle, &t) == VROverlayError_None;
}

#else
// Vulkan path (pointer to VRVulkanTextureData_t)
extern "C" bool vr_submit_vulkan(void* data){
    Texture_t t; t.handle=data; t.eType=TextureType_Vulkan; t.eColorSpace = ColorSpace_Gamma;
    return g_vro->SetOverlayTexture(g_handle,&t)==VROverlayError_None;
}
#endif

extern "C" int32_t vr_submit_d3d11_err(void* tex, VROverlayError* out)
{
    Texture_t t{};
    t.handle      = tex;
    t.eType       = TextureType_DirectX;
    t.eColorSpace = ColorSpace_Gamma;
    VROverlayError e = VROverlay()->SetOverlayTexture(g_handle, &t);
    if (out) *out = e;
    return static_cast<int32_t>(e);
}

/*──────────────────────── event poll ───────────────────────────────*/
extern "C" bool vr_overlay_poll(VREvent_t* e){
    return g_vro->PollNextOverlayEvent(g_handle,e,sizeof(VREvent_t));
}

extern "C" void vr_center_in_front(float meters)
{
    if (g_handle == k_ulOverlayHandleInvalid) return;
    HmdMatrix34_t m{};  m.m[2][3] = -meters;
    m.m[0][0]=m.m[1][1]=m.m[2][2]=1.0f;
    VROverlay()->SetOverlayTransformTrackedDeviceRelative(
        g_handle, k_unTrackedDeviceIndex_Hmd, &m);
}

extern "C" void vr_show_dashboard(const char* key) {
    // Use IVROverlay::ShowDashboard to pop up the dashboard overlay
    VROverlay()->ShowDashboard(key);
}

extern "C" void vr_set_sort_order(uint32_t order)
{
    if (g_handle == k_ulOverlayHandleInvalid) return;
    VROverlay()->SetOverlaySortOrder(g_handle, order);
}

extern "C" void vr_clear_overlay_texture()
{
    if (g_handle != k_ulOverlayHandleInvalid)
        VROverlay()->ClearOverlayTexture(g_handle);
}

// Set the physical width in metres so SteamVR knows the texture size.
extern "C" void vr_set_overlay_width_meters(float meters)
{
    if (g_handle != k_ulOverlayHandleInvalid)
        VROverlay()->SetOverlayWidthInMeters(g_handle, meters);
}

/*──────────────────────── compositor sync ────────────────────────────*/
extern "C" void vr_compositor_sync()
{
    if (auto *comp = VRCompositor())
    {
        // We don't need pose data for overlays, just the blocking call.
        // Passing nullptr arrays keeps it lightweight.
        comp->WaitGetPoses(nullptr, 0, nullptr, 0);
    }
}
