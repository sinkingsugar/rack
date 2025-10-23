#include "rack_au.h"
#include <iostream>
#include <iomanip>
#include <cstring>
#include <cmath>

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

void test_audio_processing() {
    std::cout << "Test 4: Audio processing\n";
    std::cout << "------------------------\n";

    // Scan for a plugin to test with
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

    // Find an effect plugin for processing test
    const char* unique_id = nullptr;
    const char* plugin_name = nullptr;
    for (int i = 0; i < filled; i++) {
        if (plugins[i].plugin_type == RACK_AU_TYPE_EFFECT) {
            unique_id = plugins[i].unique_id;
            plugin_name = plugins[i].name;
            break;
        }
    }

    if (!unique_id) {
        std::cout << "SKIP: No effect plugins found for processing test\n\n";
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    std::cout << "Testing audio processing with: " << plugin_name << "\n";

    // Create and initialize plugin
    RackAUPlugin* plugin = rack_au_plugin_new(unique_id);
    if (!plugin) {
        std::cerr << "FAIL: Failed to create plugin instance\n";
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    int result = rack_au_plugin_initialize(plugin, 48000.0, 512);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Failed to initialize plugin (error: " << result << ")\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    // Create test buffers (512 frames of stereo audio)
    const uint32_t frames = 512;
    float* input = new float[frames * 2];
    float* output = new float[frames * 2];

    // Fill input with a simple sine wave
    const float frequency = 440.0f; // A4
    const float sample_rate = 48000.0f;
    for (uint32_t i = 0; i < frames; i++) {
        float sample = sinf(2.0f * 3.14159265f * frequency * i / sample_rate) * 0.5f;
        input[i * 2] = sample;      // Left channel
        input[i * 2 + 1] = sample;  // Right channel
    }

    // Process audio
    result = rack_au_plugin_process(plugin, input, output, frames);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Audio processing failed (error: " << result << ")\n";
        delete[] input;
        delete[] output;
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    std::cout << "PASS: Audio processing succeeded\n";

    // Verify output is not all zeros (plugin did something)
    bool has_signal = false;
    for (uint32_t i = 0; i < frames * 2; i++) {
        if (output[i] != 0.0f) {
            has_signal = true;
            break;
        }
    }

    if (has_signal) {
        std::cout << "PASS: Output contains audio signal\n";
    } else {
        std::cout << "WARN: Output is silent (may be expected for some effects)\n";
    }

    // Cleanup
    delete[] input;
    delete[] output;
    rack_au_plugin_free(plugin);
    delete[] plugins;
    rack_au_scanner_free(scanner);

    std::cout << "\n";
}

void test_parameter_operations() {
    std::cout << "Test 5: Parameter operations\n";
    std::cout << "-----------------------------\n";

    // Scan for a plugin to test with
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

    // Find an effect plugin (more likely to have parameters)
    const char* unique_id = nullptr;
    const char* plugin_name = nullptr;
    for (int i = 0; i < filled; i++) {
        if (plugins[i].plugin_type == RACK_AU_TYPE_EFFECT) {
            unique_id = plugins[i].unique_id;
            plugin_name = plugins[i].name;
            break;
        }
    }

    if (!unique_id) {
        std::cout << "SKIP: No effect plugins found for parameter test\n\n";
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    std::cout << "Testing parameters with: " << plugin_name << "\n";

    // Create and initialize plugin
    RackAUPlugin* plugin = rack_au_plugin_new(unique_id);
    if (!plugin) {
        std::cerr << "FAIL: Failed to create plugin instance\n";
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    int result = rack_au_plugin_initialize(plugin, 48000.0, 512);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Failed to initialize plugin (error: " << result << ")\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    // Test parameter count
    int param_count = rack_au_plugin_parameter_count(plugin);
    std::cout << "  Parameter count: " << param_count << "\n";
    std::cout << "PASS: Parameter count retrieved\n";

    if (param_count == 0) {
        std::cout << "  Plugin has no parameters, skipping parameter tests\n\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    // Test parameter info
    char name[256];
    char unit[32];
    float min_val = 0.0f;
    float max_val = 0.0f;
    float default_val = 0.0f;

    result = rack_au_plugin_parameter_info(plugin, 0, name, sizeof(name), &min_val, &max_val, &default_val, unit, sizeof(unit));
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Failed to get parameter info (error: " << result << ")\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    std::cout << "  Parameter 0: " << name;
    if (unit[0] != '\0') {
        std::cout << " (" << unit << ")";
    }
    std::cout << "\n";
    std::cout << "    Range: " << min_val << " - " << max_val << "\n";
    std::cout << "    Default: " << default_val << "\n";
    std::cout << "PASS: Parameter info retrieved\n";

    // Test get parameter
    float value = 0.0f;
    result = rack_au_plugin_get_parameter(plugin, 0, &value);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Failed to get parameter (error: " << result << ")\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    std::cout << "  Current value: " << std::fixed << std::setprecision(4) << value << " (normalized)\n";
    std::cout << "PASS: Parameter value retrieved\n";

    // Test set parameter
    float original_value = value;
    result = rack_au_plugin_set_parameter(plugin, 0, 0.75f);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Failed to set parameter (error: " << result << ")\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    // Verify the value changed
    result = rack_au_plugin_get_parameter(plugin, 0, &value);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Failed to get parameter after set (error: " << result << ")\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    if (std::abs(value - 0.75f) > 0.01f) {
        std::cerr << "FAIL: Parameter value should be ~0.75, got " << value << "\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    std::cout << "  New value: " << value << " (normalized)\n";
    std::cout << "PASS: Parameter set and verified\n";

    // Restore original value
    rack_au_plugin_set_parameter(plugin, 0, original_value);

    // Test out of bounds index
    result = rack_au_plugin_get_parameter(plugin, param_count + 10, &value);
    if (result == RACK_AU_OK) {
        std::cerr << "FAIL: Should fail for out-of-bounds index\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }
    std::cout << "PASS: Out-of-bounds index rejected\n";

    // Cleanup
    rack_au_plugin_free(plugin);
    delete[] plugins;
    rack_au_scanner_free(scanner);

    std::cout << "\n";
}

void test_midi_operations() {
    std::cout << "Test 6: MIDI operations\n";
    std::cout << "------------------------\n";

    // Scan for a plugin to test with
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

    // Find an instrument plugin for MIDI testing
    const char* unique_id = nullptr;
    const char* plugin_name = nullptr;
    for (int i = 0; i < filled; i++) {
        if (plugins[i].plugin_type == RACK_AU_TYPE_INSTRUMENT) {
            unique_id = plugins[i].unique_id;
            plugin_name = plugins[i].name;
            break;
        }
    }

    if (!unique_id) {
        std::cout << "SKIP: No instrument plugins found for MIDI test\n\n";
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    std::cout << "Testing MIDI with: " << plugin_name << "\n";

    // Create and initialize plugin
    RackAUPlugin* plugin = rack_au_plugin_new(unique_id);
    if (!plugin) {
        std::cerr << "FAIL: Failed to create plugin instance\n";
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    int result = rack_au_plugin_initialize(plugin, 48000.0, 512);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Failed to initialize plugin (error: " << result << ")\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    // Test: Send a single MIDI note
    RackAUMidiEvent events[3];

    // Note On: Middle C (note 60), velocity 100, channel 0
    events[0].sample_offset = 0;
    events[0].status = RACK_AU_MIDI_NOTE_ON;
    events[0].data1 = 60;
    events[0].data2 = 100;
    events[0].channel = 0;

    // Note On: E (note 64), velocity 100, channel 0
    events[1].sample_offset = 0;
    events[1].status = RACK_AU_MIDI_NOTE_ON;
    events[1].data1 = 64;
    events[1].data2 = 100;
    events[1].channel = 0;

    // Note On: G (note 67), velocity 100, channel 0
    events[2].sample_offset = 0;
    events[2].status = RACK_AU_MIDI_NOTE_ON;
    events[2].data1 = 67;
    events[2].data2 = 100;
    events[2].channel = 0;

    result = rack_au_plugin_send_midi(plugin, events, 3);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Failed to send MIDI events (error: " << result << ")\n";
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }
    std::cout << "PASS: MIDI events sent successfully (C major chord)\n";

    // Process audio to render the notes
    const uint32_t frames = 512;
    float* input = new float[frames * 2];
    float* output = new float[frames * 2];

    // Clear buffers
    memset(input, 0, frames * 2 * sizeof(float));
    memset(output, 0, frames * 2 * sizeof(float));

    // Process audio (instrument should generate sound from MIDI)
    result = rack_au_plugin_process(plugin, input, output, frames);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Audio processing failed after MIDI (error: " << result << ")\n";
        delete[] input;
        delete[] output;
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }

    // Check if output contains audio signal (MIDI notes rendered)
    bool has_signal = false;
    for (uint32_t i = 0; i < frames * 2; i++) {
        if (output[i] != 0.0f) {
            has_signal = true;
            break;
        }
    }

    if (has_signal) {
        std::cout << "PASS: Output contains audio from MIDI notes\n";
    } else {
        std::cout << "WARN: Output is silent (plugin may need more time or different MIDI setup)\n";
    }

    // Test: Send Note Off events
    events[0].status = RACK_AU_MIDI_NOTE_OFF;
    events[1].status = RACK_AU_MIDI_NOTE_OFF;
    events[2].status = RACK_AU_MIDI_NOTE_OFF;

    result = rack_au_plugin_send_midi(plugin, events, 3);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Failed to send Note Off events (error: " << result << ")\n";
        delete[] input;
        delete[] output;
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }
    std::cout << "PASS: Note Off events sent successfully\n";

    // Test: Invalid channel (> 15)
    events[0].channel = 20;
    result = rack_au_plugin_send_midi(plugin, events, 1);
    if (result == RACK_AU_OK) {
        std::cerr << "FAIL: Should reject invalid MIDI channel\n";
        delete[] input;
        delete[] output;
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }
    std::cout << "PASS: Invalid MIDI channel rejected\n";

    // Test: Empty event array (should succeed)
    result = rack_au_plugin_send_midi(plugin, nullptr, 0);
    if (result != RACK_AU_OK) {
        std::cerr << "FAIL: Empty MIDI array should succeed\n";
        delete[] input;
        delete[] output;
        rack_au_plugin_free(plugin);
        delete[] plugins;
        rack_au_scanner_free(scanner);
        return;
    }
    std::cout << "PASS: Empty MIDI event array handled correctly\n";

    // Cleanup
    delete[] input;
    delete[] output;
    rack_au_plugin_free(plugin);
    delete[] plugins;
    rack_au_scanner_free(scanner);

    std::cout << "\n";
}

int main() {
    std::cout << "AudioUnit Plugin Instance Test\n";
    std::cout << "===============================\n\n";

    test_invalid_unique_id();
    test_plugin_lifecycle();
    test_invalid_parameters();
    test_audio_processing();
    test_parameter_operations();
    test_midi_operations();

    std::cout << "All tests completed!\n";
    return 0;
}
