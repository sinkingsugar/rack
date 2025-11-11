use crate::{Error, PluginInfo, PluginScanner, PluginType, Result};
use std::ffi::CString;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

use super::ffi;
use super::instance::Vst3Plugin;
use super::util::{c_array_to_string, map_error};

/// Scanner for VST3 plugins
///
/// # Thread Safety
///
/// This type is `Send` but not `Sync`:
/// - `Send`: The scanner can be moved between threads safely, as each scanner
///   owns its own C++ state
/// - NOT `Sync`: Multiple threads should not access the scanner simultaneously
///   without synchronization. Wrap in `Arc<Mutex<>>` if shared access is needed.
pub struct Vst3Scanner {
    inner: NonNull<ffi::RackVST3Scanner>,
    // PhantomData<*const ()> makes this type !Sync while keeping it Send
    // This prevents concurrent access without Arc<Mutex<>>
    _not_sync: PhantomData<*const ()>,
}

// Safety: Vst3Scanner can be sent between threads because:
// 1. Each scanner instance owns its C++ state exclusively
// 2. No shared mutable state exists between scanner instances
unsafe impl Send for Vst3Scanner {}

// Note: Vst3Scanner is NOT Sync due to PhantomData<*const ()>
// This is intentional - the C++ scanner requires synchronization for shared access

impl Vst3Scanner {
    /// Create a new VST3 scanner
    ///
    /// # Errors
    ///
    /// Returns an error if scanner allocation fails
    pub fn new() -> Result<Self> {
        unsafe {
            let ptr = ffi::rack_vst3_scanner_new();
            if ptr.is_null() {
                return Err(Error::Other("Failed to allocate VST3 scanner".to_string()));
            }

            // Add default system paths
            let result = ffi::rack_vst3_scanner_add_default_paths(ptr);
            if result != ffi::RACK_VST3_OK {
                // Clean up scanner before returning error
                ffi::rack_vst3_scanner_free(ptr);
                return Err(map_error(result));
            }

            Ok(Self {
                inner: NonNull::new_unchecked(ptr),
                _not_sync: PhantomData,
            })
        }
    }

    /// Create a new VST3 scanner without adding default paths
    ///
    /// This is useful for scanning specific paths without system defaults.
    ///
    /// # Errors
    ///
    /// Returns an error if scanner allocation fails
    fn new_empty() -> Result<Self> {
        unsafe {
            let ptr = ffi::rack_vst3_scanner_new();
            if ptr.is_null() {
                return Err(Error::Other("Failed to allocate VST3 scanner".to_string()));
            }

            Ok(Self {
                inner: NonNull::new_unchecked(ptr),
                _not_sync: PhantomData,
            })
        }
    }

    /// Add a custom search path for VST3 plugins
    ///
    /// # Errors
    ///
    /// Returns an error if the path is invalid or cannot be added
    pub fn add_path(&mut self, path: &Path) -> Result<()> {
        let path_str = path.to_str()
            .ok_or_else(|| Error::Other("Path contains invalid UTF-8".to_string()))?;

        let path_cstr = CString::new(path_str)
            .map_err(|_| Error::Other("Path contains null byte".to_string()))?;

        unsafe {
            let result = ffi::rack_vst3_scanner_add_path(self.inner.as_ptr(), path_cstr.as_ptr());
            if result != ffi::RACK_VST3_OK {
                return Err(map_error(result));
            }
        }

        Ok(())
    }

