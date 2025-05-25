#include <openvr.h>
#include <d3d11.h>
#include <d3dcompiler.h>
#include <vector>
#include <string>
#include <cstring>
#include <cfloat>

#include "imgui.h"
#include "backends/imgui_impl_dx11.h"

#pragma comment(lib, "d3d11.lib")
#pragma comment(lib, "d3dcompiler.lib")

using namespace vr;

// ─────────────────────────── OpenVR State ──────────────────────────────
static VROverlayHandle_t g_handle         = k_ulOverlayHandleInvalid;
static VROverlayHandle_t g_keyboard_handle = k_ulOverlayHandleInvalid;
static IVROverlay*       g_vro            = nullptr;
static IVRSystem*        g_vrs            = nullptr;
static IVRCompositor*    g_vrc            = nullptr;

// ─────────────────────────── Controller State ──────────────────────────
struct ControllerState {
    bool connected = false;
    TrackedDeviceIndex_t device_index = k_unTrackedDeviceIndexInvalid;
    VRControllerState_t state;
    VRControllerState_t prev_state;
    HmdMatrix34_t pose;
    bool has_pose = false;
    bool trigger_pressed = false;
    bool trigger_released = false;
};

static ControllerState g_controllers[2]; // [0] = left, [1] = right


// ─────────────────────────── D3D11 State ───────────────────────────────
static ID3D11Device*           g_device        = nullptr;
static ID3D11DeviceContext*    g_context       = nullptr;
static ID3D11Texture2D*        g_textures[2]   = {nullptr, nullptr};
static ID3D11RenderTargetView* g_rtvs[2]       = {nullptr, nullptr};
static ID3D11ShaderResourceView* g_srvs[2]     = {nullptr, nullptr};
static int                     g_current_tex   = 0;

static ID3D11Texture2D*        g_keyboard_textures[2] = {nullptr, nullptr};
static ID3D11RenderTargetView* g_keyboard_rtvs[2]     = {nullptr, nullptr};
static ImGuiContext*           g_keyboard_imgui_ctx   = nullptr;
static int                     g_keyboard_current_tex  = 0;

// ─────────────────────────── ImGui State ───────────────────────────────
static ImGuiContext* g_imgui_ctx = nullptr;
static float g_mouse_x = 0;
static float g_mouse_y = 0;
static bool g_mouse_down = false;

// ─────────────────────────── Chat State ─────────────────────────────────
struct ChatMessage {
    char author[64];
    char text[256];
};

static std::vector<ChatMessage> g_chat_messages;
static char g_input_buffer[256] = {0};
static bool g_message_sent = false;

// ─────────────────────────── Laser Hit Info ────────────────────────────
struct LaserPointerState {
    bool active;
    float x, y;
};

static LaserPointerState g_laser_states[2] = {{false, 0, 0}, {false, 0, 0}};

struct LaserHit {
    bool hit;
    float u, v;
    float distance;
};

extern "C" void vr_show_overlay(VROverlayHandle_t handle) {
    if (handle != k_ulOverlayHandleInvalid) {
        VROverlay()->ShowOverlay(handle);
    }
}

extern "C" void vr_hide_overlay(VROverlayHandle_t handle) {
    if (handle != k_ulOverlayHandleInvalid) {
        VROverlay()->HideOverlay(handle);
    }
}

extern "C" float vr_get_controller_trigger_value(int controller_idx) {
    if (controller_idx < 0 || controller_idx > 1) return 0.0f;
    if (!g_controllers[controller_idx].connected) return 0.0f;

    return g_controllers[controller_idx].state.rAxis[1].x;
}
// ─────────────────────────── OpenVR Functions ──────────────────────────
extern "C" bool vr_init_overlay() {
    EVRInitError e = VRInitError_None;
    if (VR_Init(&e, VRApplication_Overlay) != nullptr && e == VRInitError_None) {
        g_vro = VROverlay();
        g_vrs = VRSystem();
        g_vrc = VRCompositor();
        return true;
    }
    return false;
}

