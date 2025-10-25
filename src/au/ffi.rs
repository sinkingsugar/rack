//! Raw FFI bindings to the rack-sys C API
//!
//! This module contains unsafe FFI declarations. All safe wrappers
//! should be in scanner.rs and instance.rs.

#![allow(dead_code)]

use std::os::raw::{c_char, c_int};

// Opaque types (zero-sized to prevent construction)
#[repr(C)]
pub struct RackAUScanner {
    _private: [u8; 0],
}

#[repr(C)]
pub struct RackAUPlugin {
    _private: [u8; 0],
}

#[repr(C)]
pub struct RackAUGui {
    _private: [u8; 0],
}

// Plugin type enum
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RackAUPluginType {
    Effect = 0,
    Instrument = 1,
    Mixer = 2,
    FormatConverter = 3,
    Other = 4,
}

// Plugin info struct (matches C layout exactly)
#[repr(C)]
#[derive(Clone)]
pub struct RackAUPluginInfo {
    pub name: [c_char; 256],
    pub manufacturer: [c_char; 256],
    pub path: [c_char; 1024],
    pub unique_id: [c_char; 64],
    pub version: u32,
    pub plugin_type: RackAUPluginType,
}

// Error codes
pub const RACK_AU_OK: c_int = 0;
pub const RACK_AU_ERROR_GENERIC: c_int = -1;
pub const RACK_AU_ERROR_NOT_FOUND: c_int = -2;
pub const RACK_AU_ERROR_INVALID_PARAM: c_int = -3;
pub const RACK_AU_ERROR_NOT_INITIALIZED: c_int = -4;
pub const RACK_AU_ERROR_AUDIO_UNIT: c_int = -1000;

