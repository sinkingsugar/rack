#ifndef RACK_VST3_H
#define RACK_VST3_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>
#include <stdint.h>

// Opaque types
typedef struct RackVST3Scanner RackVST3Scanner;
typedef struct RackVST3Plugin RackVST3Plugin;
typedef struct RackVST3Gui RackVST3Gui;

// Plugin type enum
typedef enum {
    RACK_VST3_TYPE_EFFECT = 0,
    RACK_VST3_TYPE_INSTRUMENT = 1,
    RACK_VST3_TYPE_ANALYZER = 2,
    RACK_VST3_TYPE_SPATIAL = 3,
    RACK_VST3_TYPE_OTHER = 4,
} RackVST3PluginType;

// Plugin info struct (passed to Rust)
typedef struct {
    char name[256];
    char manufacturer[256];
    char path[1024];
    char unique_id[64];  // VST3 uses UID (16 bytes as hex string)
    uint32_t version;
    RackVST3PluginType plugin_type;
    char category[128];  // VST3 subcategories (e.g., "Fx|Reverb")
} RackVST3PluginInfo;

// Error codes (0 = success, negative = error)
#define RACK_VST3_OK 0
#define RACK_VST3_ERROR_GENERIC -1
#define RACK_VST3_ERROR_NOT_FOUND -2
#define RACK_VST3_ERROR_INVALID_PARAM -3
#define RACK_VST3_ERROR_NOT_INITIALIZED -4
#define RACK_VST3_ERROR_LOAD_FAILED -5

// ============================================================================
// Scanner API
// ============================================================================

// Create a new scanner
// Returns NULL if allocation fails
RackVST3Scanner* rack_vst3_scanner_new(void);

// Free scanner
void rack_vst3_scanner_free(RackVST3Scanner* scanner);

// Add a search path for VST3 plugins
// Returns 0 on success, negative error code on failure
int rack_vst3_scanner_add_path(RackVST3Scanner* scanner, const char* path);

// Add system default VST3 search paths
// Returns 0 on success, negative error code on failure
int rack_vst3_scanner_add_default_paths(RackVST3Scanner* scanner);

// Scan for plugins
// Returns number of plugins found (or would be found), or negative error code
//
// Two-pass usage pattern (recommended):
//   1. count = rack_vst3_scanner_scan(scanner, NULL, 0);  // Get total count
//   2. rack_vst3_scanner_scan(scanner, array, count);     // Fill array
//
// If plugins is NULL: Only counts plugins, does not extract details
// If plugins is not NULL: Fills array up to max_plugins
//
// IMPORTANT: Return value may exceed max_plugins if more plugins exist.
//            Compare return value with max_plugins to detect truncation.
//
// plugins: output array (allocated by caller), or NULL to get count only
// max_plugins: size of output array (ignored if plugins is NULL)
int rack_vst3_scanner_scan(RackVST3Scanner* scanner, RackVST3PluginInfo* plugins, size_t max_plugins);

// ============================================================================
// Plugin Instance API
// ============================================================================

// Create a new plugin instance from path and UID
// path: path to .vst3 bundle/folder
// uid: plugin UID (from scan result)
// Returns plugin instance or NULL on error
RackVST3Plugin* rack_vst3_plugin_new(const char* path, const char* uid);

// Free plugin instance
void rack_vst3_plugin_free(RackVST3Plugin* plugin);

// Initialize plugin
// Returns 0 on success, negative error code on failure
int rack_vst3_plugin_initialize(RackVST3Plugin* plugin, double sample_rate, uint32_t max_block_size);

// Check if plugin is initialized
int rack_vst3_plugin_is_initialized(RackVST3Plugin* plugin);

// Reset plugin state
// Clears all internal buffers, delay lines, and state without changing parameters.
// Useful for clearing reverb tails, delay lines, etc. between songs or after preset changes.
//
// Returns:
//   0 (RACK_VST3_OK) on success
//   RACK_VST3_ERROR_NOT_INITIALIZED if plugin is not initialized
//   negative error code on failure
//
// Thread-safety: Should be called from a non-realtime thread.
int rack_vst3_plugin_reset(RackVST3Plugin* plugin);

// Get input channel count
// Returns number of input channels, or 0 if not initialized or query failed
// Thread-safety: Should be called after initialize()
int rack_vst3_plugin_get_input_channels(RackVST3Plugin* plugin);

// Get output channel count
// Returns number of output channels, or 0 if not initialized or query failed
// Thread-safety: Should be called after initialize()
int rack_vst3_plugin_get_output_channels(RackVST3Plugin* plugin);

// Process audio (planar format - one buffer per channel)
// Uses planar (non-interleaved) audio format matching VST3 internal format.
// This enables zero-copy processing in effect chains.
//
// inputs: array of input channel pointers (e.g., [left_ptr, right_ptr] for stereo)
// num_input_channels: number of input channels
// outputs: array of output channel pointers (e.g., [left_ptr, right_ptr] for stereo)
// num_output_channels: number of output channels
// frames: number of frames to process
//
// Channel Layout Examples:
//   Mono:   inputs = [mono_ptr], num_input_channels = 1
//   Stereo: inputs = [left_ptr, right_ptr], num_input_channels = 2
//   5.1:    inputs = [L, R, C, LFE, SL, SR], num_input_channels = 6
//
// Returns 0 on success, negative error code on failure
int rack_vst3_plugin_process(
    RackVST3Plugin* plugin,
    const float* const* inputs,
    uint32_t num_input_channels,
    float* const* outputs,
    uint32_t num_output_channels,
    uint32_t frames
);

