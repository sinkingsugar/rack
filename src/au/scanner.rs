use crate::{Error, PluginInfo, PluginScanner, PluginType, Result};
use std::ffi::CStr;
use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::ptr::NonNull;

use super::ffi;
use super::instance::AudioUnitPlugin;

/// Scanner for AudioUnit plugins on macOS
pub struct AudioUnitScanner {
    inner: NonNull<ffi::RackAUScanner>,
}

impl AudioUnitScanner {
    /// Create a new AudioUnit scanner
    ///
    /// # Errors
    ///
    /// Returns an error if scanner allocation fails
    pub fn new() -> Result<Self> {
        unsafe {
            let ptr = ffi::rack_au_scanner_new();
            if ptr.is_null() {
                return Err(Error::Other("Failed to allocate scanner".to_string()));
            }
            Ok(Self {
                inner: NonNull::new_unchecked(ptr),
            })
        }
    }

    /// Scan for AudioUnit components
    fn scan_components(&self) -> Result<Vec<PluginInfo>> {
        unsafe {
            // First pass: get count
            let count = ffi::rack_au_scanner_scan(self.inner.as_ptr(), std::ptr::null_mut(), 0);

            if count < 0 {
                return Err(Error::from_os_status(count));
            }

            if count == 0 {
                return Ok(Vec::new());
            }

            // Check for integer overflow when converting c_int to usize
            let count_usize = usize::try_from(count)
                .map_err(|_| Error::Other("Plugin count exceeds usize".to_string()))?;

            // Allocate uninitialized array for results
            // Safety: MaybeUninit allows uninitialized memory for C interop
            let mut plugins_c: Vec<MaybeUninit<ffi::RackAUPluginInfo>> =
                Vec::with_capacity(count_usize);
            plugins_c.resize_with(count_usize, MaybeUninit::uninit);

            // Second pass: fill array
            let actual_count = ffi::rack_au_scanner_scan(
                self.inner.as_ptr(),
                plugins_c.as_mut_ptr() as *mut ffi::RackAUPluginInfo,
                count_usize,
            );

            if actual_count < 0 {
                return Err(Error::from_os_status(actual_count));
            }

            // Handle race condition: plugin list may have changed between passes
            let actual_count_usize = usize::try_from(actual_count)
                .map_err(|_| Error::Other("Actual plugin count exceeds usize".to_string()))?;

            // Use the minimum of the two counts to avoid reading uninitialized memory
            let valid_count = actual_count_usize.min(count_usize);

            // Convert initialized C structs to Rust PluginInfo
            // Safety: The C++ code guarantees that the first `actual_count` elements are initialized
            let plugins = plugins_c
                .into_iter()
                .take(valid_count)
                .map(|p| {
                    // Safety: C++ has written valid data to these elements
                    let plugin_info = p.assume_init();
                    convert_plugin_info(&plugin_info)
                })
                .collect::<Result<Vec<_>>>()?;

            Ok(plugins)
        }
    }
}

/// Convert C plugin info to Rust PluginInfo
fn convert_plugin_info(c_info: &ffi::RackAUPluginInfo) -> Result<PluginInfo> {
    unsafe {
        // Safety: The C++ code (au_scanner.cpp) guarantees:
        // 1. All string buffers are null-terminated
        // 2. Strings fit within their fixed-size buffers (256/1024/64 bytes)
        // 3. Buffers contain valid UTF-8 (AudioUnit API uses UTF-8)
        // The C++ implementation uses strncpy with explicit null-termination

        let name = CStr::from_ptr(c_info.name.as_ptr())
            .to_str()
            .map_err(|e| Error::Other(format!("Invalid UTF-8 in plugin name: {}", e)))?
            .to_string();

        let manufacturer = CStr::from_ptr(c_info.manufacturer.as_ptr())
            .to_str()
            .map_err(|e| Error::Other(format!("Invalid UTF-8 in manufacturer: {}", e)))?
            .to_string();

        let path = CStr::from_ptr(c_info.path.as_ptr())
            .to_str()
            .map_err(|e| Error::Other(format!("Invalid UTF-8 in path: {}", e)))?;

        let unique_id = CStr::from_ptr(c_info.unique_id.as_ptr())
            .to_str()
            .map_err(|e| Error::Other(format!("Invalid UTF-8 in unique_id: {}", e)))?
            .to_string();

        // Convert plugin type
        let plugin_type = match c_info.plugin_type {
            ffi::RackAUPluginType::Effect => PluginType::Effect,
            ffi::RackAUPluginType::Instrument => PluginType::Instrument,
            ffi::RackAUPluginType::Mixer => PluginType::Mixer,
            ffi::RackAUPluginType::FormatConverter => PluginType::FormatConverter,
            ffi::RackAUPluginType::Other => PluginType::Other,
        };

        Ok(PluginInfo::new(
            name,
            manufacturer,
            c_info.version,
            plugin_type,
            PathBuf::from(path),
            unique_id,
        ))
    }
}

