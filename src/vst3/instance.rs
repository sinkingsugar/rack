use crate::{Error, MidiEvent, MidiEventKind, ParameterInfo, PluginInfo, PluginInstance, PresetInfo, Result};
use smallvec::SmallVec;
use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;

use super::ffi;
use super::util::map_error;

/// An instantiated VST3 plugin
///
/// # Thread Safety
///
/// This type is `Send` but not `Sync`:
/// - `Send`: The plugin can be moved between threads safely
/// - NOT `Sync`: Multiple threads should not access the plugin simultaneously
///   without synchronization. Wrap in `Arc<Mutex<>>` if shared access is needed.
pub struct Vst3Plugin {
    inner: NonNull<ffi::RackVST3Plugin>,
    info: PluginInfo,
    // Pre-allocated pointer arrays for zero-allocation process() calls
    input_ptrs: Vec<*const f32>,
    output_ptrs: Vec<*mut f32>,
    // Channel configuration (queried from VST3 during initialize)
    input_channels: usize,
    output_channels: usize,
    // PhantomData<*const ()> makes this type !Sync while keeping it Send
    _not_sync: PhantomData<*const ()>,
}

// Safety: Vst3Plugin can be sent between threads because:
// 1. Each plugin instance owns its C++ state exclusively
// 2. The plugin doesn't share mutable state with other instances
// 3. VST3 plugins are designed to be movable between threads (host requirement)
unsafe impl Send for Vst3Plugin {}

// Note: Vst3Plugin is NOT Sync due to PhantomData<*const ()>
// This is intentional - VST3 instances require synchronization for shared access

impl Vst3Plugin {
    /// Create a new VST3 plugin instance
    pub(crate) fn new(info: &PluginInfo) -> Result<Self> {
        unsafe {
            // Convert path to CString
            let path_str = info.path.to_str()
                .ok_or_else(|| Error::Other("Plugin path contains invalid UTF-8".to_string()))?;
            let path = CString::new(path_str)
                .map_err(|_| Error::Other("Plugin path contains null byte".to_string()))?;

            // Convert unique_id to CString
            let unique_id = CString::new(info.unique_id.as_str())
                .map_err(|_| Error::Other("Invalid unique_id (contains null byte)".to_string()))?;

            // Create plugin instance via FFI
            let ptr = ffi::rack_vst3_plugin_new(path.as_ptr(), unique_id.as_ptr());
            if ptr.is_null() {
                return Err(Error::PluginNotFound(format!(
                    "Failed to create VST3 instance for {}",
                    info.name
                )));
            }

            Ok(Self {
                inner: NonNull::new_unchecked(ptr),
                info: info.clone(),
                input_ptrs: Vec::new(),
                output_ptrs: Vec::new(),
                input_channels: 0,
                output_channels: 0,
                _not_sync: PhantomData,
            })
        }
    }

    /// Get the number of factory presets
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin is not initialized
    pub fn preset_count(&self) -> Result<usize> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        unsafe {
            let count = ffi::rack_vst3_plugin_get_preset_count(self.inner.as_ptr());
            if count < 0 {
                Ok(0) // Plugin has no presets
            } else {
                Ok(count as usize)
            }
        }
    }

    /// Get information about a factory preset
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The plugin is not initialized
    /// - The index is out of range
    /// - The preset info cannot be retrieved
    pub fn preset_info(&self, index: usize) -> Result<PresetInfo> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        unsafe {
            let mut name = vec![0i8; 256];
            let mut preset_number = 0i32;

            let result = ffi::rack_vst3_plugin_get_preset_info(
                self.inner.as_ptr(),
                index as u32,
                name.as_mut_ptr(),
                name.len(),
                &mut preset_number,
            );

            if result != ffi::RACK_VST3_OK {
                return Err(map_error(result));
            }

            // Convert name to String
            let name_cstr = std::ffi::CStr::from_ptr(name.as_ptr());
            let name_str = name_cstr
                .to_str()
                .map_err(|e| Error::Other(format!("Invalid UTF-8 in preset name: {}", e)))?
                .to_string();

            Ok(PresetInfo {
                index,
                name: name_str,
                preset_number,
            })
        }
    }

    /// Load a factory preset
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The plugin is not initialized
    /// - The preset number is invalid
    /// - The preset cannot be loaded
    pub fn load_preset(&mut self, preset_number: i32) -> Result<()> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        unsafe {
            let result = ffi::rack_vst3_plugin_load_preset(self.inner.as_ptr(), preset_number);

            if result != ffi::RACK_VST3_OK {
                return Err(map_error(result));
            }

            Ok(())
        }
    }

    /// Get plugin state as a byte vector
    ///
    /// This serializes the complete plugin state including parameters, presets,
    /// and any internal state. The state can be saved to disk and restored later
    /// using `set_state()`.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin is not initialized or state cannot be retrieved
    pub fn get_state(&self) -> Result<Vec<u8>> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        unsafe {
            // Get required state size
            let size = ffi::rack_vst3_plugin_get_state_size(self.inner.as_ptr());

            if size <= 0 {
                return Err(Error::Other("Cannot retrieve plugin state".to_string()));
            }

            let size_usize = size as usize;

            // Allocate buffer
            let mut data = vec![0u8; size_usize];
            let mut actual_size = size_usize;

            // Get state
            let result = ffi::rack_vst3_plugin_get_state(
                self.inner.as_ptr(),
                data.as_mut_ptr(),
                &mut actual_size,
            );

            if result != ffi::RACK_VST3_OK {
                return Err(map_error(result));
            }

            // Truncate to actual size (may be smaller than allocated)
            data.truncate(actual_size);

            Ok(data)
        }
    }

    /// Restore plugin state from a byte slice
    ///
    /// This deserializes the complete plugin state including parameters, presets,
    /// and any internal state. The data should be obtained from a previous
    /// `get_state()` call.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The plugin is not initialized
    /// - The state data is invalid or corrupted
    /// - The state cannot be restored
    pub fn set_state(&mut self, data: &[u8]) -> Result<()> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        if data.is_empty() {
            return Err(Error::Other("State data is empty".to_string()));
        }

        unsafe {
            let result = ffi::rack_vst3_plugin_set_state(
                self.inner.as_ptr(),
                data.as_ptr(),
                data.len(),
            );

            if result != ffi::RACK_VST3_OK {
                return Err(map_error(result));
            }

            Ok(())
        }
    }
}