extern "C" void vr_shutdown() {
    if (g_handle) g_vro->DestroyOverlay(g_handle);
    if (g_keyboard_handle) g_vro->DestroyOverlay(g_keyboard_handle);
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
    // Try removing this line or using None:
    // VROverlay()->SetOverlayInputMethod(g_handle, VROverlayInputMethod_Mouse);
    VROverlay()->SetOverlayInputMethod(g_handle, VROverlayInputMethod_Mouse);

    // Enable these flags for better interaction
    VROverlay()->SetOverlayFlag(g_handle, VROverlayFlags_SendVRDiscreteScrollEvents, true);
    VROverlay()->SetOverlayFlag(g_handle, VROverlayFlags_SendVRSmoothScrollEvents, true);
    VROverlay()->SetOverlayFlag(g_handle, VROverlayFlags_ShowTouchPadScrollWheel, false);

    // This flag is important for laser interaction
    VROverlay()->SetOverlayFlag(g_handle, VROverlayFlags_VisibleInDashboard, dashboard);

    return true;
}

extern "C" VROverlayHandle_t vr_create_overlay_raw(const char* key, const char* name,
                                                   float width_m, bool visible) {
    VROverlayHandle_t handle = k_ulOverlayHandleInvalid;
    EVROverlayError oe = VROverlay()->CreateOverlay(key, name, &handle);

    if (oe == VROverlayError_None) {
        VROverlay()->SetOverlayWidthInMeters(handle, width_m);
        VROverlay()->SetOverlayInputMethod(handle, VROverlayInputMethod_Mouse);
        if (visible) {
            VROverlay()->ShowOverlay(handle);
        }
        return handle;
    }

    return k_ulOverlayHandleInvalid;
}

extern "C" void vr_destroy_overlay(VROverlayHandle_t handle) {
    if (handle != k_ulOverlayHandleInvalid) {
        VROverlay()->DestroyOverlay(handle);
    }
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

extern "C" void vr_set_overlay_transform_tracked_device_relative(
    VROverlayHandle_t handle, uint32_t device_index, const HmdMatrix34_t* transform) {
    if (handle != k_ulOverlayHandleInvalid) {
        VROverlay()->SetOverlayTransformTrackedDeviceRelative(handle, device_index, transform);
    }
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
        // Use PostPresentHandoff instead of WaitGetPoses to avoid blocking
        comp->PostPresentHandoff();
    }
}

// Also add a proper frame timing function
extern "C" void vr_wait_get_poses() {
    if (auto* comp = VRCompositor()) {
        TrackedDevicePose_t poses[k_unMaxTrackedDeviceCount];
        comp->WaitGetPoses(poses, k_unMaxTrackedDeviceCount, nullptr, 0);
    }
}

// Add keyboard initialization
extern "C" bool vr_keyboard_init_rendering(void* device_ptr, void* context_ptr) {
    if (!device_ptr || !context_ptr) return false;

    // Create keyboard textures (smaller size)
    const int width = 512;
    const int height = 384;

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

    ID3D11Device* device = (ID3D11Device*)device_ptr;

    for (int i = 0; i < 2; i++) {
        if (FAILED(device->CreateTexture2D(&desc, nullptr, &g_keyboard_textures[i])))
            return false;
        if (FAILED(device->CreateRenderTargetView(g_keyboard_textures[i], nullptr, &g_keyboard_rtvs[i])))
            return false;
    }

    // Create separate ImGui context for keyboard
    g_keyboard_imgui_ctx = ImGui::CreateContext();
    ImGui::SetCurrentContext(g_keyboard_imgui_ctx);

    ImGuiIO& io = ImGui::GetIO();
    io.DisplaySize = ImVec2((float)width, (float)height);

    ImGui::StyleColorsDark();
    ImGuiStyle& style = ImGui::GetStyle();
    style.ScaleAllSizes(2.0f);  // Larger for VR
    io.FontGlobalScale = 2.0f;

    ImGui_ImplDX11_Init(device, (ID3D11DeviceContext*)context_ptr);

    // Switch back to main context
    ImGui::SetCurrentContext(g_imgui_ctx);

    return true;
}

