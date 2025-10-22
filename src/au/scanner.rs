use crate::{Error, PluginInfo, PluginScanner, PluginType, Result};
use std::ffi::CStr;
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
    pub fn new() -> Self {
        unsafe {
            let ptr = ffi::rack_au_scanner_new();
            assert!(!ptr.is_null(), "Failed to allocate scanner");
            Self {
                inner: NonNull::new_unchecked(ptr),
            }
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

            // Allocate array for results
            let mut plugins_c = vec![std::mem::zeroed::<ffi::RackAUPluginInfo>(); count as usize];

            // Second pass: fill array
            let actual_count = ffi::rack_au_scanner_scan(
                self.inner.as_ptr(),
                plugins_c.as_mut_ptr(),
                count as usize,
            );

            if actual_count < 0 {
                return Err(Error::from_os_status(actual_count));
            }

            // Convert C structs to Rust PluginInfo
            let plugins = plugins_c
                .into_iter()
                .take(actual_count as usize)
                .map(|p| convert_plugin_info(&p))
                .collect::<Result<Vec<_>>>()?;

            Ok(plugins)
        }
    }
}

/// Convert C plugin info to Rust PluginInfo
fn convert_plugin_info(c_info: &ffi::RackAUPluginInfo) -> Result<PluginInfo> {
    unsafe {
        // Convert C strings to Rust strings
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

impl Default for AudioUnitScanner {
    fn default() -> Self {
        Self::new()
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
        let _scanner = AudioUnitScanner::new();
        assert!(true); // Just verify it compiles and runs
    }

    #[test]
    fn test_scan() {
        let scanner = AudioUnitScanner::new();
        let result = scanner.scan();
        assert!(result.is_ok());
    }
}