extern "C" {
    // ============================================================================
    // Scanner API
    // ============================================================================

    /// Create a new scanner
    ///
    /// # Returns
    ///
    /// Returns a pointer to a new scanner, or NULL if allocation fails
    ///
    /// # Safety
    ///
    /// - The returned pointer must be freed with `rack_au_scanner_free`
    /// - The pointer is valid until `rack_au_scanner_free` is called
    /// - Must not be called from multiple threads without synchronization
    pub fn rack_au_scanner_new() -> *mut RackAUScanner;

    /// Free scanner
    ///
    /// # Safety
    ///
    /// - `scanner` must be a valid pointer returned by `rack_au_scanner_new`
    /// - `scanner` must not be used after this call
    /// - Must not be called multiple times with the same pointer
    /// - If `scanner` is NULL, this function does nothing (safe no-op)
    pub fn rack_au_scanner_free(scanner: *mut RackAUScanner);

    /// Scan for plugins
    ///
    /// Two-pass usage pattern:
    /// 1. `count = rack_au_scanner_scan(scanner, NULL, 0);` - Get plugin count
    /// 2. `rack_au_scanner_scan(scanner, array, count);` - Fill array with plugin info
    ///
    /// # Returns
    ///
    /// - On success: number of plugins found (may exceed max_plugins if more exist)
    /// - On error: negative error code (see RACK_AU_ERROR_* constants)
    ///
    /// # Safety
    ///
    /// - `scanner` must be a valid pointer returned by `rack_au_scanner_new`
    /// - If `plugins` is NULL, `max_plugins` is ignored (count-only mode)
    /// - If `plugins` is not NULL:
    ///   - Must point to an array with at least `max_plugins` elements
    ///   - Array must be valid for writes
    ///   - The first min(return_value, max_plugins) elements will be initialized
    /// - C++ code guarantees:
    ///   - All string fields are null-terminated
    ///   - Strings fit within their buffer sizes
    ///   - No buffer overflows occur
    /// - Thread-safety: Scanner can be used from multiple threads with proper synchronization
    pub fn rack_au_scanner_scan(
        scanner: *mut RackAUScanner,
        plugins: *mut RackAUPluginInfo,
        max_plugins: usize,
    ) -> c_int;

    // ============================================================================
    // Plugin Instance API (for future phases)
    // ============================================================================

    /// Create a new plugin instance from unique_id
    ///
    /// # Safety
    ///
    /// - `unique_id` must be a valid null-terminated C string
    /// - `unique_id` must remain valid for the duration of the call
    /// - Returns NULL if plugin not found or allocation fails
    /// - Returned pointer must be freed with `rack_au_plugin_free`
    pub fn rack_au_plugin_new(unique_id: *const c_char) -> *mut RackAUPlugin;

    /// Free plugin instance
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - `plugin` must not be used after this call
    /// - Must not be called multiple times with the same pointer
    /// - If `plugin` is NULL, this function does nothing (safe no-op)
    pub fn rack_au_plugin_free(plugin: *mut RackAUPlugin);

    /// Initialize plugin with sample rate and buffer size
    ///
    /// # Returns
    ///
    /// - 0 on success
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - `sample_rate` must be positive and reasonable (e.g., 44100-192000)
    /// - `max_block_size` must be positive and reasonable (e.g., 64-8192)
    /// - Must be called before `rack_au_plugin_process`
    pub fn rack_au_plugin_initialize(
        plugin: *mut RackAUPlugin,
        sample_rate: f64,
        max_block_size: u32,
    ) -> c_int;

    /// Check if plugin is initialized
    ///
    /// # Returns
    ///
    /// - 1 if initialized
    /// - 0 if not initialized
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    pub fn rack_au_plugin_is_initialized(plugin: *mut RackAUPlugin) -> c_int;

    /// Reset plugin state
    ///
    /// Clears all internal buffers, delay lines, and state without changing parameters.
    /// Useful for clearing reverb tails, delay lines, etc. between songs or after preset changes.
    ///
    /// # Returns
    ///
    /// - 0 on success
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - Plugin must be initialized (returns error if not)
    pub fn rack_au_plugin_reset(plugin: *mut RackAUPlugin) -> c_int;

    /// Get input channel count
    ///
    /// # Returns
    ///
    /// - Number of input channels (>= 0)
    /// - 0 if not initialized or query failed
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - Should be called after `rack_au_plugin_initialize`
    pub fn rack_au_plugin_get_input_channels(plugin: *mut RackAUPlugin) -> c_int;

    /// Get output channel count
    ///
    /// # Returns
    ///
    /// - Number of output channels (>= 0)
    /// - 0 if not initialized or query failed
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - Should be called after `rack_au_plugin_initialize`
    pub fn rack_au_plugin_get_output_channels(plugin: *mut RackAUPlugin) -> c_int;

    /// Process audio through the plugin (planar format)
    ///
    /// Uses planar (non-interleaved) audio format - one buffer per channel.
    /// This matches AudioUnit's internal format, enabling zero-copy processing.
    ///
    /// # Returns
    ///
    /// - 0 on success
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer and initialized
    /// - `inputs` must point to an array of `num_input_channels` const f32 pointers
    /// - Each input channel pointer must point to a buffer with at least `frames` f32 values
    /// - `outputs` must point to an array of `num_output_channels` mutable f32 pointers
    /// - Each output channel pointer must point to a buffer with space for at least `frames` f32 values
    /// - `frames` must not exceed the `max_block_size` from initialization
    /// - Input and output buffers must not overlap unless doing in-place processing
    /// - Must not be called concurrently on the same plugin from multiple threads
    ///
    /// # Channel Layout
    ///
    /// For stereo: inputs/outputs = [left_ptr, right_ptr], num_channels = 2
    /// For mono: inputs/outputs = [mono_ptr], num_channels = 1
    pub fn rack_au_plugin_process(
        plugin: *mut RackAUPlugin,
        inputs: *const *const f32,
        num_input_channels: u32,
        outputs: *const *mut f32,
        num_output_channels: u32,
        frames: u32,
    ) -> c_int;

    /// Get parameter count
    ///
    /// # Returns
    ///
    /// - Parameter count (>= 0) on success
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    pub fn rack_au_plugin_parameter_count(plugin: *mut RackAUPlugin) -> c_int;

    /// Get parameter value (normalized 0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// - 0 on success (value written to `value` pointer)
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - `index` must be less than parameter count
    /// - `value` must be a valid pointer to an f32
    pub fn rack_au_plugin_get_parameter(
        plugin: *mut RackAUPlugin,
        index: u32,
        value: *mut f32,
    ) -> c_int;

    /// Set parameter value (normalized 0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// - 0 on success
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - `index` must be less than parameter count
    /// - `value` should be in range 0.0-1.0 (values outside may be clamped)
    pub fn rack_au_plugin_set_parameter(
        plugin: *mut RackAUPlugin,
        index: u32,
        value: f32,
    ) -> c_int;

    /// Get parameter info (name, min, max, default, unit)
    ///
    /// # Returns
    ///
    /// - 0 on success (values written to output pointers)
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - `index` must be less than parameter count
    /// - `name` must point to a buffer with at least `name_size` bytes
    /// - `min`, `max`, `default_value` must be valid pointers to f32
    /// - `unit` can be NULL, or must point to a buffer with at least `unit_size` bytes
    /// - `name` and `unit` (if not NULL) will be null-terminated
    /// - `name_size` should be at least 256 bytes for typical parameter names
    /// - `unit_size` should be at least 32 bytes for typical unit strings
    pub fn rack_au_plugin_parameter_info(
        plugin: *mut RackAUPlugin,
        index: u32,
        name: *mut c_char,
        name_size: usize,
        min: *mut f32,
        max: *mut f32,
        default_value: *mut f32,
        unit: *mut c_char,
        unit_size: usize,
    ) -> c_int;

    // ============================================================================
    // Preset Management API
    // ============================================================================

    /// Get factory preset count
    ///
    /// # Returns
    ///
    /// - Number of factory presets (>= 0)
    /// - 0 if plugin has no presets
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - Plugin must be initialized
    pub fn rack_au_plugin_get_preset_count(plugin: *mut RackAUPlugin) -> c_int;

    /// Get preset info by index
    ///
    /// # Returns
    ///
    /// - 0 on success (name and preset_number written to output pointers)
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - Plugin must be initialized
    /// - `index` must be less than preset count
    /// - `name` must point to a buffer with at least `name_size` bytes
    /// - `preset_number` must be a valid pointer to an i32
    /// - `name` will be null-terminated
    /// - `name_size` should be at least 256 bytes for typical preset names
    pub fn rack_au_plugin_get_preset_info(
        plugin: *mut RackAUPlugin,
        index: u32,
        name: *mut c_char,
        name_size: usize,
        preset_number: *mut i32,
    ) -> c_int;

    /// Load a factory preset by preset number
    ///
    /// # Returns
    ///
    /// - 0 on success
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - Plugin must be initialized
    /// - `preset_number` should be a valid preset number from get_preset_info
    pub fn rack_au_plugin_load_preset(plugin: *mut RackAUPlugin, preset_number: i32) -> c_int;

    /// Get plugin state size (for allocation)
    ///
    /// # Returns
    ///
    /// - Size in bytes needed to store state (> 0)
    /// - 0 if state cannot be retrieved
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - Plugin must be initialized
    pub fn rack_au_plugin_get_state_size(plugin: *mut RackAUPlugin) -> c_int;

    /// Get plugin state (full state including parameters, preset, etc.)
    ///
    /// # Returns
    ///
    /// - 0 on success (state written to data buffer, actual size written to size pointer)
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - Plugin must be initialized
    /// - `data` must point to a buffer with at least `*size` bytes
    /// - `size` must be a valid pointer to a size_t
    /// - On input, `*size` is the buffer size
    /// - On output, `*size` is the actual size written
    /// - Typical usage: call get_state_size() first, allocate buffer, then call get_state()
    pub fn rack_au_plugin_get_state(
        plugin: *mut RackAUPlugin,
        data: *mut u8,
        size: *mut usize,
    ) -> c_int;

    /// Set plugin state (restore full state including parameters, preset, etc.)
    ///
    /// # Returns
    ///
    /// - 0 on success
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - Plugin must be initialized
    /// - `data` must point to valid state data (from previous get_state call)
    /// - `data` must remain valid for the duration of the call
    /// - `size` must be the size of the state data in bytes
    pub fn rack_au_plugin_set_state(
        plugin: *mut RackAUPlugin,
        data: *const u8,
        size: usize,
    ) -> c_int;

    // ============================================================================
    // MIDI API
    // ============================================================================

    /// Send MIDI events to plugin
    ///
    /// # Returns
    ///
    /// - 0 on success
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - `events` must point to an array with at least `event_count` elements, or NULL if event_count is 0
    /// - All events must have valid channel (0-15) and data values
    /// - Must not be called concurrently with process() or other plugin operations
    pub fn rack_au_plugin_send_midi(
        plugin: *mut RackAUPlugin,
        events: *const RackAUMidiEvent,
        event_count: u32,
    ) -> c_int;
}