// Keyboard rendering with key layout
extern "C" bool vr_keyboard_render(VROverlayHandle_t handle,
                                  float selected_x, float selected_y,
                                  const char* current_text) {
    if (handle == k_ulOverlayHandleInvalid) return false;

    // Switch to keyboard context
    ImGui::SetCurrentContext(g_keyboard_imgui_ctx);

    ImGui_ImplDX11_NewFrame();
    ImGui::NewFrame();

    // Clear background
    float clear_color[4] = { 0.1f, 0.1f, 0.1f, 0.95f };
    g_context->ClearRenderTargetView(g_keyboard_rtvs[g_keyboard_current_tex], clear_color);

    // Render keyboard UI
    ImGui::SetNextWindowPos(ImVec2(0, 0));
    ImGui::SetNextWindowSize(ImVec2(512, 384));
    ImGui::Begin("Keyboard", nullptr,
        ImGuiWindowFlags_NoTitleBar |
        ImGuiWindowFlags_NoResize |
        ImGuiWindowFlags_NoMove |
        ImGuiWindowFlags_NoScrollbar);

    // Current text with cursor
    ImGui::Text("Text: %s_", current_text ? current_text : "");
    ImGui::Separator();

    // Draw keyboard buttons
    const char* rows[] = {
        "1234567890-=",
        "qwertyuiop",
        "asdfghjkl",
        "zxcvbnm"
    };

    float button_size = 35.0f;
    float spacing = 2.0f;

    // Track if we're hovering over any button
    bool any_hovered = false;

    for (int row = 0; row < 4; row++) {
        float x_offset = 10.0f + (row == 3 ? 30.0f : row * 15.0f);
        float y_offset = 80.0f + row * (button_size + spacing);

        ImGui::SetCursorPos(ImVec2(x_offset, y_offset));

        for (int i = 0; rows[row][i]; i++) {
            if (i > 0) ImGui::SameLine(0, spacing);

            char label[2] = { toupper(rows[row][i]), 0 };

            // Calculate button bounds
            float btn_x = x_offset + i * (button_size + spacing);
            float btn_y = y_offset;

            bool is_hovered = (selected_x >= btn_x &&
                             selected_x <= btn_x + button_size &&
                             selected_y >= btn_y &&
                             selected_y <= btn_y + button_size);

            if (is_hovered) {
                any_hovered = true;
                ImGui::PushStyleColor(ImGuiCol_Button, ImVec4(0.3f, 0.7f, 1.0f, 1.0f));
                ImGui::PushStyleColor(ImGuiCol_ButtonHovered, ImVec4(0.4f, 0.8f, 1.0f, 1.0f));
            }

            if (ImGui::Button(label, ImVec2(button_size, button_size))) {
                // Button was clicked (this won't happen with VR input, but good for testing)
            }

            if (is_hovered) {
                ImGui::PopStyleColor(2);
            }
        }
    }

    // Special keys
    float special_y = 80.0f + 4 * (button_size + spacing) + 10.0f;

    ImGui::SetCursorPos(ImVec2(100.0f, special_y));
    bool space_hovered = (selected_x >= 100.0f && selected_x <= 300.0f &&
                         selected_y >= special_y && selected_y <= special_y + button_size);
    if (space_hovered) {
        ImGui::PushStyleColor(ImGuiCol_Button, ImVec4(0.3f, 0.7f, 1.0f, 1.0f));
        any_hovered = true;
    }
    ImGui::Button("Space", ImVec2(200.0f, button_size));
    if (space_hovered) ImGui::PopStyleColor();

    ImGui::SameLine(0, spacing);
    bool back_hovered = (selected_x >= 302.0f && selected_x <= 402.0f &&
                        selected_y >= special_y && selected_y <= special_y + button_size);
    if (back_hovered) {
        ImGui::PushStyleColor(ImGuiCol_Button, ImVec4(1.0f, 0.3f, 0.3f, 1.0f));
        any_hovered = true;
    }
    ImGui::Button("Back", ImVec2(100.0f, button_size));
    if (back_hovered) ImGui::PopStyleColor();

    ImGui::SameLine(0, spacing);
    bool enter_hovered = (selected_x >= 404.0f && selected_x <= 484.0f &&
                         selected_y >= special_y && selected_y <= special_y + button_size);
    if (enter_hovered) {
        ImGui::PushStyleColor(ImGuiCol_Button, ImVec4(0.3f, 1.0f, 0.3f, 1.0f));
        any_hovered = true;
    }
    ImGui::Button("Enter", ImVec2(80.0f, button_size));
    if (enter_hovered) ImGui::PopStyleColor();

    // Draw laser pointer on keyboard
    if (selected_x >= 0 && selected_y >= 0) {
        ImDrawList* draw_list = ImGui::GetWindowDrawList();
        ImU32 color = IM_COL32(255, 100, 100, 255);
        draw_list->AddCircleFilled(ImVec2(selected_x, selected_y), 5.0f, color);
        draw_list->AddCircle(ImVec2(selected_x, selected_y), 8.0f, IM_COL32(255, 255, 255, 200), 0, 2.0f);
    }

    ImGui::End();

    // Render
    ImGui::Render();
    g_context->OMSetRenderTargets(1, &g_keyboard_rtvs[g_keyboard_current_tex], nullptr);

    D3D11_VIEWPORT vp = {};
    vp.Width = 512.0f;
    vp.Height = 384.0f;
    vp.MaxDepth = 1.0f;
    g_context->RSSetViewports(1, &vp);

    ImGui_ImplDX11_RenderDrawData(ImGui::GetDrawData());

    // Submit to OpenVR
    Texture_t vr_tex = {};
    vr_tex.handle = g_keyboard_textures[g_keyboard_current_tex];
    vr_tex.eType = TextureType_DirectX;
    vr_tex.eColorSpace = ColorSpace_Gamma;

    VROverlayError err = VROverlay()->SetOverlayTexture(handle, &vr_tex);

    g_keyboard_current_tex = (g_keyboard_current_tex + 1) % 2;

    // Switch back to main context
    ImGui::SetCurrentContext(g_imgui_ctx);

    return err == VROverlayError_None;
}

