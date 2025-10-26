#include "rack_au.h"

// This file contains platform-specific implementations
// Scanner and instance implementations are in au_scanner.cpp and au_instance.cpp
// GUI implementations are in au_gui.mm (macOS only)

#if TARGET_OS_IPHONE || TARGET_OS_VISION
// iOS/visionOS: GUI provided by app extensions - provide stub implementations

extern "C" {

void rack_au_gui_create_async(
    RackAUPlugin* plugin,
    RackAUGuiCallback callback,
    void* user_data
) {
    // GUI is provided by app extensions on iOS/visionOS
    if (callback) {
        callback(user_data, nullptr, RACK_AU_ERROR_GENERIC);
    }
}

void rack_au_gui_destroy(RackAUGui* gui) {
    // No-op on iOS/visionOS
    (void)gui;
}

void* rack_au_gui_get_view(RackAUGui* gui) {
    (void)gui;
    return nullptr;
}

int rack_au_gui_get_size(RackAUGui* gui, float* width, float* height) {
    (void)gui;
    (void)width;
    (void)height;
    return RACK_AU_ERROR_GENERIC;
}

int rack_au_gui_show_window(RackAUGui* gui, const char* title) {
    (void)gui;
    (void)title;
    return RACK_AU_ERROR_GENERIC;
}

int rack_au_gui_hide_window(RackAUGui* gui) {
    (void)gui;
    return RACK_AU_ERROR_GENERIC;
}

} // extern "C"

#endif // TARGET_OS_IPHONE || TARGET_OS_VISION
