#include "rack_au.h"
#include <iostream>
#include <iomanip>
#include <cstring>

void test_invalid_unique_id() {
    std::cout << "Test 1: Invalid unique_id handling\n";
    std::cout << "-----------------------------------\n";

    // Test with nullptr
    RackAUPlugin* plugin = rack_au_plugin_new(nullptr);
    if (plugin != nullptr) {
        std::cerr << "FAIL: Expected nullptr for NULL unique_id, got valid pointer\n";
        rack_au_plugin_free(plugin);
        return;
    }
    std::cout << "PASS: nullptr for NULL unique_id\n";

    // Test with invalid format
    plugin = rack_au_plugin_new("invalid-format");
    if (plugin != nullptr) {
        std::cerr << "FAIL: Expected nullptr for invalid unique_id format\n";
        rack_au_plugin_free(plugin);
        return;
    }
    std::cout << "PASS: nullptr for invalid format\n";

    // Test with non-existent plugin
    plugin = rack_au_plugin_new("ffffffff-ffffffff-ffffffff");
    if (plugin != nullptr) {
        std::cerr << "FAIL: Expected nullptr for non-existent plugin\n";
        rack_au_plugin_free(plugin);
        return;
    }
    std::cout << "PASS: nullptr for non-existent plugin\n\n";
}

void test_plugin_lifecycle() {
    std::cout << "Test 2: Plugin lifecycle\n";
    std::cout << "------------------------\n";

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

    RackAUPluginInfo* plugins = new(std::nothrow) RackAUPluginInfo[count];
    if (!plugins) {
        std::cerr << "FAIL: Failed to allocate memory\n";
        rack_au_scanner_free(scanner);
        return;
    }

    int filled = rack_au_scanner_scan(scanner, plugins, count);
    if (filled <= 0) {
        std::cerr << "FAIL: Failed to scan plugins\n";
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    // Try to find an effect or instrument plugin (not a format converter)
    const char* unique_id = nullptr;
    const char* plugin_name = nullptr;
    for (int i = 0; i < filled; i++) {
        if (plugins[i].plugin_type == RACK_AU_TYPE_EFFECT ||
            plugins[i].plugin_type == RACK_AU_TYPE_INSTRUMENT) {
            unique_id = plugins[i].unique_id;
            plugin_name = plugins[i].name;
            break;
        }
    }

    if (!unique_id) {
        // Fall back to first plugin if no effect/instrument found
        unique_id = plugins[0].unique_id;
        plugin_name = plugins[0].name;
    }

    std::cout << "Loading plugin: " << plugin_name << "\n";
    std::cout << "Unique ID: " << unique_id << "\n";

    RackAUPlugin* plugin = rack_au_plugin_new(unique_id);
    if (!plugin) {
        std::cerr << "FAIL: Failed to create plugin instance\n";
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }
    std::cout << "PASS: Plugin instance created\n";

    // Check that it's not initialized yet
    int is_init = rack_au_plugin_is_initialized(plugin);
    if (is_init != 0) {
        std::cerr << "FAIL: Plugin should not be initialized yet\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }
    std::cout << "PASS: Plugin not initialized initially\n";

    // Initialize the plugin
    double sample_rate = 48000.0;
    uint32_t max_block_size = 512;
    int result = rack_au_plugin_initialize(plugin, sample_rate, max_block_size);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Failed to initialize plugin (error: " << result << ")\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }
    std::cout << "PASS: Plugin initialized successfully\n";

    // Check that it's now initialized
    is_init = rack_au_plugin_is_initialized(plugin);
    if (is_init != 1) {
        std::cerr << "FAIL: Plugin should be initialized now\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }
    std::cout << "PASS: Plugin is initialized\n";

    // Try to initialize again (should succeed immediately)
    result = rack_au_plugin_initialize(plugin, sample_rate, max_block_size);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Re-initialization should succeed\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }
    std::cout << "PASS: Re-initialization succeeded\n";

    // Cleanup
    rack_au_plugin_free(plugin);
    std::cout << "PASS: Plugin cleaned up\n";

    // Test that free with nullptr is safe
    rack_au_plugin_free(nullptr);
    std::cout << "PASS: free(nullptr) is safe\n";

    delete[] plugins;
    rack_au_scanner_free(scanner);
    std::cout << "\n";
}

void test_invalid_parameters() {
    std::cout << "Test 3: Invalid parameter handling\n";
    std::cout << "-----------------------------------\n";

    // Test initialize with nullptr
    int result = rack_au_plugin_initialize(nullptr, 48000.0, 512);
    if (result != RACK_AU_ERROR_INVALID_PARAM) {
        std::cerr << "FAIL: Expected INVALID_PARAM for nullptr plugin\n";
        return;
    }
    std::cout << "PASS: initialize(nullptr) returns INVALID_PARAM\n";

    // Test is_initialized with nullptr
    int is_init = rack_au_plugin_is_initialized(nullptr);
    if (is_init != 0) {
        std::cerr << "FAIL: is_initialized(nullptr) should return 0\n";
        return;
    }
    std::cout << "PASS: is_initialized(nullptr) returns 0\n\n";
}

int main() {
    std::cout << "AudioUnit Plugin Instance Test\n";
    std::cout << "===============================\n\n";

    test_invalid_unique_id();
    test_plugin_lifecycle();
    test_invalid_parameters();

    std::cout << "All tests completed!\n";
    return 0;
}
