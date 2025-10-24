#ifndef RACK_AU_H
#define RACK_AU_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>
#include <stdint.h>

// Opaque types
typedef struct RackAUScanner RackAUScanner;
typedef struct RackAUPlugin RackAUPlugin;
typedef struct RackAUGui RackAUGui;

// Plugin type enum
typedef enum {
    RACK_AU_TYPE_EFFECT = 0,
    RACK_AU_TYPE_INSTRUMENT = 1,
    RACK_AU_TYPE_MIXER = 2,
    RACK_AU_TYPE_FORMAT_CONVERTER = 3,
    RACK_AU_TYPE_OTHER = 4,
} RackAUPluginType;

// Plugin info struct (passed to Rust)
typedef struct {
    char name[256];
    char manufacturer[256];
    char path[1024];
    char unique_id[64];
    uint32_t version;
    RackAUPluginType plugin_type;
} RackAUPluginInfo;

// Error codes (0 = success, negative = error)
#define RACK_AU_OK 0
#define RACK_AU_ERROR_GENERIC -1
#define RACK_AU_ERROR_NOT_FOUND -2
#define RACK_AU_ERROR_INVALID_PARAM -3
#define RACK_AU_ERROR_NOT_INITIALIZED -4
#define RACK_AU_ERROR_AUDIO_UNIT -1000  // Base for AudioUnit OSStatus errors

// ============================================================================
// Scanner API
// ============================================================================

// Create a new scanner
// Returns NULL if allocation fails
RackAUScanner* rack_au_scanner_new(void);

// Free scanner
void rack_au_scanner_free(RackAUScanner* scanner);

// Scan for plugins
// Returns number of plugins found (or would be found), or negative error code
//
// Two-pass usage pattern (recommended):
//   1. count = rack_au_scanner_scan(scanner, NULL, 0);  // Get total count
//   2. rack_au_scanner_scan(scanner, array, count);     // Fill array
//
// If plugins is NULL: Only counts plugins, does not extract details
// If plugins is not NULL: Fills array up to max_plugins
//
// IMPORTANT: Return value may exceed max_plugins if more plugins exist.
//            Compare return value with max_plugins to detect truncation.
//
// plugins: output array (allocated by caller), or NULL to get count only
// max_plugins: size of output array (ignored if plugins is NULL)
int rack_au_scanner_scan(RackAUScanner* scanner, RackAUPluginInfo* plugins, size_t max_plugins);

// ============================================================================
// Plugin Instance API
// ============================================================================

// Create a new plugin instance from unique_id
RackAUPlugin* rack_au_plugin_new(const char* unique_id);

// Free plugin instance
void rack_au_plugin_free(RackAUPlugin* plugin);

// Initialize plugin
// Returns 0 on success, negative error code on failure
int rack_au_plugin_initialize(RackAUPlugin* plugin, double sample_rate, uint32_t max_block_size);

// Check if plugin is initialized
int rack_au_plugin_is_initialized(RackAUPlugin* plugin);

// Get input channel count
// Returns number of input channels, or 0 if not initialized or query failed
// Thread-safety: Should be called after initialize()
int rack_au_plugin_get_input_channels(RackAUPlugin* plugin);

// Get output channel count
// Returns number of output channels, or 0 if not initialized or query failed
// Thread-safety: Should be called after initialize()
int rack_au_plugin_get_output_channels(RackAUPlugin* plugin);

// Process audio (planar format - one buffer per channel)
// Uses planar (non-interleaved) audio format matching AudioUnit internal format.
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
int rack_au_plugin_process(
    RackAUPlugin* plugin,
    const float* const* inputs,
    uint32_t num_input_channels,
    float* const* outputs,
    uint32_t num_output_channels,
    uint32_t frames
);

// Get parameter count
// Thread-safety: Read-only after initialization. Safe to call from any thread,
// but plugin instances should not be shared across threads (Send but not Sync).
int rack_au_plugin_parameter_count(RackAUPlugin* plugin);

// Get parameter value (normalized 0.0 to 1.0)
// Returns 0 on success, negative error code on failure
// Thread-safety: Can be called from any thread, but the same plugin instance
// must not be accessed concurrently. Parameter cache is read-only after init.
// Typical usage: one thread owns the plugin, calls from audio/UI threads are serialized.
int rack_au_plugin_get_parameter(RackAUPlugin* plugin, uint32_t index, float* value);