// ─────────────────────────── Controller Functions ──────────────────────
extern "C" void vr_update_controllers() {
    TrackedDevicePose_t poses[k_unMaxTrackedDeviceCount];
    g_vrc->GetLastPoses(poses, k_unMaxTrackedDeviceCount, nullptr, 0);

    // Find and update controllers
    for (uint32_t i = 0; i < k_unMaxTrackedDeviceCount; i++) {
        if (g_vrs->GetTrackedDeviceClass(i) == TrackedDeviceClass_Controller) {
            ETrackedControllerRole role = g_vrs->GetControllerRoleForTrackedDeviceIndex(i);
            if (role == TrackedControllerRole_Invalid) continue;

            int idx = (role == TrackedControllerRole_LeftHand) ? 0 : 1;

            g_controllers[idx].device_index = i;
            g_controllers[idx].connected = poses[i].bDeviceIsConnected;
            g_controllers[idx].has_pose = poses[i].bPoseIsValid;

            if (poses[i].bPoseIsValid) {
                g_controllers[idx].pose = poses[i].mDeviceToAbsoluteTracking;
            }

            // Store previous state
            g_controllers[idx].prev_state = g_controllers[idx].state;

            // Get current state
            g_vrs->GetControllerState(i, &g_controllers[idx].state, sizeof(VRControllerState_t));

            // Check trigger state changes
            bool was_pressed = g_controllers[idx].prev_state.rAxis[1].x > 0.5f;
            bool is_pressed = g_controllers[idx].state.rAxis[1].x > 0.5f;

            g_controllers[idx].trigger_pressed = !was_pressed && is_pressed;
            g_controllers[idx].trigger_released = was_pressed && !is_pressed;
        }
    }
}

extern "C" bool vr_get_controller_menu_pressed(int controller_idx) {
    if (controller_idx < 0 || controller_idx > 1) return false;
    if (!g_controllers[controller_idx].connected) return false;

    // Menu button is typically button 1 in the button mask
    uint64_t menu_button_mask = ButtonMaskFromId(k_EButton_ApplicationMenu);
    bool was_pressed = (g_controllers[controller_idx].prev_state.ulButtonPressed & menu_button_mask) != 0;
    bool is_pressed = (g_controllers[controller_idx].state.ulButtonPressed & menu_button_mask) != 0;

    return !was_pressed && is_pressed;
}

extern "C" bool vr_get_controller_connected(int controller_idx) {
    if (controller_idx < 0 || controller_idx > 1) return false;
    return g_controllers[controller_idx].connected;
}

extern "C" bool vr_get_controller_trigger_pressed(int controller_idx) {
    if (controller_idx < 0 || controller_idx > 1) return false;
    return g_controllers[controller_idx].trigger_pressed;
}

extern "C" bool vr_get_controller_trigger_released(int controller_idx) {
    if (controller_idx < 0 || controller_idx > 1) return false;
    return g_controllers[controller_idx].trigger_released;
}

