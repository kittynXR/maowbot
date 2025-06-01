#include <cstring>
#include <cfloat>
#include <cstdint>
#include <vector>
#include <string>
#include <iostream>
#include <chrono>
#include <thread>

// Stub ImGui types to avoid requiring actual ImGui headers
namespace ImGui {
    struct ImVec2 { float x, y; ImVec2(float _x = 0.0f, float _y = 0.0f) : x(_x), y(_y) {} };
    struct ImVec4 { float x, y, z, w; ImVec4(float _x = 0.0f, float _y = 0.0f, float _z = 0.0f, float _w = 0.0f) : x(_x), y(_y), z(_z), w(_w) {} };
    struct ImGuiContext {};
    struct ImGuiIO {
        ImVec2 DisplaySize;
        ImVec2 MousePos;
        bool MouseDown[5];
        int ConfigFlags;
        bool ConfigInputTextCursorBlink;
        float FontGlobalScale;
    };
    struct ImGuiStyle {
        float WindowRounding;
        float FrameRounding;
        float ScrollbarRounding;
        float GrabRounding;
        float WindowBorderSize;
        void ScaleAllSizes(float scale_factor) {}
    };
    
    enum ImGuiConfigFlags_ {
        ImGuiConfigFlags_NavEnableKeyboard = 1 << 0
    };
    
    enum ImGuiWindowFlags_ {
        ImGuiWindowFlags_None = 0,
        ImGuiWindowFlags_NoTitleBar = 1 << 0,
        ImGuiWindowFlags_NoResize = 1 << 1,
        ImGuiWindowFlags_NoMove = 1 << 2,
        ImGuiWindowFlags_NoCollapse = 1 << 4
    };
    
    enum ImGuiCond_ {
        ImGuiCond_FirstUseEver = 1 << 2
    };
    
    enum ImGuiInputTextFlags_ {
        ImGuiInputTextFlags_None = 0,
        ImGuiInputTextFlags_EnterReturnsTrue = 1 << 5
    };
    
    // Stub ImGui functions
    static ImGuiContext* CreateContext() { return new ImGuiContext(); }
    static void DestroyContext(ImGuiContext* ctx) { delete ctx; }
    static void SetCurrentContext(ImGuiContext* ctx) {}
    static ImGuiIO& GetIO() { static ImGuiIO io; return io; }
    static ImGuiStyle& GetStyle() { static ImGuiStyle style; return style; }
    static void StyleColorsDark() {}
    static void NewFrame() {}
    static void EndFrame() {}
    static void SetNextWindowPos(const ImVec2& pos, int cond = 0) {}
    static void SetNextWindowSize(const ImVec2& size, int cond = 0) {}
    static bool Begin(const char* name, bool* p_open = nullptr, int flags = 0) { return true; }
    static void End() {}
    static void Text(const char* fmt, ...) {}
    static void TextColored(const ImVec4& col, const char* fmt, ...) {}
    static void TextWrapped(const char* fmt, ...) {}
    static bool BeginChild(const char* str_id, const ImVec2& size = ImVec2(0,0), bool border = false) { return true; }
    static void EndChild() {}
    static void Separator() {}
    static void SameLine(float offset_from_start_x = 0.0f) {}
    static bool Button(const char* label) { return false; }
    static bool Checkbox(const char* label, bool* v) { return false; }
    static bool SliderFloat(const char* label, float* v, float v_min, float v_max) { return false; }
    static bool DragFloat(const char* label, float* v, float v_speed = 1.0f, float v_min = 0.0f, float v_max = 0.0f) { return false; }
    static bool InputText(const char* label, char* buf, size_t buf_size, int flags = 0) { return false; }
    static bool BeginTabBar(const char* str_id) { return true; }
    static void EndTabBar() {}
    static bool BeginTabItem(const char* label) { static int tab = 0; tab++; return (tab % 4) == 1; }
    static void EndTabItem() {}
    static float GetScrollY() { return 0.0f; }
    static float GetScrollMaxY() { return 0.0f; }
    static void SetScrollHereY(float center_y_ratio = 0.5f) {}
    static float GetFrameHeightWithSpacing() { return 20.0f; }
    static void PushID(const char* str_id) {}
    static void PopID() {}
    static bool IsItemActive() { return false; }
    static bool IsItemFocused() { return false; }
    static void SetItemDefaultFocus() {}
    static void SetKeyboardFocusHere(int offset = 0) {}
    static float GetWindowWidth() { return 400.0f; }
}

#define IMGUI_CHECKVERSION()

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

// Dashboard State types (matching the real implementation)
struct OverlaySettingsFFI {
    bool show_chat;
    float chat_opacity;
    float chat_position_x;
    float chat_position_y;
    float chat_width;
    float chat_height;
    bool show_alerts;
    float alert_duration;
};

struct DashboardState {
    bool show_settings;
    int current_tab;
};