// MIDI event types
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RackAUMidiEventType {
    NoteOn = 0x90,
    NoteOff = 0x80,
    PolyphonicAftertouch = 0xA0,
    ControlChange = 0xB0,
    ProgramChange = 0xC0,
    ChannelAftertouch = 0xD0,
    PitchBend = 0xE0,
    // System messages (no channel)
    SystemExclusive = 0xF0,
    TimeCode = 0xF1,
    SongPosition = 0xF2,
    SongSelect = 0xF3,
    TuneRequest = 0xF6,
    TimingClock = 0xF8,
    Start = 0xFA,
    Continue = 0xFB,
    Stop = 0xFC,
    ActiveSensing = 0xFE,
    SystemReset = 0xFF,
}

// MIDI event struct (matches C layout exactly)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RackAUMidiEvent {
    pub sample_offset: u32,
    pub status: u8,
    pub data1: u8,
    pub data2: u8,
    pub channel: u8,
}

// ============================================================================
// GUI FFI Types and Functions
// ============================================================================

/// Callback type for async GUI creation
///
/// # Parameters
///
/// - `user_data`: User-provided data passed to `rack_au_gui_create_async`
/// - `gui`: Created GUI handle, or NULL on error
/// - `error_code`: RACK_AU_OK on success, negative error code on failure
pub type RackAUGuiCallback = extern "C" fn(user_data: *mut std::ffi::c_void, gui: *mut RackAUGui, error_code: c_int);

