#include <openvr.h>
#include <d3d11.h>
#include <d3dcompiler.h>
#include <vector>
#include <string>
#include <cstring>

#include "imgui.h"
#include "backends/imgui_impl_dx11.h"

#pragma comment(lib, "d3d11.lib")
#pragma comment(lib, "d3dcompiler.lib")

using namespace vr;

// ─────────────────────────── OpenVR State ──────────────────────────────
static VROverlayHandle_t g_handle = k_ulOverlayHandleInvalid;
static IVROverlay*       g_vro    = nullptr;

// ─────────────────────────── D3D11 State ───────────────────────────────
static ID3D11Device*           g_device        = nullptr;
static ID3D11DeviceContext*    g_context       = nullptr;
static ID3D11Texture2D*        g_textures[2]   = {nullptr, nullptr};
static ID3D11RenderTargetView* g_rtvs[2]       = {nullptr, nullptr};
static ID3D11ShaderResourceView* g_srvs[2]     = {nullptr, nullptr};
static int                     g_current_tex   = 0;

// ─────────────────────────── ImGui State ───────────────────────────────
static ImGuiContext* g_imgui_ctx = nullptr;

// ─────────────────────────── Chat State ─────────────────────────────────
struct ChatMessage {
    char author[64];
    char text[256];
};

static std::vector<ChatMessage> g_chat_messages;
static char g_input_buffer[256] = {0};
static bool g_message_sent = false;

// ─────────────────────────── OpenVR Functions (unchanged) ──────────────
extern "C" bool vr_init_overlay() {
    EVRInitError e = VRInitError_None;
    if (VR_Init(&e, VRApplication_Overlay) != nullptr && e == VRInitError_None) {
        g_vro = VROverlay();
        return true;
    }
    return false;
}

extern "C" void vr_shutdown() {
    if (g_handle) g_vro->DestroyOverlay(g_handle);
    VR_Shutdown();
}

extern "C" bool vr_create_overlay(const char* key, const char* name,
                                  float width_m, bool dashboard) {
    EVROverlayError oe;
    if (dashboard) {
        VROverlayHandle_t thumb;
        oe = VROverlay()->CreateDashboardOverlay(key, name, &g_handle, &thumb);
        if (oe != VROverlayError_None) return false;
        VROverlay()->ShowDashboard(key);
    } else {
        oe = VROverlay()->CreateOverlay(key, name, &g_handle);
        if (oe != VROverlayError_None) return false;
        VROverlay()->ShowOverlay(g_handle);
    }

    VROverlay()->SetOverlayWidthInMeters(g_handle, width_m);
    VROverlay()->SetOverlayInputMethod(g_handle, VROverlayInputMethod_Mouse);
    return true;
}

extern "C" bool vr_overlay_poll(VREvent_t* e) {
    return g_vro->PollNextOverlayEvent(g_handle, e, sizeof(VREvent_t));
}

extern "C" void vr_center_in_front(float meters) {
    if (g_handle == k_ulOverlayHandleInvalid) return;
    HmdMatrix34_t m{};
    m.m[2][3] = -meters;
    m.m[0][0] = m.m[1][1] = m.m[2][2] = 1.0f;
    VROverlay()->SetOverlayTransformTrackedDeviceRelative(
        g_handle, k_unTrackedDeviceIndex_Hmd, &m);
}

extern "C" void vr_show_dashboard(const char* key) {
    VROverlay()->ShowDashboard(key);
}

extern "C" void vr_set_sort_order(uint32_t order) {
    if (g_handle == k_ulOverlayHandleInvalid) return;
    VROverlay()->SetOverlaySortOrder(g_handle, order);
}

extern "C" void vr_set_overlay_width_meters(float meters) {
    if (g_handle != k_ulOverlayHandleInvalid)
        VROverlay()->SetOverlayWidthInMeters(g_handle, meters);
}

extern "C" void vr_compositor_sync() {
    if (auto* comp = VRCompositor()) {
        comp->WaitGetPoses(nullptr, 0, nullptr, 0);
    }
}

// ─────────────────────────── ImGui Functions ───────────────────────────
extern "C" void imgui_init(void* device_ptr, void* context_ptr) {
    g_device = (ID3D11Device*)device_ptr;
    g_context = (ID3D11DeviceContext*)context_ptr;
    
    // Create render targets
    const int width = 1024;
    const int height = 768;
    
    D3D11_TEXTURE2D_DESC desc = {};
    desc.Width = width;
    desc.Height = height;
    desc.MipLevels = 1;
    desc.ArraySize = 1;
    desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    desc.SampleDesc.Count = 1;
    desc.Usage = D3D11_USAGE_DEFAULT;
    desc.BindFlags = D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE;
    desc.MiscFlags = D3D11_RESOURCE_MISC_SHARED;
    
    for (int i = 0; i < 2; i++) {
        g_device->CreateTexture2D(&desc, nullptr, &g_textures[i]);
        g_device->CreateRenderTargetView(g_textures[i], nullptr, &g_rtvs[i]);
        g_device->CreateShaderResourceView(g_textures[i], nullptr, &g_srvs[i]);
    }
    
    // Initialize ImGui
    IMGUI_CHECKVERSION();
    g_imgui_ctx = ImGui::CreateContext();
    ImGui::SetCurrentContext(g_imgui_ctx);
    
    ImGuiIO& io = ImGui::GetIO();
    io.DisplaySize = ImVec2((float)width, (float)height);
    io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard;
    
    ImGui::StyleColorsDark();
    
    // Customize style for VR
    ImGuiStyle& style = ImGui::GetStyle();
    style.WindowRounding = 5.0f;
    style.FrameRounding = 3.0f;
    style.ScrollbarRounding = 3.0f;
    style.GrabRounding = 3.0f;
    style.WindowBorderSize = 0.0f;
    
    // Scale for VR readability
    style.ScaleAllSizes(1.5f);
    io.FontGlobalScale = 1.5f;
    
    ImGui_ImplDX11_Init(g_device, g_context);
}