    /// Scan for VST3 plugins
    fn scan_plugins(&self) -> Result<Vec<PluginInfo>> {
        unsafe {
            // First pass: get count
            let count = ffi::rack_vst3_scanner_scan(self.inner.as_ptr(), std::ptr::null_mut(), 0);

            if count < 0 {
                return Err(map_error(count));
            }

            if count == 0 {
                return Ok(Vec::new());
            }

            // Check for integer overflow when converting c_int to usize
            let count_usize = usize::try_from(count)
                .map_err(|_| Error::Other("Plugin count exceeds usize".to_string()))?;

            // Allocate uninitialized array for results
            // Safety: MaybeUninit allows uninitialized memory for C interop
            let mut plugins_c: Vec<MaybeUninit<ffi::RackVST3PluginInfo>> =
                Vec::with_capacity(count_usize);
            plugins_c.resize_with(count_usize, MaybeUninit::uninit);

            // Second pass: fill array
            let actual_count = ffi::rack_vst3_scanner_scan(
                self.inner.as_ptr(),
                plugins_c.as_mut_ptr() as *mut ffi::RackVST3PluginInfo,
                count_usize,
            );

            if actual_count < 0 {
                return Err(map_error(actual_count));
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
fn convert_plugin_info(c_info: &ffi::RackVST3PluginInfo) -> Result<PluginInfo> {
    unsafe {
        // Use bounded string conversion for safety
        // This protects against C++ bugs (missing null-termination)
        // by only searching for null within the fixed array bounds
        let name = c_array_to_string(&c_info.name, "plugin name")?;
        let manufacturer = c_array_to_string(&c_info.manufacturer, "manufacturer")?;
        let path_str = c_array_to_string(&c_info.path, "path")?;
        let unique_id = c_array_to_string(&c_info.unique_id, "unique_id")?;

        // Convert plugin type
        let plugin_type = match c_info.plugin_type {
            ffi::RackVST3PluginType::Effect => PluginType::Effect,
            ffi::RackVST3PluginType::Instrument => PluginType::Instrument,
            ffi::RackVST3PluginType::Analyzer => PluginType::Analyzer,
            ffi::RackVST3PluginType::Spatial => PluginType::Spatial,
            ffi::RackVST3PluginType::Other => PluginType::Other,
        };

        Ok(PluginInfo::new(
            name,
            manufacturer,
            c_info.version,
            plugin_type,
            PathBuf::from(path_str),
            unique_id,
        ))
    }
}

impl Drop for Vst3Scanner {
    fn drop(&mut self) {
        unsafe {
            ffi::rack_vst3_scanner_free(self.inner.as_ptr());
        }
    }
}

impl PluginScanner for Vst3Scanner {
    type Plugin = Vst3Plugin;

    fn scan(&self) -> Result<Vec<PluginInfo>> {
        self.scan_plugins()
    }

    fn scan_path(&self, path: &Path) -> Result<Vec<PluginInfo>> {
        // Create a scanner without default paths for path-specific scanning
        let mut scanner = Self::new_empty()?;

        // Add only the requested path
        scanner.add_path(path)?;

        // Scan
        scanner.scan_plugins()
    }

    fn load(&self, info: &PluginInfo) -> Result<Self::Plugin> {
        Vst3Plugin::new(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanner_creation() {
        let result = Vst3Scanner::new();
        assert!(result.is_ok(), "Scanner creation should succeed");
    }

    #[test]
    fn test_scanner_creation_returns_result() {
        // Verify that new() returns Result
        let scanner = Vst3Scanner::new();
        match scanner {
            Ok(_) => (),
            Err(e) => panic!("Scanner creation failed: {}", e),
        }
    }

    #[test]
    fn test_scan() {
        let scanner = Vst3Scanner::new().expect("Scanner creation should succeed");
        let result = scanner.scan();
        assert!(result.is_ok(), "Scan should succeed");
    }

    #[test]
    fn test_scan_returns_plugins() {
        let scanner = Vst3Scanner::new().expect("Scanner creation should succeed");
        let plugins = scanner.scan().expect("Scan should succeed");
        // VST3 plugins may or may not be installed on the system
        println!("Found {} VST3 plugins", plugins.len());
    }

    #[test]
    fn test_drop_behavior() {
        // Create and immediately drop scanner to test Drop implementation
        {
            let _scanner = Vst3Scanner::new().expect("Scanner creation should succeed");
        } // Scanner dropped here
          // If Drop is implemented correctly, this shouldn't leak or crash
    }

    #[test]
    fn test_multiple_scans() {
        let scanner = Vst3Scanner::new().expect("Scanner creation should succeed");

        // Scan multiple times to ensure it's stable
        let result1 = scanner.scan().expect("First scan should succeed");
        let result2 = scanner.scan().expect("Second scan should succeed");

        // Results should be consistent
        let count1 = result1.len();
        let count2 = result2.len();

        assert_eq!(count1, count2, "Multiple scans should return same count");
    }

    #[test]
    fn test_plugin_info_fields() {
        let scanner = Vst3Scanner::new().expect("Scanner creation should succeed");
        let plugins = scanner.scan().expect("Scan should succeed");

        if let Some(plugin) = plugins.first() {
            // Verify all fields are populated
            assert!(!plugin.name.is_empty(), "Plugin name should not be empty");
            assert!(!plugin.manufacturer.is_empty(), "Manufacturer should not be empty");
            assert!(!plugin.unique_id.is_empty(), "Unique ID should not be empty");
            assert!(plugin.path.as_os_str().len() > 0, "Path should not be empty");
        }
    }

    #[test]
    fn test_add_path() {
        let mut scanner = Vst3Scanner::new().expect("Scanner creation should succeed");
        let path = Path::new("/tmp");

        let result = scanner.add_path(path);
        assert!(result.is_ok(), "Adding path should succeed");
    }
}