extern "C" {
    // ============================================================================
    // GUI API
    // ============================================================================

    /// Create GUI asynchronously
    ///
    /// Tries AUv3 (modern) → AUv2 (legacy) → generic parameter UI in order.
    /// Callback is invoked on main thread when GUI is ready or creation fails.
    ///
    /// **IMPORTANT**: This function must be called from the main thread.
    /// The callback will also be invoked on the main thread.
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer returned by `rack_au_plugin_new`
    /// - Plugin must be initialized
    /// - `callback` must be a valid function pointer
    /// - `user_data` can be any pointer (will be passed to callback)
    /// - Must be called from main thread
    /// - Callback will be invoked on main thread
    pub fn rack_au_gui_create_async(
        plugin: *mut RackAUPlugin,
        callback: RackAUGuiCallback,
        user_data: *mut std::ffi::c_void,
    );

    /// Destroy GUI and clean up resources
    ///
    /// # Safety
    ///
    /// - `gui` must be a valid pointer returned via `rack_au_gui_create_async` callback
    /// - Should be called from main thread
    /// - `gui` must not be used after this call
    /// - If `gui` is NULL, this function does nothing (safe no-op)
    pub fn rack_au_gui_destroy(gui: *mut RackAUGui);

    /// Get native NSView pointer for embedding in host UI
    ///
    /// Returns void* that can be cast to NSView* in Objective-C/Swift code.
    ///
    /// # Returns
    ///
    /// - NSView pointer as void*, or NULL if gui is invalid
    ///
    /// # Safety
    ///
    /// - `gui` must be a valid pointer returned via `rack_au_gui_create_async` callback
    /// - Can be called from any thread (read-only operation)
    /// - Returned pointer is valid until `rack_au_gui_destroy` is called
    pub fn rack_au_gui_get_view(gui: *mut RackAUGui) -> *mut std::ffi::c_void;

    /// Get view size
    ///
    /// # Returns
    ///
    /// - 0 on success (width and height written to output pointers)
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `gui` must be a valid pointer returned via `rack_au_gui_create_async` callback
    /// - `width` and `height` must be valid pointers to f32
    /// - Can be called from any thread
    pub fn rack_au_gui_get_size(
        gui: *mut RackAUGui,
        width: *mut f32,
        height: *mut f32,
    ) -> c_int;

    /// Create and show window with GUI
    ///
    /// Creates an NSWindow and displays the plugin GUI in it.
    ///
    /// # Returns
    ///
    /// - 0 on success
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `gui` must be a valid pointer returned via `rack_au_gui_create_async` callback
    /// - `title` can be NULL for default title, or must point to null-terminated C string
    /// - Must be called from main thread
    pub fn rack_au_gui_show_window(
        gui: *mut RackAUGui,
        title: *const c_char,
    ) -> c_int;

    /// Hide window (without destroying GUI)
    ///
    /// # Returns
    ///
    /// - 0 on success
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `gui` must be a valid pointer returned via `rack_au_gui_create_async` callback
    /// - Must be called from main thread
    pub fn rack_au_gui_hide_window(gui: *mut RackAUGui) -> c_int;
}
