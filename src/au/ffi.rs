//! Raw FFI bindings to the rack-sys C API
//!
//! This module contains unsafe FFI declarations. All safe wrappers
//! should be in scanner.rs and instance.rs.

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
    /// Returns NULL if allocation fails
    pub fn rack_au_scanner_new() -> *mut RackAUScanner;

    /// Free scanner
    pub fn rack_au_scanner_free(scanner: *mut RackAUScanner);

    /// Scan for plugins
    ///
    /// Two-pass usage:
    /// 1. count = rack_au_scanner_scan(scanner, NULL, 0);
    /// 2. rack_au_scanner_scan(scanner, array, count);
    ///
    /// Returns number of plugins found (or would be found), or negative error code
    pub fn rack_au_scanner_scan(
        scanner: *mut RackAUScanner,
        plugins: *mut RackAUPluginInfo,
        max_plugins: usize,
    ) -> c_int;

    // ============================================================================
    // Plugin Instance API (for future phases)
    // ============================================================================

    /// Create a new plugin instance from unique_id
    pub fn rack_au_plugin_new(unique_id: *const c_char) -> *mut RackAUPlugin;

    /// Free plugin instance
    pub fn rack_au_plugin_free(plugin: *mut RackAUPlugin);

    /// Initialize plugin
    /// Returns 0 on success, negative error code on failure
    pub fn rack_au_plugin_initialize(
        plugin: *mut RackAUPlugin,
        sample_rate: f64,
        max_block_size: u32,
    ) -> c_int;

    /// Check if plugin is initialized
    pub fn rack_au_plugin_is_initialized(plugin: *mut RackAUPlugin) -> c_int;

    /// Process audio
    /// input/output: interleaved stereo buffers
    /// frames: number of frames to process
    /// Returns 0 on success, negative error code on failure
    pub fn rack_au_plugin_process(
        plugin: *mut RackAUPlugin,
        input: *const f32,
        output: *mut f32,
        frames: u32,
    ) -> c_int;

    /// Get parameter count
    pub fn rack_au_plugin_parameter_count(plugin: *mut RackAUPlugin) -> c_int;

    /// Get parameter value (normalized 0.0 to 1.0)
    /// Returns 0 on success, negative error code on failure
    pub fn rack_au_plugin_get_parameter(
        plugin: *mut RackAUPlugin,
        index: u32,
        value: *mut f32,
    ) -> c_int;

    /// Set parameter value (normalized 0.0 to 1.0)
    /// Returns 0 on success, negative error code on failure
    pub fn rack_au_plugin_set_parameter(
        plugin: *mut RackAUPlugin,
        index: u32,
        value: f32,
    ) -> c_int;

    /// Get parameter info
    /// Returns 0 on success, negative error code on failure
    pub fn rack_au_plugin_parameter_info(
        plugin: *mut RackAUPlugin,
        index: u32,
        name: *mut c_char,
        name_size: usize,
        min: *mut f32,
        max: *mut f32,
        default_value: *mut f32,
    ) -> c_int;
}