// Get parameter count
// Thread-safety: Read-only after initialization. Safe to call from any thread.
int rack_vst3_plugin_parameter_count(RackVST3Plugin* plugin);

// Get parameter value (normalized 0.0 to 1.0)
// Returns 0 on success, negative error code on failure
// Thread-safety: Can be called from any thread, but the same plugin instance
// must not be accessed concurrently.
int rack_vst3_plugin_get_parameter(RackVST3Plugin* plugin, uint32_t index, float* value);

// Set parameter value (normalized 0.0 to 1.0)
// Returns 0 on success, negative error code on failure
// Thread-safety: Can be called from any thread, but the same plugin instance
// must not be accessed concurrently.
// Note: Calling during audio processing may cause clicks/pops.
int rack_vst3_plugin_set_parameter(RackVST3Plugin* plugin, uint32_t index, float value);

// Get parameter info
// name: output buffer for parameter name (allocated by caller)
// name_size: size of name buffer
// unit: output buffer for parameter unit string (allocated by caller, can be NULL)
// unit_size: size of unit buffer (ignored if unit is NULL)
// Returns 0 on success, negative error code on failure
int rack_vst3_plugin_parameter_info(
    RackVST3Plugin* plugin,
    uint32_t index,
    char* name,
    size_t name_size,
    float* min,
    float* max,
    float* default_value,
    char* unit,
    size_t unit_size
);

// ============================================================================
// Preset Management API
// ============================================================================

// Preset info struct
typedef struct {
    char name[256];
    int32_t preset_number;
} RackVST3PresetInfo;

// Get factory preset count
// Returns number of factory presets, or 0 if plugin has no presets
// Thread-safety: Read-only after initialization. Safe to call from any thread.
int rack_vst3_plugin_get_preset_count(RackVST3Plugin* plugin);

// Get preset info by index
// index: preset index (0 to preset_count - 1)
// name: output buffer for preset name (allocated by caller)
// name_size: size of name buffer
// preset_number: output parameter for preset number (used with load_preset)
// Returns 0 on success, negative error code on failure
int rack_vst3_plugin_get_preset_info(
    RackVST3Plugin* plugin,
    uint32_t index,
    char* name,
    size_t name_size,
    int32_t* preset_number
);

// Load a factory preset by preset number
// preset_number: the preset number from get_preset_info()
// Returns 0 on success, negative error code on failure
// Thread-safety: Should be called from the same thread that owns the plugin instance.
int rack_vst3_plugin_load_preset(RackVST3Plugin* plugin, int32_t preset_number);

// Get plugin state size (for allocation)
// Returns size in bytes needed to store state, or 0 if state cannot be retrieved
// Thread-safety: Read-only after initialization. Safe to call from any thread.
int rack_vst3_plugin_get_state_size(RackVST3Plugin* plugin);

// Get plugin state (full state including parameters, preset, etc.)
// data: output buffer for state data (allocated by caller)
// size: input/output - buffer size on input, actual size on output
// Returns 0 on success, negative error code on failure
// Thread-safety: Should be called from the same thread that owns the plugin instance.
int rack_vst3_plugin_get_state(RackVST3Plugin* plugin, uint8_t* data, size_t* size);

// Set plugin state (restore full state including parameters, preset, etc.)
// data: state data (from previous get_state call)
// size: size of state data in bytes
// Returns 0 on success, negative error code on failure
// Thread-safety: Should be called from the same thread that owns the plugin instance.
int rack_vst3_plugin_set_state(RackVST3Plugin* plugin, const uint8_t* data, size_t size);

// ============================================================================
// MIDI API
// ============================================================================

// MIDI event types (matches VST3 event types)
typedef enum {
    RACK_VST3_MIDI_NOTE_ON = 0x90,
    RACK_VST3_MIDI_NOTE_OFF = 0x80,
    RACK_VST3_MIDI_POLYPHONIC_AFTERTOUCH = 0xA0,
    RACK_VST3_MIDI_CONTROL_CHANGE = 0xB0,
    RACK_VST3_MIDI_PROGRAM_CHANGE = 0xC0,
    RACK_VST3_MIDI_CHANNEL_AFTERTOUCH = 0xD0,
    RACK_VST3_MIDI_PITCH_BEND = 0xE0,
} RackVST3MidiEventType;

// MIDI event struct
typedef struct {
    uint32_t sample_offset;  // Sample offset within buffer
    uint8_t status;          // MIDI status byte
    uint8_t data1;           // First data byte (note/CC number)
    uint8_t data2;           // Second data byte (velocity/value)
    uint8_t channel;         // MIDI channel (0-15)
} RackVST3MidiEvent;

// Send MIDI events to plugin
// events: array of MIDI events
// event_count: number of events in array
// Returns 0 on success, negative error code on failure
// Thread-safety: Should be called from the same thread that owns the plugin instance.
// Not safe to call concurrently with process() or other plugin operations.
int rack_vst3_plugin_send_midi(
    RackVST3Plugin* plugin,
    const RackVST3MidiEvent* events,
    uint32_t event_count
);

#ifdef __cplusplus
}
#endif

#endif // RACK_VST3_H
