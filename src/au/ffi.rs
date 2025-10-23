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

    /// Process audio through the plugin
    ///
    /// # Returns
    ///
    /// - 0 on success
    /// - Negative error code on failure
    ///
    /// # Safety
    ///
    /// - `plugin` must be a valid pointer and initialized
    /// - `input` must point to a buffer with at least `frames * 2` f32 values (stereo interleaved)
    /// - `output` must point to a buffer with space for at least `frames * 2` f32 values (stereo interleaved)
    /// - `frames` must not exceed the `max_block_size` from initialization
    /// - Buffers must not overlap unless input == output (in-place processing)
    /// - Must not be called concurrently on the same plugin from multiple threads
    pub fn rack_au_plugin_process(
        plugin: *mut RackAUPlugin,
        input: *const f32,
        output: *mut f32,
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
    ControlChange = 0xB0,
    ProgramChange = 0xC0,
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