extern "C" LaserHit vr_test_laser_intersection(int controller_idx, VROverlayHandle_t handle) {
    LaserHit result = {false, 0, 0, FLT_MAX};

    if (controller_idx < 0 || controller_idx > 1) return result;
    if (!g_controllers[controller_idx].connected) return result;
    if (!g_controllers[controller_idx].has_pose) return result;
    if (handle == k_ulOverlayHandleInvalid) return result;

    // Get controller tip position and forward direction
    HmdMatrix34_t& pose = g_controllers[controller_idx].pose;
    HmdVector3_t origin = {pose.m[0][3], pose.m[1][3], pose.m[2][3]};
    // Forward is -Z in controller space
    HmdVector3_t direction = {-pose.m[0][2], -pose.m[1][2], -pose.m[2][2]};

    VROverlayIntersectionParams_t params;
    params.eOrigin = TrackingUniverseStanding;
    params.vSource = origin;
    params.vDirection = direction;

    VROverlayIntersectionResults_t results;
    if (g_vro->ComputeOverlayIntersection(handle, &params, &results)) {
        result.hit = true;
        result.u = results.vUVs.v[0];
        result.v = results.vUVs.v[1];
        result.distance = results.fDistance;
    }

    return result;
}

// Default test with main overlay
extern "C" LaserHit vr_test_laser_intersection_main(int controller_idx) {
    return vr_test_laser_intersection(controller_idx, g_handle);
}

extern "C" void vr_trigger_haptic_pulse(int controller_idx, unsigned short duration_us) {
    if (controller_idx < 0 || controller_idx > 1) return;
    if (!g_controllers[controller_idx].connected) return;

    g_vrs->TriggerHapticPulse(g_controllers[controller_idx].device_index, 0, duration_us);
}

extern "C" uint32_t vr_find_hip_tracker() {
    for (uint32_t i = 0; i < k_unMaxTrackedDeviceCount; i++) {
        if (g_vrs->GetTrackedDeviceClass(i) == TrackedDeviceClass_GenericTracker) {
            TrackedDevicePose_t pose;
            g_vrc->GetLastPoses(&pose, 1, nullptr, 0);

            if (pose.bPoseIsValid) {
                float y = pose.mDeviceToAbsoluteTracking.m[1][3];
                // Hip trackers are typically 0.8-1.2m high
                if (y > 0.8f && y < 1.2f) {
                    return i;
                }
            }
        }
    }
    return k_unTrackedDeviceIndexInvalid;
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

extern "C" void imgui_inject_mouse_pos(float x, float y) {
    g_mouse_x = x;
    g_mouse_y = y;
}

extern "C" void imgui_inject_mouse_button(int button, bool down) {
    if (button == 0) {
        g_mouse_down = down;
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

extern "C" void imgui_update_laser_state(int controller_idx, bool hit, float x, float y) {
    if (controller_idx >= 0 && controller_idx < 2) {
        g_laser_states[controller_idx].active = hit;
        g_laser_states[controller_idx].x = x;
        g_laser_states[controller_idx].y = y;
    }
}

static void render_laser_pointers() {
    ImDrawList* draw_list = ImGui::GetForegroundDrawList();

    for (int i = 0; i < 2; i++) {
        if (g_laser_states[i].active) {
            ImU32 color = (i == 0) ? IM_COL32(100, 200, 255, 255) : IM_COL32(255, 200, 100, 255);

            // Draw a more visible laser pointer
            float x = g_laser_states[i].x;
            float y = g_laser_states[i].y;

            // Outer ring
            draw_list->AddCircle(ImVec2(x, y), 20.0f, IM_COL32(255, 255, 255, 128), 0, 3.0f);
            // Middle ring
            draw_list->AddCircle(ImVec2(x, y), 15.0f, color, 0, 2.0f);
            // Inner filled circle
            draw_list->AddCircleFilled(ImVec2(x, y), 8.0f, color);
            // Center dot
            draw_list->AddCircleFilled(ImVec2(x, y), 3.0f, IM_COL32(255, 255, 255, 255));
        }
    }
}

extern "C" bool imgui_render_and_submit(uint32_t width, uint32_t height, bool is_dashboard) {
    // Update mouse from injected position
    ImGuiIO& io = ImGui::GetIO();
    io.MousePos = ImVec2(g_mouse_x, g_mouse_y);
    io.MouseDown[0] = g_mouse_down;

    // Start new frame
    ImGui_ImplDX11_NewFrame();
    ImGui::NewFrame();

    // Clear background
    float clear_color[4] = { 0.05f, 0.05f, 0.05f, 0.95f };
    g_context->ClearRenderTargetView(g_rtvs[g_current_tex], clear_color);

    // Render chat window
    render_chat_window(is_dashboard);

    // Render laser pointers on top of everything
    render_laser_pointers();

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