// Global state for stub
static VROverlayHandle_t g_handle = 1;  // Non-zero to indicate success
static VROverlayHandle_t g_keyboard_handle = 2;
static ImGui::ImGuiContext* g_imgui_ctx = nullptr;
static ImGui::ImGuiContext* g_keyboard_imgui_ctx = nullptr;

// Dashboard state
static OverlaySettingsFFI g_overlay_settings = {
    true,   // show_chat
    0.8f,   // chat_opacity
    10.0f,  // chat_position_x
    10.0f,  // chat_position_y
    400.0f, // chat_width
    600.0f, // chat_height
    true,   // show_alerts
    5.0f    // alert_duration
};

static DashboardState g_dashboard_state = {false, 0};
static bool g_dashboard_state_changed = false;

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

// Forward declarations
static void render_settings_window();

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

    ImGui::ImGuiIO& io = ImGui::GetIO();
    io.DisplaySize = ImGui::ImVec2(1024.0f, 768.0f);
    io.ConfigFlags |= ImGui::ImGuiConfigFlags_NavEnableKeyboard;
    io.ConfigInputTextCursorBlink = false;

    ImGui::StyleColorsDark();

    ImGui::ImGuiStyle& style = ImGui::GetStyle();
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

static void render_chat_window(bool is_dashboard) {
    ImGui::SetNextWindowPos(ImGui::ImVec2(10, 10), ImGui::ImGuiCond_FirstUseEver);
    ImGui::SetNextWindowSize(ImGui::ImVec2(400, 748), ImGui::ImGuiCond_FirstUseEver);

    ImGui::Begin("Chat", nullptr,
        ImGui::ImGuiWindowFlags_NoCollapse |
        ImGui::ImGuiWindowFlags_NoTitleBar |
        ImGui::ImGuiWindowFlags_NoResize |
        ImGui::ImGuiWindowFlags_NoMove);

    // Title
    ImGui::TextColored(ImGui::ImVec4(0.7f, 0.9f, 1.0f, 1.0f), "maowbot %s [STUB MODE]",
        is_dashboard ? "Dashboard" : "HUD");
    
    // Show settings button in dashboard mode
    if (is_dashboard) {
        ImGui::SameLine(ImGui::GetWindowWidth() - 100);
        if (ImGui::Button("Settings")) {
            g_dashboard_state.show_settings = !g_dashboard_state.show_settings;
            g_dashboard_state_changed = true;
            std::cout << "[STUB] Settings toggled: " << g_dashboard_state.show_settings << "\n";
        }
    }
    
    ImGui::Separator();

    // Chat area
    ImGui::ImVec2 chat_size = ImGui::ImVec2(0, -ImGui::GetFrameHeightWithSpacing() - 10);
    if (ImGui::BeginChild("ChatArea", chat_size, true)) {
        for (const auto& msg : g_chat_messages) {
            ImGui::TextColored(ImGui::ImVec4(0.8f, 0.8f, 0.2f, 1.0f), "%s:", msg.author);
            ImGui::SameLine();
            ImGui::TextWrapped("%s", msg.text);
        }

        if (ImGui::GetScrollY() >= ImGui::GetScrollMaxY())
            ImGui::SetScrollHereY(1.0f);
    }
    ImGui::EndChild();

    // Input area
    ImGui::Separator();
    bool reclaim_focus = false;
    ImGui::ImGuiInputTextFlags_ input_flags = ImGui::ImGuiInputTextFlags_EnterReturnsTrue;

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
}

extern "C" bool imgui_render_and_submit(uint32_t width, uint32_t height, bool is_dashboard) {
    // In stub mode, we just update ImGui state without actual rendering
    ImGui::ImGuiIO& io = ImGui::GetIO();
    io.MousePos = ImGui::ImVec2(g_mouse_x, g_mouse_y);
    io.MouseDown[0] = g_mouse_down;

    ImGui::NewFrame();

    // Render chat window
    render_chat_window(is_dashboard);

    // Render settings window in dashboard mode if requested
    if (is_dashboard && g_dashboard_state.show_settings) {
        render_settings_window();
    }

    ImGui::EndFrame();
    
    return true;
}