// Set parameter value (normalized 0.0 to 1.0)
// Returns 0 on success, negative error code on failure
// Thread-safety: Can be called from any thread, but the same plugin instance
// must not be accessed concurrently. Parameter cache is read-only after init.
// Note: Calling during audio processing may cause clicks/pops (AudioUnit internal behavior).
int rack_au_plugin_set_parameter(RackAUPlugin* plugin, uint32_t index, float value);

// Get parameter info
// name: output buffer for parameter name (allocated by caller)
// name_size: size of name buffer
// unit: output buffer for parameter unit string (allocated by caller, can be NULL)
// unit_size: size of unit buffer (ignored if unit is NULL)
// Returns 0 on success, negative error code on failure
int rack_au_plugin_parameter_info(
    RackAUPlugin* plugin,
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
} RackAUPresetInfo;

// Get factory preset count
// Returns number of factory presets, or 0 if plugin has no presets
// Thread-safety: Read-only after initialization. Safe to call from any thread.
int rack_au_plugin_get_preset_count(RackAUPlugin* plugin);

// Get preset info by index
// index: preset index (0 to preset_count - 1)
// name: output buffer for preset name (allocated by caller)
// name_size: size of name buffer
// preset_number: output parameter for preset number (used with load_preset)
// Returns 0 on success, negative error code on failure
int rack_au_plugin_get_preset_info(
    RackAUPlugin* plugin,
    uint32_t index,
    char* name,
    size_t name_size,
    int32_t* preset_number
);

// Load a factory preset by preset number
// preset_number: the preset number from get_preset_info()
// Returns 0 on success, negative error code on failure
// Thread-safety: Should be called from the same thread that owns the plugin instance.
int rack_au_plugin_load_preset(RackAUPlugin* plugin, int32_t preset_number);

// Get plugin state size (for allocation)
// Returns size in bytes needed to store state, or 0 if state cannot be retrieved
// Thread-safety: Read-only after initialization. Safe to call from any thread.
int rack_au_plugin_get_state_size(RackAUPlugin* plugin);

// Get plugin state (full state including parameters, preset, etc.)
// data: output buffer for state data (allocated by caller)
// size: input/output - buffer size on input, actual size on output
// Returns 0 on success, negative error code on failure
// Thread-safety: Should be called from the same thread that owns the plugin instance.
// Typical usage: call get_state_size() first, allocate buffer, then call get_state()
int rack_au_plugin_get_state(RackAUPlugin* plugin, uint8_t* data, size_t* size);

// Set plugin state (restore full state including parameters, preset, etc.)
// data: state data (from previous get_state call)
// size: size of state data in bytes
// Returns 0 on success, negative error code on failure
// Thread-safety: Should be called from the same thread that owns the plugin instance.
int rack_au_plugin_set_state(RackAUPlugin* plugin, const uint8_t* data, size_t size);

// ============================================================================
// MIDI API
// ============================================================================

// MIDI event types
typedef enum {
    RACK_AU_MIDI_NOTE_ON = 0x90,
    RACK_AU_MIDI_NOTE_OFF = 0x80,
    RACK_AU_MIDI_POLYPHONIC_AFTERTOUCH = 0xA0,
    RACK_AU_MIDI_CONTROL_CHANGE = 0xB0,
    RACK_AU_MIDI_PROGRAM_CHANGE = 0xC0,
    RACK_AU_MIDI_CHANNEL_AFTERTOUCH = 0xD0,
    RACK_AU_MIDI_PITCH_BEND = 0xE0,
    // System messages (no channel)
    RACK_AU_MIDI_SYSTEM_EXCLUSIVE = 0xF0,
    RACK_AU_MIDI_TIME_CODE = 0xF1,
    RACK_AU_MIDI_SONG_POSITION = 0xF2,
    RACK_AU_MIDI_SONG_SELECT = 0xF3,
    RACK_AU_MIDI_TUNE_REQUEST = 0xF6,
    RACK_AU_MIDI_TIMING_CLOCK = 0xF8,
    RACK_AU_MIDI_START = 0xFA,
    RACK_AU_MIDI_CONTINUE = 0xFB,
    RACK_AU_MIDI_STOP = 0xFC,
    RACK_AU_MIDI_ACTIVE_SENSING = 0xFE,
    RACK_AU_MIDI_SYSTEM_RESET = 0xFF,
} RackAUMidiEventType;

