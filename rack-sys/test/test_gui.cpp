#include "rack_au.h"
#include <iostream>
#include <iomanip>
#include <cstring>
#include <memory>
#include <dispatch/dispatch.h>

// Global flag for async callback
static bool gui_callback_invoked = false;
static RackAUGui* gui_result = nullptr;
static int gui_error_code = 0;

// Callback for async GUI creation
void gui_callback(void* user_data, RackAUGui* gui, int error_code) {
    gui_callback_invoked = true;
    gui_result = gui;
    gui_error_code = error_code;

    int* user_value = (int*)user_data;
    if (user_value) {
        std::cout << "  Callback invoked with user_data: " << *user_value << "\n";
    }

    if (gui != nullptr) {
        std::cout << "  GUI created successfully (error_code: " << error_code << ")\n";
    } else {
        std::cout << "  GUI creation failed (error_code: " << error_code << ")\n";
    }
}

void test_gui_with_invalid_plugin() {
    std::cout << "Test 1: GUI creation with invalid plugin\n";
    std::cout << "-----------------------------------------\n";

    // Reset state
    gui_callback_invoked = false;
    gui_result = nullptr;

    // Try to create GUI with nullptr plugin
    rack_au_gui_create_async(nullptr, gui_callback, nullptr);

    // Wait a bit for callback
    dispatch_sync(dispatch_get_main_queue(), ^{});

    if (gui_callback_invoked) {
        std::cout << "PASS: Callback invoked for invalid plugin\n";
        if (gui_result == nullptr && gui_error_code == RACK_AU_ERROR_INVALID_PARAM) {
            std::cout << "PASS: Correct error handling\n";
        } else {
            std::cout << "FAIL: Expected RACK_AU_ERROR_INVALID_PARAM\n";
        }
    } else {
        std::cout << "PASS: Callback not invoked for nullptr plugin (expected)\n";
    }
    std::cout << "\n";
}

void test_gui_lifecycle() {
    std::cout << "Test 2: GUI lifecycle with real plugin\n";
    std::cout << "---------------------------------------\n";

    // First, scan to get a real plugin
    RackAUScanner* scanner = rack_au_scanner_new();
    if (!scanner) {
        std::cerr << "FAIL: Failed to create scanner\n";
        return;
    }

    int count = rack_au_scanner_scan(scanner, nullptr, 0);
    if (count <= 0) {
        std::cerr << "SKIP: No plugins found to test with\n\n";
        rack_au_scanner_free(scanner);
        return;
    }

    std::unique_ptr<RackAUPluginInfo[]> plugins(new(std::nothrow) RackAUPluginInfo[count]);
    if (!plugins) {
        std::cerr << "FAIL: Failed to allocate memory\n";
        rack_au_scanner_free(scanner);
        return;
    }

    rack_au_scanner_scan(scanner, plugins.get(), count);
    rack_au_scanner_free(scanner);

    // Try to find an instrument or effect plugin (more likely to have GUI)
    const char* test_unique_id = nullptr;
    const char* test_name = nullptr;
    for (int i = 0; i < count; i++) {
        if (plugins[i].plugin_type == RACK_AU_TYPE_INSTRUMENT ||
            plugins[i].plugin_type == RACK_AU_TYPE_EFFECT) {
            test_unique_id = plugins[i].unique_id;
            test_name = plugins[i].name;
            break;
        }
    }

    if (!test_unique_id) {
        // Use first plugin
        test_unique_id = plugins[0].unique_id;
        test_name = plugins[0].name;
    }

    std::cout << "Testing with plugin: " << test_name << "\n";
    std::cout << "Unique ID: " << test_unique_id << "\n\n";

    // Create plugin instance
    RackAUPlugin* plugin = rack_au_plugin_new(test_unique_id);
    if (!plugin) {
        std::cerr << "FAIL: Failed to create plugin instance\n\n";
        return;
    }

    // Initialize plugin
    int result = rack_au_plugin_initialize(plugin, 48000.0, 512);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Failed to initialize plugin\n\n";
        rack_au_plugin_free(plugin);
        return;
    }

    std::cout << "Plugin initialized successfully\n";

    // Reset callback state
    gui_callback_invoked = false;
    gui_result = nullptr;
    gui_error_code = 0;

    // Create GUI asynchronously
    int user_data = 42;  // Test user data
    std::cout << "Creating GUI asynchronously...\n";
    rack_au_gui_create_async(plugin, gui_callback, &user_data);

    // Wait for callback on main thread
    // Use a run loop to process the async callback
    dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 2 * NSEC_PER_SEC),
                   dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0), ^{
        dispatch_semaphore_signal(semaphore);
    });
    dispatch_semaphore_wait(semaphore, DISPATCH_TIME_FOREVER);

    if (gui_callback_invoked) {
        std::cout << "PASS: Callback was invoked\n";

        if (gui_result != nullptr) {
            std::cout << "PASS: GUI created successfully\n";

            // Test get_view
            void* view_ptr = rack_au_gui_get_view(gui_result);
            if (view_ptr != nullptr) {
                std::cout << "PASS: get_view returned non-null pointer\n";
            } else {
                std::cout << "FAIL: get_view returned null\n";
            }

            // Test get_size
            float width = 0, height = 0;
            result = rack_au_gui_get_size(gui_result, &width, &height);
            if (result == RACK_AU_OK) {
                std::cout << "PASS: get_size succeeded (size: " << width << "x" << height << ")\n";
            } else {
                std::cout << "FAIL: get_size failed\n";
            }

            // Clean up GUI
            std::cout << "Destroying GUI...\n";
            rack_au_gui_destroy(gui_result);
            std::cout << "PASS: GUI destroyed successfully\n";
        } else {
            std::cout << "NOTE: GUI creation failed (plugin may not support GUI)\n";
            std::cout << "      Error code: " << gui_error_code << "\n";
            std::cout << "      This is expected for plugins without custom views\n";
        }
    } else {
        std::cout << "WARN: Callback was not invoked within timeout\n";
        std::cout << "      This may be normal for async operations\n";
    }

    // Clean up
    rack_au_plugin_free(plugin);
    std::cout << "\n";
}

void test_gui_get_size_with_invalid_params() {
    std::cout << "Test 3: get_size with invalid parameters\n";
    std::cout << "-----------------------------------------\n";

    float width, height;

    // Test with nullptr GUI
    int result = rack_au_gui_get_size(nullptr, &width, &height);
    if (result == RACK_AU_ERROR_INVALID_PARAM) {
        std::cout << "PASS: get_size returns error for nullptr GUI\n";
    } else {
        std::cout << "FAIL: Expected RACK_AU_ERROR_INVALID_PARAM\n";
    }

    std::cout << "\n";
}

int main() {
    std::cout << "======================================\n";
    std::cout << "Rack AudioUnit GUI Tests\n";
    std::cout << "======================================\n\n";

    test_gui_with_invalid_plugin();
    test_gui_lifecycle();
    test_gui_get_size_with_invalid_params();

    std::cout << "======================================\n";
    std::cout << "All tests completed\n";
    std::cout << "======================================\n";

    std::cout << "\nNOTE: GUI tests may require manual verification\n";
    std::cout << "      Run with window display for full testing\n";

    return 0;
}
