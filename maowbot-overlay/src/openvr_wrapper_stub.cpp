#include <cstring>
#include <cfloat>
#include <cstdint>
#include <vector>
#include <string>
#include <iostream>
#include <chrono>
#include <thread>

#include "imgui.h"

// Stub types to match OpenVR interface
using VROverlayHandle_t = uint64_t;
static const VROverlayHandle_t k_ulOverlayHandleInvalid = 0;

struct VREvent_t {
    uint8_t _data[64];
};

struct HmdMatrix34_t {
    float m[3][4];
};

struct LaserHit {
    bool hit;
    float u, v;
    float distance;
};

// Global state for stub
static VROverlayHandle_t g_handle = 1;  // Non-zero to indicate success
static VROverlayHandle_t g_keyboard_handle = 2;
static ImGuiContext* g_imgui_ctx = nullptr;
static ImGuiContext* g_keyboard_imgui_ctx = nullptr;

// Chat state
struct ChatMessage {
    char author[64];
    char text[256];
};

static std::vector<ChatMessage> g_chat_messages;
static char g_input_buffer[256] = {0};
static bool g_message_sent = false;
static bool g_input_focused = false;
static bool g_input_just_focused = false;

// Mouse state
static float g_mouse_x = 512.0f;
static float g_mouse_y = 384.0f;
static bool g_mouse_down = false;

// Stub OpenVR functions
extern "C" bool vr_init_overlay() {
    std::cout << "[STUB] VR initialized in stub mode\n";
    
    // Add some test messages
    ChatMessage msg1 = {};
    strcpy(msg1.author, "System");
    strcpy(msg1.text, "Running in VR stub mode - no actual VR hardware required");
    g_chat_messages.push_back(msg1);
    
    ChatMessage msg2 = {};
    strcpy(msg2.author, "Test");
    strcpy(msg2.text, "This is a test message in the stub implementation");
    g_chat_messages.push_back(msg2);
    
    return true;
}

extern "C" void vr_shutdown() {
    std::cout << "[STUB] VR shutdown\n";
}

extern "C" bool vr_create_overlay(const char* key, const char* name, float width_m, bool dashboard) {
    std::cout << "[STUB] Creating overlay: " << key << " (" << name << ")\n";
    return true;
}

extern "C" VROverlayHandle_t vr_create_overlay_raw(const char* key, const char* name,
                                                   float width_m, bool visible) {
    std::cout << "[STUB] Creating raw overlay: " << key << "\n";
    static VROverlayHandle_t next_handle = 3;
    return next_handle++;
}

extern "C" void vr_destroy_overlay(VROverlayHandle_t handle) {
    std::cout << "[STUB] Destroying overlay: " << handle << "\n";
}

extern "C" void vr_show_overlay(VROverlayHandle_t handle) {
    std::cout << "[STUB] Showing overlay: " << handle << "\n";
}

extern "C" void vr_hide_overlay(VROverlayHandle_t handle) {
    std::cout << "[STUB] Hiding overlay: " << handle << "\n";
}

extern "C" bool vr_overlay_poll(VREvent_t* e) {
    return false;  // No events in stub
}

extern "C" void vr_center_in_front(float meters) {
    std::cout << "[STUB] Centering overlay " << meters << " meters in front\n";
}

extern "C" void vr_set_overlay_transform_tracked_device_relative(
    VROverlayHandle_t handle, uint32_t device_index, const HmdMatrix34_t* transform) {
    std::cout << "[STUB] Setting overlay transform\n";
}

extern "C" void vr_show_dashboard(const char* key) {
    std::cout << "[STUB] Showing dashboard: " << key << "\n";
}

extern "C" void vr_set_sort_order(uint32_t order) {
    std::cout << "[STUB] Setting sort order: " << order << "\n";
}

extern "C" void vr_set_overlay_width_meters(float meters) {
    std::cout << "[STUB] Setting overlay width: " << meters << " meters\n";
}