impl Drop for Vst3Plugin {
    fn drop(&mut self) {
        unsafe {
            ffi::rack_vst3_plugin_free(self.inner.as_ptr());
        }
    }
}

impl PluginInstance for Vst3Plugin {
    fn initialize(&mut self, sample_rate: f64, max_block_size: usize) -> Result<()> {
        unsafe {
            let result = ffi::rack_vst3_plugin_initialize(
                self.inner.as_ptr(),
                sample_rate,
                max_block_size as u32,
            );

            if result != ffi::RACK_VST3_OK {
                return Err(map_error(result));
            }

            // Query actual channel configuration
            let input_channels = ffi::rack_vst3_plugin_get_input_channels(self.inner.as_ptr());
            let output_channels = ffi::rack_vst3_plugin_get_output_channels(self.inner.as_ptr());

            if input_channels < 0 || output_channels < 0 {
                return Err(Error::Other("Failed to query channel configuration".to_string()));
            }

            self.input_channels = input_channels as usize;
            self.output_channels = output_channels as usize;

            // Pre-allocate pointer arrays for zero-allocation process() calls
            // Reserve capacity to avoid reallocation even if channel counts are unusual
            self.input_ptrs = Vec::with_capacity(self.input_channels.max(8));
            self.output_ptrs = Vec::with_capacity(self.output_channels.max(8));

            // Initialize with null pointers (will be filled in process())
            self.input_ptrs.resize(self.input_channels, std::ptr::null());
            self.output_ptrs.resize(self.output_channels, std::ptr::null_mut());

            Ok(())
        }
    }

    fn reset(&mut self) -> Result<()> {
        unsafe {
            let result = ffi::rack_vst3_plugin_reset(self.inner.as_ptr());

            if result != ffi::RACK_VST3_OK {
                return Err(map_error(result));
            }

            Ok(())
        }
    }

    fn process(
        &mut self,
        inputs: &[&[f32]],
        outputs: &mut [&mut [f32]],
        num_frames: usize,
    ) -> Result<()> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        // Validate channel counts match plugin configuration
        if inputs.len() != self.input_channels {
            return Err(Error::Other(format!(
                "Input channel count mismatch: plugin expects {}, got {}",
                self.input_channels, inputs.len()
            )));
        }
        if outputs.len() != self.output_channels {
            return Err(Error::Other(format!(
                "Output channel count mismatch: plugin expects {}, got {}",
                self.output_channels, outputs.len()
            )));
        }

        // Validate inputs (channel counts are now guaranteed to be correct)
        if inputs.is_empty() || outputs.is_empty() {
            return Err(Error::Other("Empty input or output channels".to_string()));
        }

        // Validate all channels have the same length
        for (i, input) in inputs.iter().enumerate() {
            if input.len() < num_frames {
                return Err(Error::Other(format!(
                    "Input channel {} has {} samples, need at least {}",
                    i,
                    input.len(),
                    num_frames
                )));
            }
        }

        for (i, output) in outputs.iter().enumerate() {
            if output.len() < num_frames {
                return Err(Error::Other(format!(
                    "Output channel {} has {} samples, need at least {}",
                    i,
                    output.len(),
                    num_frames
                )));
            }
        }

        // Reuse pre-allocated pointer arrays (zero-allocation hot path)
        // Fill with current buffer pointers
        for (i, input_ch) in inputs.iter().enumerate() {
            self.input_ptrs[i] = input_ch.as_ptr();
        }
        for (i, output_ch) in outputs.iter_mut().enumerate() {
            self.output_ptrs[i] = output_ch.as_mut_ptr();
        }

