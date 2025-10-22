use crate::{Error, ParameterInfo, PluginInfo, PluginInstance, Result};
use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;

use super::ffi;
use super::util::map_error;

/// An instantiated AudioUnit plugin
///
/// # Thread Safety
///
/// This type is `Send` but not `Sync`:
/// - `Send`: The plugin can be moved between threads safely
/// - NOT `Sync`: Multiple threads should not access the plugin simultaneously
///   without synchronization. Wrap in `Arc<Mutex<>>` if shared access is needed.
pub struct AudioUnitPlugin {
    inner: NonNull<ffi::RackAUPlugin>,
    info: PluginInfo,
    // PhantomData<*const ()> makes this type !Sync while keeping it Send
    _not_sync: PhantomData<*const ()>,
}

// Safety: AudioUnitPlugin can be sent between threads because:
// 1. Each plugin instance owns its C++ state exclusively
// 2. The plugin doesn't share mutable state with other instances
unsafe impl Send for AudioUnitPlugin {}

// Note: AudioUnitPlugin is NOT Sync due to PhantomData<*const ()>
// This is intentional - AudioUnit instances require synchronization for shared access

impl AudioUnitPlugin {
    /// Create a new AudioUnit plugin instance
    pub(crate) fn new(info: &PluginInfo) -> Result<Self> {
        unsafe {
            // Convert unique_id to CString
            let unique_id = CString::new(info.unique_id.as_str())
                .map_err(|_| Error::Other("Invalid unique_id (contains null byte)".to_string()))?;

            // Create plugin instance via FFI
            let ptr = ffi::rack_au_plugin_new(unique_id.as_ptr());
            if ptr.is_null() {
                return Err(Error::PluginNotFound(format!(
                    "Failed to create AudioUnit instance for {}",
                    info.name
                )));
            }

            Ok(Self {
                inner: NonNull::new_unchecked(ptr),
                info: info.clone(),
                _not_sync: PhantomData,
            })
        }
    }
}

impl PluginInstance for AudioUnitPlugin {
    fn initialize(&mut self, sample_rate: f64, max_block_size: usize) -> Result<()> {
        unsafe {
            let result = ffi::rack_au_plugin_initialize(
                self.inner.as_ptr(),
                sample_rate,
                max_block_size as u32,
            );

            if result != ffi::RACK_AU_OK {
                return Err(map_error(result));
            }

            Ok(())
        }
    }

    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        // Calculate frames (input/output are interleaved stereo)
        let frames = (input.len() / 2).min(output.len() / 2);

        unsafe {
            let result = ffi::rack_au_plugin_process(
                self.inner.as_ptr(),
                input.as_ptr(),
                output.as_mut_ptr(),
                frames as u32,
            );

            if result != ffi::RACK_AU_OK {
                return Err(map_error(result));
            }

            Ok(())
        }
    }

    fn parameter_count(&self) -> usize {
        unsafe {
            let count = ffi::rack_au_plugin_parameter_count(self.inner.as_ptr());
            if count < 0 {
                0
            } else {
                count as usize
            }
        }
    }

    fn parameter_info(&self, index: usize) -> Result<ParameterInfo> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        unsafe {
            let mut name = vec![0i8; 256];
            let mut min = 0.0f32;
            let mut max = 0.0f32;
            let mut default_value = 0.0f32;

            let result = ffi::rack_au_plugin_parameter_info(
                self.inner.as_ptr(),
                index as u32,
                name.as_mut_ptr(),
                name.len(),
                &mut min,
                &mut max,
                &mut default_value,
            );

            if result != ffi::RACK_AU_OK {
                return Err(map_error(result));
            }

            // Convert name to String
            let name_cstr = std::ffi::CStr::from_ptr(name.as_ptr());
            let name_str = name_cstr
                .to_str()
                .map_err(|e| Error::Other(format!("Invalid UTF-8 in parameter name: {}", e)))?
                .to_string();

            Ok(ParameterInfo {
                index,
                name: name_str,
                min,
                max,
                default: default_value,
                unit: String::new(), // TODO: Query unit from AudioUnit
            })
        }
    }

    fn get_parameter(&self, index: usize) -> Result<f32> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        unsafe {
            let mut value = 0.0f32;
            let result =
                ffi::rack_au_plugin_get_parameter(self.inner.as_ptr(), index as u32, &mut value);

            if result != ffi::RACK_AU_OK {
                return Err(map_error(result));
            }

            Ok(value)
        }
    }

    fn set_parameter(&mut self, index: usize, value: f32) -> Result<()> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        unsafe {
            let result =
                ffi::rack_au_plugin_set_parameter(self.inner.as_ptr(), index as u32, value);

            if result != ffi::RACK_AU_OK {
                return Err(map_error(result));
            }

            Ok(())
        }
    }

    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn is_initialized(&self) -> bool {
        unsafe {
            let result = ffi::rack_au_plugin_is_initialized(self.inner.as_ptr());
            result != 0
        }
    }
}

impl Drop for AudioUnitPlugin {
    fn drop(&mut self) {
        unsafe {
            ffi::rack_au_plugin_free(self.inner.as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PluginScanner, PluginType};

    // Helper to get a real plugin for testing
    fn get_test_plugin() -> Option<PluginInfo> {
        use super::super::scanner::AudioUnitScanner;

        let scanner = AudioUnitScanner::new().ok()?;
        let plugins = scanner.scan().ok()?;

        // Find an effect or instrument plugin
        plugins
            .into_iter()
            .find(|p| p.plugin_type == PluginType::Effect || p.plugin_type == PluginType::Instrument)
    }

    #[test]
    fn test_plugin_creation_with_invalid_id() {
        use std::path::PathBuf;

        // Try to create plugin with non-existent unique_id
        let info = PluginInfo::new(
            "Fake Plugin".to_string(),
            "Fake Vendor".to_string(),
            1,
            PluginType::Effect,
            PathBuf::from("/fake/path"),
            "ffffffff-ffffffff-ffffffff".to_string(),
        );

        let result = AudioUnitPlugin::new(&info);
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_lifecycle() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        println!("Testing with plugin: {}", info.name);

        // Create plugin
        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        assert!(!plugin.is_initialized(), "Plugin should not be initialized");

        // Initialize plugin
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");
        assert!(plugin.is_initialized(), "Plugin should be initialized");

        // Re-initialize should succeed
        plugin
            .initialize(48000.0, 512)
            .expect("Re-initialization should succeed");
        assert!(plugin.is_initialized());
    }

    #[test]
    fn test_plugin_info() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        let retrieved_info = plugin.info();

        assert_eq!(retrieved_info.name, info.name);
        assert_eq!(retrieved_info.unique_id, info.unique_id);
    }

    #[test]
    fn test_drop_cleanup() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        // Create and drop plugin in a scope
        {
            let _plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        }
        // Plugin should be cleaned up here without crashing
    }
}