extern "C" void vr_compositor_sync() {
    // No-op in stub
}

extern "C" void vr_wait_get_poses() {
    // Simulate VR frame timing (90 Hz)
    static auto last_time = std::chrono::high_resolution_clock::now();
    auto target_time = last_time + std::chrono::microseconds(11111); // ~90 FPS
    
    auto now = std::chrono::high_resolution_clock::now();
    if (now < target_time) {
        std::this_thread::sleep_until(target_time);
    }
    
    last_time = std::chrono::high_resolution_clock::now();
}

// Controller functions
extern "C" void vr_update_controllers() {
    // No-op in stub
}

extern "C" bool vr_get_controller_connected(int controller_idx) {
    return false;  // No controllers in stub
}

extern "C" bool vr_get_controller_trigger_pressed(int controller_idx) {
    return false;
}

extern "C" bool vr_get_controller_trigger_released(int controller_idx) {
    return false;
}

extern "C" float vr_get_controller_trigger_value(int controller_idx) {
    return 0.0f;
}

extern "C" bool vr_get_controller_menu_pressed(int controller_idx) {
    return false;
}

extern "C" LaserHit vr_test_laser_intersection(int controller_idx, VROverlayHandle_t handle) {
    LaserHit result = {false, 0, 0, FLT_MAX};
    return result;
}

extern "C" LaserHit vr_test_laser_intersection_main(int controller_idx) {
    LaserHit result = {false, 0, 0, FLT_MAX};
    return result;
}

extern "C" void vr_trigger_haptic_pulse(int controller_idx, unsigned short duration_us) {
    // No-op in stub
}

extern "C" uint32_t vr_find_hip_tracker() {
    return 0xFFFFFFFF;  // No tracker
}

// ImGui functions
extern "C" void imgui_init(void* device_ptr, void* context_ptr) {
    std::cout << "[STUB] Initializing ImGui\n";
    
    IMGUI_CHECKVERSION();
    g_imgui_ctx = ImGui::CreateContext();
    ImGui::SetCurrentContext(g_imgui_ctx);

    ImGuiIO& io = ImGui::GetIO();
    io.DisplaySize = ImVec2(1024.0f, 768.0f);
    io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard;
    io.ConfigInputTextCursorBlink = false;

    ImGui::StyleColorsDark();

    ImGuiStyle& style = ImGui::GetStyle();
    style.WindowRounding = 5.0f;
    style.FrameRounding = 3.0f;
    style.ScrollbarRounding = 3.0f;
    style.GrabRounding = 3.0f;
    style.WindowBorderSize = 0.0f;
    style.ScaleAllSizes(1.5f);
    io.FontGlobalScale = 1.5f;
}