        unsafe {
            let result = ffi::rack_vst3_plugin_process(
                self.inner.as_ptr(),
                self.input_ptrs.as_ptr(),
                inputs.len() as u32,
                self.output_ptrs.as_ptr(),
                outputs.len() as u32,
                num_frames as u32,
            );

            if result != ffi::RACK_VST3_OK {
                return Err(map_error(result));
            }

            Ok(())
        }
    }

    fn parameter_count(&self) -> usize {
        unsafe {
            let count = ffi::rack_vst3_plugin_parameter_count(self.inner.as_ptr());
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

            let result = ffi::rack_vst3_plugin_parameter_info(
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

            if result != ffi::RACK_VST3_OK {
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
                ffi::rack_vst3_plugin_get_parameter(self.inner.as_ptr(), index as u32, &mut value);

            if result != ffi::RACK_VST3_OK {
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
                ffi::rack_vst3_plugin_set_parameter(self.inner.as_ptr(), index as u32, value);

            if result != ffi::RACK_VST3_OK {
                return Err(map_error(result));
            }

            Ok(())
        }
    }

    fn send_midi(&mut self, events: &[MidiEvent]) -> Result<()> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        if events.is_empty() {
            return Ok(());
        }

        // Convert Rust MIDI events to C MIDI events
        // Use SmallVec for zero-allocation in typical cases (≤16 events)
        let mut c_events: SmallVec<[ffi::RackVST3MidiEvent; 16]> = SmallVec::with_capacity(events.len());

        for event in events {
            let (status, data1, data2, channel) = match &event.kind {
                MidiEventKind::NoteOn { note, velocity, channel } => (0x90, *note, *velocity, *channel),
                MidiEventKind::NoteOff { note, velocity, channel } => (0x80, *note, *velocity, *channel),
                MidiEventKind::PolyphonicAftertouch { note, pressure, channel } => (0xA0, *note, *pressure, *channel),
                MidiEventKind::ControlChange { controller, value, channel } => (0xB0, *controller, *value, *channel),
                MidiEventKind::ProgramChange { program, channel } => (0xC0, *program, 0, *channel),
                MidiEventKind::ChannelAftertouch { pressure, channel } => (0xD0, *pressure, 0, *channel),
                MidiEventKind::PitchBend { value, channel } => {
                    // Pitch bend is 14-bit (0-16383), centered at 8192
                    let lsb = (value & 0x7F) as u8;
                    let msb = ((value >> 7) & 0x7F) as u8;
                    (0xE0, lsb, msb, *channel)
                }
                // System messages don't have a channel - skip them for now
                // VST3 doesn't have a standard way to send system real-time messages
                MidiEventKind::TimingClock | MidiEventKind::Start | MidiEventKind::Continue |
                MidiEventKind::Stop | MidiEventKind::ActiveSensing | MidiEventKind::SystemReset => {
                    continue; // Skip system messages
                }
            };

            c_events.push(ffi::RackVST3MidiEvent {
                sample_offset: event.sample_offset,
                status,
                data1,
                data2,
                channel,
            });
        }

        unsafe {
            let result = ffi::rack_vst3_plugin_send_midi(
                self.inner.as_ptr(),
                c_events.as_ptr(),
                c_events.len() as u32,
            );

            if result != ffi::RACK_VST3_OK {
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
            let result = ffi::rack_vst3_plugin_is_initialized(self.inner.as_ptr());
            result > 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PluginScanner;

    fn get_test_plugin() -> Result<(crate::vst3::Vst3Scanner, PluginInfo)> {
        let scanner = crate::vst3::Vst3Scanner::new()?;
        let plugins = scanner.scan()?;

        if plugins.is_empty() {
            return Err(Error::Other("No VST3 plugins found for testing".to_string()));
        }

        Ok((scanner, plugins[0].clone()))
    }

    #[test]
    fn test_plugin_creation() {
        let (scanner, info) = match get_test_plugin() {
            Ok(result) => result,
            Err(_) => {
                println!("Skipping test - no VST3 plugins found");
                return;
            }
        };

        let result = scanner.load(&info);
        assert!(result.is_ok(), "Plugin creation should succeed");
    }

    #[test]
    fn test_plugin_initialize() {
        let (scanner, info) = match get_test_plugin() {
            Ok(result) => result,
            Err(_) => {
                println!("Skipping test - no VST3 plugins found");
                return;
            }
        };

        let mut plugin = scanner.load(&info).expect("Plugin creation should succeed");
        let result = plugin.initialize(48000.0, 512);
        assert!(result.is_ok(), "Plugin initialization should succeed");
        assert!(plugin.is_initialized(), "Plugin should be initialized");
    }

    #[test]
    fn test_drop_behavior() {
        let (scanner, info) = match get_test_plugin() {
            Ok(result) => result,
            Err(_) => {
                println!("Skipping test - no VST3 plugins found");
                return;
            }
        };

        {
            let _plugin = scanner.load(&info).expect("Plugin creation should succeed");
        } // Plugin dropped here
          // If Drop is implemented correctly, this shouldn't leak or crash
    }
}