extern "C" void imgui_shutdown() {
    ImGui_ImplDX11_Shutdown();
    ImGui::DestroyContext(g_imgui_ctx);
    
    for (int i = 0; i < 2; i++) {
        if (g_rtvs[i]) g_rtvs[i]->Release();
        if (g_srvs[i]) g_srvs[i]->Release();
        if (g_textures[i]) g_textures[i]->Release();
    }
}

extern "C" void imgui_update_chat_state(const uint8_t* messages_ptr, size_t messages_count,
                                       uint8_t* input_buffer, size_t input_capacity) {
    // Update messages
    g_chat_messages.clear();
    if (messages_ptr && messages_count > 0) {
        const ChatMessage* msgs = (const ChatMessage*)messages_ptr;
        for (size_t i = 0; i < messages_count; i++) {
            g_chat_messages.push_back(msgs[i]);
        }
    }
}

extern "C" bool imgui_get_sent_message(uint8_t* buffer, size_t capacity) {
    if (g_message_sent && buffer && capacity > 0) {
        strncpy((char*)buffer, g_input_buffer, capacity - 1);
        buffer[capacity - 1] = 0;
        g_message_sent = false;
        g_input_buffer[0] = 0;
        return true;
    }
    return false;
}

static void render_chat_window(bool is_dashboard) {
    ImGui::SetNextWindowPos(ImVec2(10, 10), ImGuiCond_FirstUseEver);
    ImGui::SetNextWindowSize(ImVec2(1004, 748), ImGuiCond_FirstUseEver);
    
    ImGui::Begin("Chat", nullptr, 
        ImGuiWindowFlags_NoCollapse | 
        ImGuiWindowFlags_NoTitleBar |
        ImGuiWindowFlags_NoResize |
        ImGuiWindowFlags_NoMove);
    
    // Title
    ImGui::TextColored(ImVec4(0.7f, 0.9f, 1.0f, 1.0f), "maowbot %s", 
        is_dashboard ? "Dashboard" : "HUD");
    ImGui::Separator();
    
    // Chat area
    ImVec2 chat_size = ImVec2(0, -ImGui::GetFrameHeightWithSpacing() - 10);
    if (ImGui::BeginChild("ChatArea", chat_size, true)) {
        for (const auto& msg : g_chat_messages) {
            ImGui::TextColored(ImVec4(0.8f, 0.8f, 0.2f, 1.0f), "%s:", msg.author);
            ImGui::SameLine();
            ImGui::TextWrapped("%s", msg.text);
        }
        
        // Auto-scroll
        if (ImGui::GetScrollY() >= ImGui::GetScrollMaxY())
            ImGui::SetScrollHereY(1.0f);
    }
    ImGui::EndChild();
    
    // Input
    ImGui::Separator();
    bool reclaim_focus = false;
    ImGuiInputTextFlags input_flags = ImGuiInputTextFlags_EnterReturnsTrue;
    
    if (ImGui::InputText("##Input", g_input_buffer, sizeof(g_input_buffer), input_flags)) {
        if (strlen(g_input_buffer) > 0) {
            g_message_sent = true;
            reclaim_focus = true;
        }
    }
    
    ImGui::SetItemDefaultFocus();
    if (reclaim_focus)
        ImGui::SetKeyboardFocusHere(-1);
    
    ImGui::End();
}

extern "C" bool imgui_render_and_submit(uint32_t width, uint32_t height, bool is_dashboard) {
    // Start new frame
    ImGui_ImplDX11_NewFrame();
    ImGui::NewFrame();
    
    // Clear background
    float clear_color[4] = { 0.05f, 0.05f, 0.05f, 0.95f };
    g_context->ClearRenderTargetView(g_rtvs[g_current_tex], clear_color);
    
    // Render chat window
    render_chat_window(is_dashboard);
    
    // Render to texture
    ImGui::Render();
    g_context->OMSetRenderTargets(1, &g_rtvs[g_current_tex], nullptr);
    
    D3D11_VIEWPORT vp = {};
    vp.Width = (float)width;
    vp.Height = (float)height;
    vp.MaxDepth = 1.0f;
    g_context->RSSetViewports(1, &vp);
    
    ImGui_ImplDX11_RenderDrawData(ImGui::GetDrawData());
    
    // Submit to OpenVR
    Texture_t vr_tex = {};
    vr_tex.handle = g_textures[g_current_tex];
    vr_tex.eType = TextureType_DirectX;
    vr_tex.eColorSpace = ColorSpace_Gamma;
    
    VROverlayError err = VROverlay()->SetOverlayTexture(g_handle, &vr_tex);
    
    // Swap buffers
    g_current_tex = (g_current_tex + 1) % 2;
    
    return err == VROverlayError_None;
}