extern "C" void imgui_shutdown() {
    std::cout << "[STUB] Shutting down ImGui\n";
    if (g_imgui_ctx) {
        ImGui::DestroyContext(g_imgui_ctx);
        g_imgui_ctx = nullptr;
    }
    if (g_keyboard_imgui_ctx) {
        ImGui::DestroyContext(g_keyboard_imgui_ctx);
        g_keyboard_imgui_ctx = nullptr;
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

extern "C" bool imgui_get_input_focused() {
    bool result = g_input_just_focused;
    g_input_just_focused = false;
    return result;
}

extern "C" void imgui_update_chat_state(const uint8_t* messages_ptr, size_t messages_count,
                                       uint8_t* input_buffer, size_t input_capacity) {
    // In stub mode, we maintain our own test messages
    // Don't clear them when updating from Rust
}

extern "C" bool imgui_get_sent_message(uint8_t* buffer, size_t capacity) {
    if (g_message_sent && buffer && capacity > 0) {
        strncpy((char*)buffer, g_input_buffer, capacity - 1);
        buffer[capacity - 1] = 0;
        g_message_sent = false;
        
        // Add the sent message to our local chat for testing
        ChatMessage msg = {};
        strcpy(msg.author, "You");
        strncpy(msg.text, g_input_buffer, sizeof(msg.text) - 1);
        g_chat_messages.push_back(msg);
        
        g_input_buffer[0] = 0;
        return true;
    }
    return false;
}

extern "C" void imgui_update_laser_state(int controller_idx, bool hit, float x, float y) {
    // No-op in stub
}

extern "C" bool imgui_render_and_submit(uint32_t width, uint32_t height, bool is_dashboard) {
    // In stub mode, we just update ImGui state without actual rendering
    ImGuiIO& io = ImGui::GetIO();
    io.MousePos = ImVec2(g_mouse_x, g_mouse_y);
    io.MouseDown[0] = g_mouse_down;

    ImGui::NewFrame();

    // Render chat window
    ImGui::SetNextWindowPos(ImVec2(10, 10), ImGuiCond_FirstUseEver);
    ImGui::SetNextWindowSize(ImVec2(1004, 748), ImGuiCond_FirstUseEver);

    ImGui::Begin("Chat", nullptr,
        ImGuiWindowFlags_NoCollapse |
        ImGuiWindowFlags_NoTitleBar |
        ImGuiWindowFlags_NoResize |
        ImGuiWindowFlags_NoMove);

    ImGui::TextColored(ImVec4(0.7f, 0.9f, 1.0f, 1.0f), "maowbot %s [STUB MODE]",
        is_dashboard ? "Dashboard" : "HUD");
    ImGui::Separator();

    ImVec2 chat_size = ImVec2(0, -ImGui::GetFrameHeightWithSpacing() - 10);
    if (ImGui::BeginChild("ChatArea", chat_size, true)) {
        for (const auto& msg : g_chat_messages) {
            ImGui::TextColored(ImVec4(0.8f, 0.8f, 0.2f, 1.0f), "%s:", msg.author);
            ImGui::SameLine();
            ImGui::TextWrapped("%s", msg.text);
        }

        if (ImGui::GetScrollY() >= ImGui::GetScrollMaxY())
            ImGui::SetScrollHereY(1.0f);
    }
    ImGui::EndChild();

    ImGui::Separator();
    bool reclaim_focus = false;
    ImGuiInputTextFlags input_flags = ImGuiInputTextFlags_EnterReturnsTrue;

    bool was_focused = g_input_focused;
    ImGui::PushID("ChatInput");

    if (ImGui::InputText("##Input", g_input_buffer, sizeof(g_input_buffer), input_flags)) {
        if (strlen(g_input_buffer) > 0) {
            g_message_sent = true;
            reclaim_focus = true;
        }
    }

    g_input_focused = ImGui::IsItemActive() || ImGui::IsItemFocused();
    if (g_input_focused && !was_focused) {
        g_input_just_focused = true;
    }

    ImGui::PopID();
    ImGui::SetItemDefaultFocus();
    if (reclaim_focus)
        ImGui::SetKeyboardFocusHere(-1);

    ImGui::End();

    ImGui::EndFrame();
    
    return true;
}

// Keyboard functions
extern "C" bool vr_keyboard_init_rendering(void* device_ptr, void* context_ptr) {
    std::cout << "[STUB] Initializing keyboard rendering\n";
    
    g_keyboard_imgui_ctx = ImGui::CreateContext();
    ImGui::SetCurrentContext(g_keyboard_imgui_ctx);

    ImGuiIO& io = ImGui::GetIO();
    io.DisplaySize = ImVec2(512.0f, 384.0f);

    ImGui::StyleColorsDark();
    ImGuiStyle& style = ImGui::GetStyle();
    style.ScaleAllSizes(2.0f);
    io.FontGlobalScale = 2.0f;

    ImGui::SetCurrentContext(g_imgui_ctx);
    
    return true;
}

extern "C" bool vr_keyboard_render(VROverlayHandle_t handle,
                                  float selected_x, float selected_y,
                                  const char* current_text) {
    // In stub mode, we don't actually render
    return true;
}