impl Drop for AudioUnitScanner {
    fn drop(&mut self) {
        unsafe {
            ffi::rack_au_scanner_free(self.inner.as_ptr());
        }
    }
}

impl PluginScanner for AudioUnitScanner {
    type Plugin = AudioUnitPlugin;

    fn scan(&self) -> Result<Vec<PluginInfo>> {
        self.scan_components()
    }

    fn scan_path(&self, _path: &std::path::Path) -> Result<Vec<PluginInfo>> {
        // AudioUnits are registered with the system, not scanned from paths
        // So we just return the same result as scan()
        self.scan()
    }

    fn load(&self, info: &PluginInfo) -> Result<Self::Plugin> {
        AudioUnitPlugin::new(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanner_creation() {
        let result = AudioUnitScanner::new();
        assert!(result.is_ok(), "Scanner creation should succeed");
    }

    #[test]
    fn test_scanner_creation_returns_result() {
        // Verify that new() returns Result
        let scanner = AudioUnitScanner::new();
        match scanner {
            Ok(_) => (),
            Err(e) => panic!("Scanner creation failed: {}", e),
        }
    }

    #[test]
    fn test_scan() {
        let scanner = AudioUnitScanner::new().expect("Scanner creation should succeed");
        let result = scanner.scan();
        assert!(result.is_ok(), "Scan should succeed");
    }

    #[test]
    fn test_scan_returns_plugins() {
        let scanner = AudioUnitScanner::new().expect("Scanner creation should succeed");
        let plugins = scanner.scan().expect("Scan should succeed");
        // On macOS, there should always be at least some system AudioUnits
        // But we don't assert a specific count as it varies by system
        println!("Found {} plugins", plugins.len());
    }

    #[test]
    fn test_drop_behavior() {
        // Create and immediately drop scanner to test Drop implementation
        {
            let _scanner = AudioUnitScanner::new().expect("Scanner creation should succeed");
        } // Scanner dropped here
          // If Drop is implemented correctly, this shouldn't leak or crash
    }

    #[test]
    fn test_multiple_scans() {
        let scanner = AudioUnitScanner::new().expect("Scanner creation should succeed");

        // Scan multiple times to ensure it's stable
        let result1 = scanner.scan().expect("First scan should succeed");
        let result2 = scanner.scan().expect("Second scan should succeed");

        // Results should be consistent (though plugin count might vary slightly)
        // We just verify both scans succeed without errors
        assert!(!result1.is_empty() || result2.is_empty(), "Scans should be stable");
    }

    #[test]
    fn test_plugin_info_fields() {
        let scanner = AudioUnitScanner::new().expect("Scanner creation should succeed");
        let plugins = scanner.scan().expect("Scan should succeed");

        if let Some(plugin) = plugins.first() {
            // Verify all fields are populated
            assert!(!plugin.name.is_empty(), "Plugin name should not be empty");
            assert!(!plugin.manufacturer.is_empty(), "Manufacturer should not be empty");
            assert!(!plugin.unique_id.is_empty(), "Unique ID should not be empty");
            // Version can be 0, so we don't assert it
            // path may be "<system>" for system plugins, so we just check it's not empty
            assert!(plugin.path.as_os_str().len() > 0, "Path should not be empty");
        }
    }

    #[test]
    fn test_scan_path_delegates_to_scan() {
        let scanner = AudioUnitScanner::new().expect("Scanner creation should succeed");
        let path = std::path::Path::new("/dummy/path");

        // scan_path should work for AudioUnits (delegates to scan)
        let result = scanner.scan_path(path);
        assert!(result.is_ok(), "scan_path should succeed");
    }
}