// Keyboard functions
extern "C" bool vr_keyboard_init_rendering(void* device_ptr, void* context_ptr) {
    std::cout << "[STUB] Initializing keyboard rendering\n";
    
    g_keyboard_imgui_ctx = ImGui::CreateContext();
    ImGui::SetCurrentContext(g_keyboard_imgui_ctx);

    ImGui::ImGuiIO& io = ImGui::GetIO();
    io.DisplaySize = ImGui::ImVec2(512.0f, 384.0f);

    ImGui::StyleColorsDark();
    ImGui::ImGuiStyle& style = ImGui::GetStyle();
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

// Settings window rendering function
static void render_settings_window() {
    ImGui::ImGuiWindowFlags_ window_flags = ImGui::ImGuiWindowFlags_NoCollapse;
    
    ImGui::SetNextWindowPos(ImGui::ImVec2(430, 10), ImGui::ImGuiCond_FirstUseEver);
    ImGui::SetNextWindowSize(ImGui::ImVec2(580, 750), ImGui::ImGuiCond_FirstUseEver);
    
    if (!ImGui::Begin("Settings", &g_dashboard_state.show_settings, window_flags)) {
        ImGui::End();
        return;
    }
    
    // Tab bar for different settings sections
    if (ImGui::BeginTabBar("SettingsTabs")) {
        if (ImGui::BeginTabItem("Overlay")) {
            ImGui::Text("Overlay Settings");
            ImGui::Separator();
            
            // Chat settings
            ImGui::Checkbox("Show Chat", &g_overlay_settings.show_chat);
            
            ImGui::SliderFloat("Chat Opacity", &g_overlay_settings.chat_opacity, 0.0f, 1.0f);
            
            ImGui::DragFloat("Chat X Position", &g_overlay_settings.chat_position_x, 1.0f);
            ImGui::DragFloat("Chat Y Position", &g_overlay_settings.chat_position_y, 1.0f);
            
            ImGui::DragFloat("Chat Width", &g_overlay_settings.chat_width, 1.0f, 100.0f, 800.0f);
            ImGui::DragFloat("Chat Height", &g_overlay_settings.chat_height, 1.0f, 100.0f, 1000.0f);
            
            ImGui::Separator();
            
            // Alert settings
            ImGui::Checkbox("Show Alerts", &g_overlay_settings.show_alerts);
            ImGui::SliderFloat("Alert Duration", &g_overlay_settings.alert_duration, 1.0f, 30.0f);
            
            if (ImGui::Button("Apply Settings")) {
                g_dashboard_state_changed = true;
                std::cout << "[STUB] Settings applied\n";
            }
            
            ImGui::EndTabItem();
        }
        
        if (ImGui::BeginTabItem("Audio")) {
            ImGui::Text("Audio settings would go here");
            ImGui::Text("(Stub implementation - no actual audio)");
            ImGui::EndTabItem();
        }
        
        if (ImGui::BeginTabItem("Platforms")) {
            ImGui::Text("Platform settings would go here");
            ImGui::Text("(Stub implementation - no actual platforms)");
            ImGui::EndTabItem();
        }
        
        if (ImGui::BeginTabItem("Debug")) {
            ImGui::Text("Debug Information");
            ImGui::Separator();
            ImGui::Text("Running in STUB mode");
            ImGui::Text("Mouse Position: %.1f, %.1f", g_mouse_x, g_mouse_y);
            ImGui::Text("Mouse Down: %s", g_mouse_down ? "Yes" : "No");
            ImGui::Text("Messages in chat: %zu", g_chat_messages.size());
            ImGui::EndTabItem();
        }
        
        ImGui::EndTabBar();
    }
    
    ImGui::End();
}

// Dashboard State Functions
extern "C" void imgui_update_dashboard_state(const DashboardState* state) {
    if (state) {
        g_dashboard_state = *state;
        g_dashboard_state_changed = true;
        std::cout << "[STUB] Dashboard state updated - show_settings: " 
                  << state->show_settings << ", tab: " << state->current_tab << "\n";
    }
}

extern "C" void imgui_update_overlay_settings(const OverlaySettingsFFI* settings) {
    if (settings) {
        g_overlay_settings = *settings;
        std::cout << "[STUB] Overlay settings updated - show_chat: " 
                  << settings->show_chat << ", opacity: " << settings->chat_opacity << "\n";
    }
}

extern "C" bool imgui_get_dashboard_state(DashboardState* state) {
    if (state && g_dashboard_state_changed) {
        *state = g_dashboard_state;
        g_dashboard_state_changed = false;
        return true;
    }
    return false;
}

// Additional overlay functions
extern "C" bool vr_create_overlays() {
    std::cout << "[STUB] Creating overlays\n";
    return true;
}

extern "C" bool imgui_render_hud(uint32_t width, uint32_t height) {
    std::cout << "[STUB] Rendering HUD - " << width << "x" << height << "\n";
    
    // In stub mode, just update state without actual rendering
    ImGui::SetCurrentContext(g_imgui_ctx);
    ImGui::ImGuiIO& io = ImGui::GetIO();
    io.DisplaySize = ImGui::ImVec2((float)width, (float)height);
    
    ImGui::NewFrame();
    render_chat_window(false);  // false = HUD mode
    ImGui::EndFrame();
    
    return true;
}

extern "C" bool imgui_render_dashboard(uint32_t width, uint32_t height) {
    std::cout << "[STUB] Rendering Dashboard - " << width << "x" << height << "\n";
    
    // In stub mode, just update state without actual rendering
    ImGui::SetCurrentContext(g_imgui_ctx);
    ImGui::ImGuiIO& io = ImGui::GetIO();
    io.DisplaySize = ImGui::ImVec2((float)width, (float)height);
    
    ImGui::NewFrame();
    render_chat_window(true);  // true = Dashboard mode
    
    // Render settings window if it's open
    if (g_dashboard_state.show_settings) {
        render_settings_window();
    }
    
    ImGui::EndFrame();
    
    return true;
}