// MIDI event struct
typedef struct {
    uint32_t sample_offset;  // Sample offset within buffer
    uint8_t status;          // MIDI status byte
    uint8_t data1;           // First data byte (note/CC number)
    uint8_t data2;           // Second data byte (velocity/value)
    uint8_t channel;         // MIDI channel (0-15)
} RackAUMidiEvent;

// Send MIDI events to plugin
// events: array of MIDI events
// event_count: number of events in array
// Returns 0 on success, negative error code on failure
// Thread-safety: Should be called from the same thread that owns the plugin instance.
// Not safe to call concurrently with process() or other plugin operations.
int rack_au_plugin_send_midi(
    RackAUPlugin* plugin,
    const RackAUMidiEvent* events,
    uint32_t event_count
);

// ============================================================================
// GUI API
// ============================================================================

// Callback type for async GUI creation
// user_data: user-provided data passed to rack_au_gui_create_async
// gui: created GUI handle, or NULL on error
// error_code: RACK_AU_OK on success, negative error code on failure
typedef void (*RackAUGuiCallback)(void* user_data, RackAUGui* gui, int error_code);

// Create GUI asynchronously
// Tries AUv3 (modern) → AUv2 (legacy) → generic parameter UI in order
// Callback is invoked on main thread when GUI is ready or creation fails
//
// Generic UI fallback:
//   - Displays up to 20 parameters with sliders
//   - Bidirectional: sliders update plugin parameters in real-time
//   - Most plugins provide AUv3/AUv2 custom UIs with richer features
//
// IMPORTANT: This function must be called from the main thread
// The callback will also be invoked on the main thread
//
// plugin: plugin instance (must be initialized)
// callback: callback function to invoke when GUI is ready
// user_data: user data to pass to callback
//
// Thread-safety: Must be called from main thread. GUI operations are not thread-safe.
void rack_au_gui_create_async(
    RackAUPlugin* plugin,
    RackAUGuiCallback callback,
    void* user_data
);

// Destroy GUI and clean up resources
// gui: GUI handle returned by rack_au_gui_create_async
// IMPORTANT: gui pointer becomes invalid immediately after this call
// Cleanup happens asynchronously on main thread
// Thread-safety: Should be called from main thread
void rack_au_gui_destroy(RackAUGui* gui);

// Get native NSView pointer for embedding in host UI
// Returns void* that can be cast to NSView* in Objective-C/Swift code
// gui: GUI handle
// Returns: NSView pointer as void*, or NULL if gui is invalid
// Thread-safety: Can be called from any thread (read-only operation)
void* rack_au_gui_get_view(RackAUGui* gui);

// Get view size
// gui: GUI handle
// width: output parameter for view width
// height: output parameter for view height
// Returns 0 on success, negative error code on failure
// Thread-safety: Can be called from any thread
int rack_au_gui_get_size(RackAUGui* gui, float* width, float* height);

// Create and show window with GUI
// Creates an NSWindow and displays the plugin GUI in it
// gui: GUI handle
// title: window title (or NULL for default "AudioUnit GUI")
// Returns 0 on success, negative error code on failure
// Thread-safety: Must be called from main thread
int rack_au_gui_show_window(RackAUGui* gui, const char* title);

// Hide window (without destroying GUI)
// gui: GUI handle
// Returns 0 on success, negative error code on failure
// Thread-safety: Must be called from main thread
int rack_au_gui_hide_window(RackAUGui* gui);

#ifdef __cplusplus
}
#endif

// ============================================================================
// Internal Helper (used by au_gui.mm)
// ============================================================================

#ifdef __OBJC__
// Get AudioComponentInstance from plugin (internal use only)
// Used by au_gui.mm to access the audio unit from opaque plugin handle
// Only available in Objective-C++ where AudioToolbox types are available
#include <AudioToolbox/AudioToolbox.h>
#ifdef __cplusplus
extern "C" {
#endif
AudioComponentInstance rack_au_plugin_get_audio_unit(RackAUPlugin* plugin);
#ifdef __cplusplus
}
#endif
#endif

#endif // RACK_AU_H
