use crate::{AudioBuffer, Error, ParameterInfo, PluginInfo, PluginInstance, Result};
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

    fn process(&mut self, input: &AudioBuffer, output: &mut AudioBuffer) -> Result<()> {
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
            let mut unit = vec![0i8; 32];
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
                unit.as_mut_ptr(),
                unit.len(),
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

            // Convert unit to String
            let unit_cstr = std::ffi::CStr::from_ptr(unit.as_ptr());
            let unit_str = unit_cstr
                .to_str()
                .map_err(|e| Error::Other(format!("Invalid UTF-8 in parameter unit: {}", e)))?
                .to_string();

            Ok(ParameterInfo {
                index,
                name: name_str,
                min,
                max,
                default: default_value,
                unit: unit_str,
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

    #[test]
    fn test_audio_processing() {
        use super::super::scanner::AudioUnitScanner;

        let scanner = AudioUnitScanner::new().expect("Failed to create scanner");
        let plugins = scanner.scan().expect("Failed to scan plugins");

        // Find an effect plugin for processing test
        let Some(info) = plugins
            .into_iter()
            .find(|p| p.plugin_type == PluginType::Effect)
        else {
            println!("No effect plugins available, skipping test");
            return;
        };

        println!("Testing audio processing with: {}", info.name);

        // Create and initialize plugin
        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        // Create test buffers (512 frames of stereo audio = 1024 samples)
        let frames = 512;
        let mut input = AudioBuffer::new(frames * 2);
        let mut output = AudioBuffer::new(frames * 2);

        // Fill input with a simple sine wave (440 Hz)
        let frequency = 440.0f32;
        let sample_rate = 48000.0f32;
        for i in 0..frames {
            let sample = (2.0 * std::f32::consts::PI * frequency * i as f32 / sample_rate).sin() * 0.5;
            input[i * 2] = sample; // Left channel
            input[i * 2 + 1] = sample; // Right channel
        }

        // Process audio
        plugin
            .process(&input, &mut output)
            .expect("Audio processing failed");

        // Verify output is not all zeros (plugin did something)
        let has_signal = output.iter().any(|&sample| sample != 0.0);
        assert!(
            has_signal,
            "Output should contain audio signal (not all zeros)"
        );

        println!("✓ Audio processing succeeded, output contains signal");
    }

    #[test]
    fn test_parameter_count() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        println!("Testing parameter count with: {}", info.name);

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let count = plugin.parameter_count();
        println!("  Found {} parameters", count);

        // Verify we can call it - parameter_count should never panic
        // (Some plugins might have 0 parameters, which is fine)
    }

    #[test]
    fn test_parameter_info() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        println!("Testing parameter info with: {}", info.name);

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let count = plugin.parameter_count();
        if count == 0 {
            println!("  Plugin has no parameters, skipping test");
            return;
        }

        // Get info for first parameter
        let param_info = plugin
            .parameter_info(0)
            .expect("Failed to get parameter info");

        println!("  Parameter 0: {}", param_info.name);
        println!("    Range: {} - {}", param_info.min, param_info.max);
        println!("    Default: {}", param_info.default);

        assert!(!param_info.name.is_empty(), "Parameter name should not be empty");
        assert!(param_info.index == 0);
    }

    #[test]
    fn test_get_set_parameter() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        println!("Testing get/set parameter with: {}", info.name);

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let count = plugin.parameter_count();
        if count == 0 {
            println!("  Plugin has no parameters, skipping test");
            return;
        }

        // Get original value
        let original_value = plugin
            .get_parameter(0)
            .expect("Failed to get parameter");

        println!("  Original value: {}", original_value);

        // Set to a different value (0.75, normalized)
        plugin
            .set_parameter(0, 0.75)
            .expect("Failed to set parameter");

        // Verify it changed
        let new_value = plugin
            .get_parameter(0)
            .expect("Failed to get parameter after set");

        println!("  New value: {}", new_value);

        // Value should be close to 0.75 (allowing for small floating point error)
        assert!(
            (new_value - 0.75).abs() < 0.01,
            "Parameter value should be ~0.75, got {}",
            new_value
        );

        // Restore original value
        plugin
            .set_parameter(0, original_value)
            .expect("Failed to restore parameter");
    }

    #[test]
    fn test_parameter_range_clamping() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let count = plugin.parameter_count();
        if count == 0 {
            return;
        }

        // Test setting values outside 0.0-1.0 range (should be clamped by C++ layer)
        plugin.set_parameter(0, 2.0).expect("Should handle > 1.0");
        let value = plugin.get_parameter(0).expect("Failed to get parameter");
        assert!(
            value <= 1.0,
            "Parameter should be clamped to <= 1.0, got {}",
            value
        );

        plugin.set_parameter(0, -1.0).expect("Should handle < 0.0");
        let value = plugin.get_parameter(0).expect("Failed to get parameter");
        assert!(
            value >= 0.0,
            "Parameter should be clamped to >= 0.0, got {}",
            value
        );
    }

    #[test]
    fn test_parameter_out_of_bounds() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let count = plugin.parameter_count();

        // Try to access parameter beyond count
        let result = plugin.get_parameter(count + 10);
        assert!(result.is_err(), "Should fail for out-of-bounds index");

        let result = plugin.set_parameter(count + 10, 0.5);
        assert!(result.is_err(), "Should fail for out-of-bounds index");

        let result = plugin.parameter_info(count + 10);
        assert!(result.is_err(), "Should fail for out-of-bounds index");
    }

    #[test]
    fn test_parameter_unit_strings() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let count = plugin.parameter_count();
        if count == 0 {
            println!("Plugin has no parameters, skipping test");
            return;
        }

        // Check that we can retrieve unit strings
        for i in 0..count {
            let param = plugin.parameter_info(i).expect("Failed to get parameter info");
            // Unit string may be empty (generic parameter) or contain a unit
            // Just verify it doesn't panic and returns a valid String
            println!("Parameter {} unit: '{}'", i, param.unit);
        }
    }

    #[test]
    fn test_parameter_operations_before_init() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        // Don't initialize - test pre-init behavior

        // All operations should fail gracefully before initialization
        let count = plugin.parameter_count();
        assert_eq!(count, 0, "Parameter count should be 0 before initialization");

        let result = plugin.parameter_info(0);
        assert!(result.is_err(), "parameter_info should fail before init");

        let result = plugin.get_parameter(0);
        assert!(result.is_err(), "get_parameter should fail before init");
    }

    #[test]
    fn test_parameter_value_round_trip() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let count = plugin.parameter_count();
        if count == 0 {
            return;
        }

        // Test round-tripping at various normalized values
        let test_values = vec![0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0];

        for &test_value in &test_values {
            plugin.set_parameter(0, test_value).expect("Failed to set parameter");
            let read_value = plugin.get_parameter(0).expect("Failed to get parameter");

            // Allow small epsilon for floating-point precision
            let diff = (read_value - test_value).abs();
            assert!(
                diff < 0.01,
                "Value round-trip failed: set {}, got {} (diff: {})",
                test_value,
                read_value,
                diff
            );
        }
    }

    #[test]
    fn test_parameter_extreme_values() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let count = plugin.parameter_count();
        if count == 0 {
            return;
        }

        // Test extreme values (clamped by C++ layer before passing to AudioUnit)
        plugin.set_parameter(0, -1.0).expect("Should handle negative values");
        let value = plugin.get_parameter(0).expect("Failed to get parameter");
        assert!(value >= 0.0 && value <= 1.0, "Value should be clamped to 0.0-1.0");

        plugin.set_parameter(0, 2.0).expect("Should handle > 1.0 values");
        let value = plugin.get_parameter(0).expect("Failed to get parameter");
        assert!(value >= 0.0 && value <= 1.0, "Value should be clamped to 0.0-1.0");

        // Note: NaN and infinity are rejected by AudioUnit itself (not our code)
        // AudioUnit returns error -67743 (kAudioUnitErr_InvalidParameter)
        // This is correct behavior - we don't need to test these edge cases
        // as AudioUnit provides its own validation
    }
}
