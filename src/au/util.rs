//! Shared utilities for AudioUnit FFI interop

use crate::{Error, Result};
use std::ffi::CStr;

use super::ffi;

/// Convert C API error code to Rust Error
///
/// The C API returns negative error codes:
/// - RACK_AU_ERROR_* codes (-1 to -4): rack-specific errors
/// - AudioUnit OSStatus codes (< -1000): Apple AudioUnit errors
pub(crate) fn map_error(code: i32) -> Error {
    match code {
        ffi::RACK_AU_ERROR_GENERIC => Error::Other("Generic AudioUnit scanner error".to_string()),
        ffi::RACK_AU_ERROR_NOT_FOUND => Error::PluginNotFound("AudioUnit not found".to_string()),
        ffi::RACK_AU_ERROR_INVALID_PARAM => Error::Other("Invalid parameter".to_string()),
        ffi::RACK_AU_ERROR_NOT_INITIALIZED => Error::NotInitialized,
        // AudioUnit OSStatus errors (< -1000) or unknown negative codes
        _ => Error::from_os_status(code),
    }
}

/// Safely convert a fixed-size C char array to a Rust String
///
/// This uses bounded string conversion to prevent UB even if the C++ code
/// has a bug and fails to null-terminate within the array bounds.
///
/// # Safety
///
/// The caller must ensure the array pointer is valid and the size is correct.
pub(crate) unsafe fn c_array_to_string(arr: &[i8], field_name: &str) -> Result<String> {
    // Cast to u8 slice for CStr::from_bytes_until_nul
    let bytes = std::slice::from_raw_parts(arr.as_ptr() as *const u8, arr.len());

    // Find null terminator within bounds (defense against C++ bugs)
    let cstr = CStr::from_bytes_until_nul(bytes).map_err(|_| {
        Error::Other(format!(
            "{} not null-terminated within buffer (potential C++ bug)",
            field_name
        ))
    })?;

    // Convert to UTF-8 string
    cstr.to_str()
        .map_err(|e| Error::Other(format!("Invalid UTF-8 in {}: {}", field_name, e)))
        .map(|s| s.to_string())
}
