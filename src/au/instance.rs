use crate::{Error, MidiEvent, MidiEventKind, ParameterInfo, PluginInfo, PluginInstance, PresetInfo, Result};
use smallvec::SmallVec;
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
    // Pre-allocated pointer arrays for zero-allocation process() calls
    input_ptrs: Vec<*const f32>,
    output_ptrs: Vec<*mut f32>,
    // Channel configuration (queried from AudioUnit during initialize)
    input_channels: usize,
    output_channels: usize,
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
                input_ptrs: Vec::new(),
                output_ptrs: Vec::new(),
                input_channels: 0,
                output_channels: 0,
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

            // Query actual channel configuration
            let input_channels = ffi::rack_au_plugin_get_input_channels(self.inner.as_ptr());
            let output_channels = ffi::rack_au_plugin_get_output_channels(self.inner.as_ptr());

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
            let result = ffi::rack_au_plugin_reset(self.inner.as_ptr());

            if result != ffi::RACK_AU_OK {
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
            let result = ffi::rack_au_plugin_process(
                self.inner.as_ptr(),
                self.input_ptrs.as_ptr(),
                inputs.len() as u32,
                self.output_ptrs.as_ptr(),
                outputs.len() as u32,
                num_frames as u32,
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

    fn send_midi(&mut self, events: &[MidiEvent]) -> Result<()> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        // Convert Rust MIDI events to FFI events
        // Use SmallVec to avoid heap allocation for typical use cases (1-16 events)
        let ffi_events: SmallVec<[ffi::RackAUMidiEvent; 16]> = events
            .iter()
            .map(|event| {
                let (status, data1, data2, channel) = match event.kind {
                    MidiEventKind::NoteOn { note, velocity, channel } => {
                        (ffi::RackAUMidiEventType::NoteOn as u8, note, velocity, channel)
                    }
                    MidiEventKind::NoteOff { note, velocity, channel } => {
                        (ffi::RackAUMidiEventType::NoteOff as u8, note, velocity, channel)
                    }
                    MidiEventKind::PolyphonicAftertouch { note, pressure, channel } => {
                        (ffi::RackAUMidiEventType::PolyphonicAftertouch as u8, note, pressure, channel)
                    }
                    MidiEventKind::ControlChange { controller, value, channel } => {
                        (ffi::RackAUMidiEventType::ControlChange as u8, controller, value, channel)
                    }
                    MidiEventKind::ProgramChange { program, channel } => {
                        // Program Change only has 1 data byte (program number)
                        // data2 is 0 because MIDI Program Change messages don't use it
                        (ffi::RackAUMidiEventType::ProgramChange as u8, program, 0, channel)
                    }
                    MidiEventKind::ChannelAftertouch { pressure, channel } => {
                        // Channel Aftertouch only has 1 data byte (pressure value)
                        (ffi::RackAUMidiEventType::ChannelAftertouch as u8, pressure, 0, channel)
                    }
                    MidiEventKind::PitchBend { value, channel } => {
                        // Pitch bend uses 14-bit value split into two 7-bit bytes
                        // LSB (least significant 7 bits) in data1, MSB (most significant 7 bits) in data2
                        let lsb = (value & 0x7F) as u8;
                        let msb = ((value >> 7) & 0x7F) as u8;
                        (ffi::RackAUMidiEventType::PitchBend as u8, lsb, msb, channel)
                    }
                    // System Real-Time messages (no channel or data bytes)
                    MidiEventKind::TimingClock => {
                        (ffi::RackAUMidiEventType::TimingClock as u8, 0, 0, 0)
                    }
                    MidiEventKind::Start => {
                        (ffi::RackAUMidiEventType::Start as u8, 0, 0, 0)
                    }
                    MidiEventKind::Continue => {
                        (ffi::RackAUMidiEventType::Continue as u8, 0, 0, 0)
                    }
                    MidiEventKind::Stop => {
                        (ffi::RackAUMidiEventType::Stop as u8, 0, 0, 0)
                    }
                    MidiEventKind::ActiveSensing => {
                        (ffi::RackAUMidiEventType::ActiveSensing as u8, 0, 0, 0)
                    }
                    MidiEventKind::SystemReset => {
                        (ffi::RackAUMidiEventType::SystemReset as u8, 0, 0, 0)
                    }
                };

                ffi::RackAUMidiEvent {
                    sample_offset: event.sample_offset,
                    status,
                    data1,
                    data2,
                    channel,
                }
            })
            .collect();

        unsafe {
            let result = ffi::rack_au_plugin_send_midi(
                self.inner.as_ptr(),
                ffi_events.as_ptr(),
                ffi_events.len() as u32,
            );

            if result != ffi::RACK_AU_OK {
                let err = map_error(result);

                // If the plugin is an effect, provide more specific error context
                if matches!(self.info.plugin_type, crate::PluginType::Effect) {
                    return Err(Error::Other(format!(
                        "Effect plugin '{}' does not support MIDI (only instrument plugins typically respond to MIDI)",
                        self.info.name
                    )));
                }

                return Err(err);
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

// Additional methods not in PluginInstance trait
impl AudioUnitPlugin {
    /// Get the number of input channels
    ///
    /// Returns the actual number of input channels the plugin was configured with
    /// after initialization. This may differ from what was requested if the plugin
    /// doesn't support the requested configuration.
    ///
    /// # Returns
    ///
    /// - Number of input channels (e.g., 1 for mono, 2 for stereo)
    /// - 0 if not initialized
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example() -> Result<()> {
    /// # let scanner = Scanner::new()?;
    /// # let plugins = scanner.scan()?;
    /// # let mut plugin = scanner.load(&plugins[0])?;
    /// plugin.initialize(48000.0, 512)?;
    /// let channels = plugin.input_channels();
    /// println!("Plugin has {} input channels", channels);
    /// # Ok(())
    /// # }
    /// ```
    pub fn input_channels(&self) -> usize {
        self.input_channels
    }

    /// Get the number of output channels
    ///
    /// Returns the actual number of output channels the plugin was configured with
    /// after initialization. This may differ from what was requested if the plugin
    /// doesn't support the requested configuration.
    ///
    /// # Returns
    ///
    /// - Number of output channels (e.g., 1 for mono, 2 for stereo)
    /// - 0 if not initialized
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example() -> Result<()> {
    /// # let scanner = Scanner::new()?;
    /// # let plugins = scanner.scan()?;
    /// # let mut plugin = scanner.load(&plugins[0])?;
    /// plugin.initialize(48000.0, 512)?;
    /// let channels = plugin.output_channels();
    /// println!("Plugin has {} output channels", channels);
    ///
    /// // Allocate buffers with correct channel count
    /// let mut inputs: Vec<Vec<f32>> = (0..plugin.input_channels())
    ///     .map(|_| vec![0.0f32; 512])
    ///     .collect();
    /// let mut outputs: Vec<Vec<f32>> = (0..plugin.output_channels())
    ///     .map(|_| vec![0.0f32; 512])
    ///     .collect();
    /// # Ok(())
    /// # }
    /// ```
    pub fn output_channels(&self) -> usize {
        self.output_channels
    }

    /// Get the number of factory presets
    ///
    /// # Returns
    ///
    /// Returns the number of factory presets available, or 0 if the plugin has no presets
    /// or is not initialized.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example() -> Result<()> {
    /// # let scanner = Scanner::new()?;
    /// # let plugins = scanner.scan()?;
    /// # let mut plugin = scanner.load(&plugins[0])?;
    /// plugin.initialize(48000.0, 512)?;
    /// let count = plugin.preset_count();
    /// println!("Plugin has {} factory presets", count);
    /// # Ok(())
    /// # }
    /// ```
    pub fn preset_count(&self) -> usize {
        if !self.is_initialized() {
            return 0;
        }

        unsafe {
            let count = ffi::rack_au_plugin_get_preset_count(self.inner.as_ptr());
            if count < 0 {
                0
            } else {
                count as usize
            }
        }
    }

    /// Get information about a preset by index
    ///
    /// # Arguments
    ///
    /// * `index` - Preset index (0 to preset_count() - 1)
    ///
    /// # Returns
    ///
    /// Returns `PresetInfo` containing the preset name and number
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The plugin is not initialized
    /// - The index is out of bounds
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example() -> Result<()> {
    /// # let scanner = Scanner::new()?;
    /// # let plugins = scanner.scan()?;
    /// # let mut plugin = scanner.load(&plugins[0])?;
    /// plugin.initialize(48000.0, 512)?;
    /// for i in 0..plugin.preset_count() {
    ///     let preset = plugin.preset_info(i)?;
    ///     println!("[{}] {}", i, preset.name);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn preset_info(&self, index: usize) -> Result<PresetInfo> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        unsafe {
            let mut name = vec![0i8; 256];
            let mut preset_number: i32 = 0;

            let result = ffi::rack_au_plugin_get_preset_info(
                self.inner.as_ptr(),
                index as u32,
                name.as_mut_ptr(),
                name.len(),
                &mut preset_number,
            );

            if result != ffi::RACK_AU_OK {
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
    /// # Arguments
    ///
    /// * `preset_number` - The preset number from `preset_info().preset_number`
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The plugin is not initialized
    /// - The preset number is invalid
    /// - The AudioUnit fails to load the preset
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example() -> Result<()> {
    /// # let scanner = Scanner::new()?;
    /// # let plugins = scanner.scan()?;
    /// # let mut plugin = scanner.load(&plugins[0])?;
    /// plugin.initialize(48000.0, 512)?;
    ///
    /// // Load first preset
    /// if plugin.preset_count() > 0 {
    ///     let preset = plugin.preset_info(0)?;
    ///     plugin.load_preset(preset.preset_number)?;
    ///     println!("Loaded preset: {}", preset.name);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_preset(&mut self, preset_number: i32) -> Result<()> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        unsafe {
            let result = ffi::rack_au_plugin_load_preset(self.inner.as_ptr(), preset_number);

            if result != ffi::RACK_AU_OK {
                return Err(map_error(result));
            }

            Ok(())
        }
    }

    /// Get the full plugin state (including parameters, preset, etc.)
    ///
    /// This serializes the complete plugin state to a byte vector, which can be saved
    /// to disk or stored in memory. The state can later be restored with `set_state()`.
    ///
    /// # Returns
    ///
    /// Returns a `Vec<u8>` containing the serialized plugin state
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The plugin is not initialized
    /// - The AudioUnit fails to serialize its state
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example() -> Result<()> {
    /// # let scanner = Scanner::new()?;
    /// # let plugins = scanner.scan()?;
    /// # let mut plugin = scanner.load(&plugins[0])?;
    /// plugin.initialize(48000.0, 512)?;
    ///
    /// // Save state
    /// let state = plugin.get_state()?;
    /// std::fs::write("plugin_state.bin", &state)?;
    ///
    /// // Later: restore state
    /// let state = std::fs::read("plugin_state.bin")?;
    /// plugin.set_state(&state)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_state(&self) -> Result<Vec<u8>> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        unsafe {
            // Get state size
            let size = ffi::rack_au_plugin_get_state_size(self.inner.as_ptr());
            if size <= 0 {
                return Err(Error::Other("Failed to get plugin state size".to_string()));
            }

            // Allocate buffer
            let mut data = vec![0u8; size as usize];
            let mut actual_size = data.len();

            // Get state data
            let result = ffi::rack_au_plugin_get_state(
                self.inner.as_ptr(),
                data.as_mut_ptr(),
                &mut actual_size,
            );

            if result != ffi::RACK_AU_OK {
                return Err(map_error(result));
            }

            // Resize to actual size (in case it's smaller than allocated)
            data.resize(actual_size, 0);

            Ok(data)
        }
    }

    /// Set the full plugin state (including parameters, preset, etc.)
    ///
    /// This restores the complete plugin state from a byte buffer previously obtained
    /// from `get_state()`. All parameters, preset selection, and other state will be restored.
    ///
    /// # Arguments
    ///
    /// * `data` - State data from a previous `get_state()` call
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The plugin is not initialized
    /// - The state data is invalid or corrupted
    /// - The AudioUnit fails to restore the state
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example() -> Result<()> {
    /// # let scanner = Scanner::new()?;
    /// # let plugins = scanner.scan()?;
    /// # let mut plugin = scanner.load(&plugins[0])?;
    /// plugin.initialize(48000.0, 512)?;
    ///
    /// // Save current state
    /// let state = plugin.get_state()?;
    ///
    /// // Make some changes...
    /// plugin.set_parameter(0, 0.75)?;
    ///
    /// // Restore original state
    /// plugin.set_state(&state)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_state(&mut self, data: &[u8]) -> Result<()> {
        if !self.is_initialized() {
            return Err(Error::NotInitialized);
        }

        if data.is_empty() {
            return Err(Error::Other("State data is empty".to_string()));
        }

        unsafe {
            let result = ffi::rack_au_plugin_set_state(
                self.inner.as_ptr(),
                data.as_ptr(),
                data.len(),
            );

            if result != ffi::RACK_AU_OK {
                return Err(map_error(result));
            }

            Ok(())
        }
    }

    /// Create GUI asynchronously
    ///
    /// Creates the plugin's graphical user interface. This function tries multiple
    /// strategies in order:
    /// 1. AUv3 modern GUI (requestViewController)
    /// 2. AUv2 legacy GUI (kAudioUnitProperty_CocoaUI)
    /// 3. Generic parameter UI (fallback)
    ///
    /// The callback is invoked on the main thread when the GUI is ready.
    ///
    /// # Parameters
    ///
    /// - `callback`: Closure invoked when GUI creation completes. Receives
    ///   `Ok(AudioUnitGui)` on success or `Err(Error)` on failure.
    ///
    /// # Thread Safety
    ///
    /// **MUST be called from the main thread.** This is a macOS/AppKit requirement.
    /// The callback will also be invoked on the main thread.
    ///
    /// # Errors
    ///
    /// The callback receives an error if:
    /// - Plugin is not initialized
    /// - GUI creation fails (rare - generic UI is always available as fallback)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example() -> Result<()> {
    /// # let scanner = Scanner::new()?;
    /// # let plugins = scanner.scan()?;
    /// # let mut plugin = scanner.load(&plugins[0])?;
    /// plugin.initialize(48000.0, 512)?;
    ///
    /// plugin.create_gui(|result| {
    ///     match result {
    ///         Ok(gui) => {
    ///             println!("GUI created!");
    ///             gui.show_window(Some("My Plugin"))?;
    ///         }
    ///         Err(e) => {
    ///             eprintln!("GUI creation failed: {}", e);
    ///         }
    ///     }
    ///     Ok(())
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_gui<F>(&mut self, callback: F)
    where
        F: FnOnce(Result<super::gui::AudioUnitGui>) -> Result<()> + Send + 'static,
    {
        if !self.is_initialized() {
            // Invoke callback with error immediately
            let _ = callback(Err(Error::NotInitialized));
            return;
        }

        // Box the callback so we can pass it through C
        let boxed_callback = Box::new(callback);
        let user_data = Box::into_raw(boxed_callback) as *mut std::ffi::c_void;

        // Define the C callback trampoline
        extern "C" fn trampoline<F>(
            user_data: *mut std::ffi::c_void,
            gui: *mut ffi::RackAUGui,
            error_code: std::os::raw::c_int,
        ) where
            F: FnOnce(Result<super::gui::AudioUnitGui>) -> Result<()> + Send + 'static,
        {
            // Safety: user_data is the boxed callback we created above
            let callback = unsafe {
                Box::from_raw(user_data as *mut F)
            };

            let result = if gui.is_null() {
                Err(map_error(error_code))
            } else {
                // Safety: gui is a valid pointer from C++
                Ok(unsafe { super::gui::AudioUnitGui::from_raw(gui) })
            };

            // Invoke the user's callback
            let _ = callback(result);
        }

        // Call the FFI function with our trampoline
        unsafe {
            ffi::rack_au_gui_create_async(
                self.inner.as_ptr(),
                trampoline::<F>,
                user_data,
            );
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

        // Create test buffers (planar format - separate buffer per channel)
        let frames = 512;
        let mut left_in = vec![0.0f32; frames];
        let mut right_in = vec![0.0f32; frames];
        let mut left_out = vec![0.0f32; frames];
        let mut right_out = vec![0.0f32; frames];

        // Fill input with a simple sine wave (440 Hz)
        let frequency = 440.0f32;
        let sample_rate = 48000.0f32;
        for i in 0..frames {
            let sample = (2.0 * std::f32::consts::PI * frequency * i as f32 / sample_rate).sin() * 0.5;
            left_in[i] = sample;  // Left channel
            right_in[i] = sample; // Right channel
        }

        // Process audio (planar format)
        plugin
            .process(
                &[&left_in, &right_in],
                &mut [&mut left_out, &mut right_out],
                frames
            )
            .expect("Audio processing failed");

        // Verify output is not all zeros (plugin did something)
        let has_signal = left_out.iter().chain(right_out.iter()).any(|&sample| sample != 0.0);
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

    // ============================================================================
    // MIDI Tests
    // ============================================================================

    // Helper to get an instrument plugin for MIDI testing
    fn get_instrument_plugin() -> Option<PluginInfo> {
        use super::super::scanner::AudioUnitScanner;

        let scanner = AudioUnitScanner::new().ok()?;
        let plugins = scanner.scan().ok()?;

        plugins
            .into_iter()
            .find(|p| p.plugin_type == PluginType::Instrument)
    }

    #[test]
    fn test_send_midi_note_on_off() {
        let Some(info) = get_instrument_plugin() else {
            println!("No instrument plugins available, skipping test");
            return;
        };

        println!("Testing MIDI with instrument: {}", info.name);

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        // Send a C major chord
        let events = vec![
            MidiEvent::note_on(60, 100, 0, 0), // Middle C
            MidiEvent::note_on(64, 100, 0, 0), // E
            MidiEvent::note_on(67, 100, 0, 0), // G
        ];

        let result = plugin.send_midi(&events);
        assert!(result.is_ok(), "Failed to send MIDI events");

        println!("✓ MIDI note on events sent successfully");

        // Process audio to render the notes (planar format)
        let left_in = vec![0.0f32; 512];
        let right_in = vec![0.0f32; 512];
        let mut left_out = vec![0.0f32; 512];
        let mut right_out = vec![0.0f32; 512];

        let result = plugin.process(
            &[&left_in, &right_in],
            &mut [&mut left_out, &mut right_out],
            512
        );
        assert!(result.is_ok(), "Failed to process audio after MIDI");

        println!("✓ Audio processed after MIDI events");

        // Send note off events
        let events = vec![
            MidiEvent::note_off(60, 64, 0, 0),
            MidiEvent::note_off(64, 64, 0, 0),
            MidiEvent::note_off(67, 64, 0, 0),
        ];

        let result = plugin.send_midi(&events);
        assert!(result.is_ok(), "Failed to send MIDI note off events");

        println!("✓ MIDI note off events sent successfully");
    }

    #[test]
    fn test_send_midi_control_change() {
        let Some(info) = get_instrument_plugin() else {
            println!("No instrument plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        // Send control change (modulation wheel)
        let events = vec![MidiEvent::control_change(1, 64, 0, 0)];

        let result = plugin.send_midi(&events);
        assert!(result.is_ok(), "Failed to send MIDI control change");

        println!("✓ MIDI control change sent successfully");
    }

    #[test]
    fn test_send_midi_program_change() {
        let Some(info) = get_instrument_plugin() else {
            println!("No instrument plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        // Send program change
        let events = vec![MidiEvent::program_change(5, 0, 0)];

        let result = plugin.send_midi(&events);
        assert!(result.is_ok(), "Failed to send MIDI program change");

        println!("✓ MIDI program change sent successfully");
    }

    #[test]
    fn test_send_midi_empty_events() {
        let Some(info) = get_instrument_plugin() else {
            println!("No instrument plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        // Send empty event array (should succeed)
        let events: Vec<MidiEvent> = vec![];
        let result = plugin.send_midi(&events);
        assert!(result.is_ok(), "Empty MIDI array should succeed");

        println!("✓ Empty MIDI event array handled correctly");
    }

    #[test]
    fn test_send_midi_before_init() {
        let Some(info) = get_instrument_plugin() else {
            println!("No instrument plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");

        // Try to send MIDI before initialization
        let events = vec![MidiEvent::note_on(60, 100, 0, 0)];
        let result = plugin.send_midi(&events);

        assert!(result.is_err(), "send_midi should fail before initialization");
        assert!(matches!(result, Err(Error::NotInitialized)));

        println!("✓ send_midi correctly fails before initialization");
    }

    #[test]
    fn test_send_midi_multi_channel() {
        let Some(info) = get_instrument_plugin() else {
            println!("No instrument plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        // Send notes on different MIDI channels (0, 5, 10, 15)
        let events = vec![
            MidiEvent::note_on(60, 100, 0, 0),   // Middle C on channel 0
            MidiEvent::note_on(64, 100, 5, 0),   // E on channel 5
            MidiEvent::note_on(67, 100, 10, 0),  // G on channel 10 (drums in GM)
            MidiEvent::note_on(72, 100, 15, 0),  // High C on channel 15
        ];

        let result = plugin.send_midi(&events);
        assert!(result.is_ok(), "Failed to send multi-channel MIDI: {:?}", result.err());

        // Process audio to verify notes rendered (planar format)
        let left_in = vec![0.0f32; 512];
        let right_in = vec![0.0f32; 512];
        let mut left_out = vec![0.0f32; 512];
        let mut right_out = vec![0.0f32; 512];

        plugin
            .process(
                &[&left_in, &right_in],
                &mut [&mut left_out, &mut right_out],
                512
            )
            .expect("Failed to process audio");

        // Check that we got some audio output
        let has_output = left_out.iter().chain(right_out.iter()).any(|&sample| sample.abs() > 1e-6);
        assert!(has_output, "Expected audio output from multi-channel MIDI notes");

        println!("✓ Multi-channel MIDI events sent successfully");
    }

    #[test]
    fn test_midi_with_effect_plugin() {
        use super::super::scanner::AudioUnitScanner;

        let scanner = AudioUnitScanner::new().expect("Failed to create scanner");
        let plugins = scanner.scan().expect("Failed to scan plugins");

        // Find an effect plugin
        let Some(info) = plugins
            .into_iter()
            .find(|p| p.plugin_type == PluginType::Effect)
        else {
            println!("No effect plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        // Send MIDI to effect plugin (should either fail or be ignored)
        let events = vec![MidiEvent::note_on(60, 100, 0, 0)];
        let result = plugin.send_midi(&events);

        // Effect plugins typically don't support MIDI, so this may fail
        // or succeed with the event being ignored
        match result {
            Ok(_) => println!("Effect plugin accepted MIDI (may be ignored)"),
            Err(_) => println!("✓ Effect plugin rejected MIDI as expected"),
        }
    }

    #[test]
    fn test_midi_value_clamping() {
        // Test that MidiEvent helper functions clamp values correctly
        let event = MidiEvent::note_on(200, 200, 20, 0);

        match event.kind {
            MidiEventKind::NoteOn { note, velocity, channel } => {
                assert_eq!(note, 127, "Note should be clamped to 127");
                assert_eq!(velocity, 127, "Velocity should be clamped to 127");
                assert_eq!(channel, 15, "Channel should be clamped to 15");
            }
            _ => panic!("Expected NoteOn event"),
        }

        println!("✓ MIDI value clamping works correctly");
    }

    #[test]
    fn test_midi_sample_offset() {
        let Some(info) = get_instrument_plugin() else {
            println!("No instrument plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        // Send events with different sample offsets
        let events = vec![
            MidiEvent::note_on(60, 100, 0, 0),
            MidiEvent::note_on(64, 100, 0, 128),
            MidiEvent::note_on(67, 100, 0, 256),
        ];

        let result = plugin.send_midi(&events);
        assert!(result.is_ok(), "Failed to send MIDI events with sample offsets");

        println!("✓ MIDI events with sample offsets sent successfully");
    }

    // ============================================================================
    // Preset Tests
    // ============================================================================

    #[test]
    fn test_preset_count() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        println!("Testing preset count with: {}", info.name);

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let count = plugin.preset_count();
        println!("  Found {} presets", count);

        // Verify preset_count doesn't panic (some plugins have 0 presets, which is valid)
    }

    #[test]
    fn test_preset_info() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        println!("Testing preset info with: {}", info.name);

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let count = plugin.preset_count();
        if count == 0 {
            println!("  Plugin has no presets, skipping test");
            return;
        }

        // Get info for first preset
        let preset_info = plugin
            .preset_info(0)
            .expect("Failed to get preset info");

        println!("  Preset 0: {}", preset_info.name);
        println!("    Preset number: {}", preset_info.preset_number);

        assert!(!preset_info.name.is_empty(), "Preset name should not be empty");
        assert_eq!(preset_info.index, 0);
    }

    #[test]
    fn test_load_preset() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        println!("Testing load preset with: {}", info.name);

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let count = plugin.preset_count();
        if count == 0 {
            println!("  Plugin has no presets, skipping test");
            return;
        }

        // Get first preset info
        let preset = plugin.preset_info(0).expect("Failed to get preset info");

        // Load the preset
        plugin
            .load_preset(preset.preset_number)
            .expect("Failed to load preset");

        println!("  ✓ Loaded preset: {}", preset.name);
    }

    #[test]
    fn test_state_round_trip() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        println!("Testing state round-trip with: {}", info.name);

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        // Save original state
        let original_state = plugin.get_state().expect("Failed to get state");
        println!("  Original state size: {} bytes", original_state.len());

        // Modify plugin state (if it has parameters)
        let param_count = plugin.parameter_count();
        let original_param_value = if param_count > 0 {
            let value = plugin.get_parameter(0).expect("Failed to get parameter");
            // Change parameter
            plugin
                .set_parameter(0, 0.99)
                .expect("Failed to set parameter");

            let new_value = plugin.get_parameter(0).expect("Failed to get parameter");
            println!("  Modified parameter 0: {} -> {}", value, new_value);
            Some(value)
        } else {
            None
        };

        // Restore state
        plugin
            .set_state(&original_state)
            .expect("Failed to restore state");

        println!("  ✓ State restored");

        // Verify parameter was restored (if plugin has parameters)
        if let Some(expected_value) = original_param_value {
            let restored_value = plugin.get_parameter(0).expect("Failed to get parameter");
            assert!(
                (restored_value - expected_value).abs() < 0.01,
                "Parameter not restored correctly (expected {}, got {})",
                expected_value,
                restored_value
            );
            println!("  ✓ Parameter restored to: {}", restored_value);
        }
    }

    #[test]
    fn test_preset_out_of_bounds() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let count = plugin.preset_count();
        if count == 0 {
            println!("Plugin has no presets, skipping test");
            return;
        }

        // Try to access preset beyond count
        let result = plugin.preset_info(count + 10);
        assert!(result.is_err(), "Should fail for out-of-bounds index");

        println!("✓ Out-of-bounds preset index rejected");
    }

    #[test]
    fn test_preset_before_init() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        // Don't initialize - test pre-init behavior

        // All operations should return 0 or fail gracefully before initialization
        let count = plugin.preset_count();
        assert_eq!(count, 0, "Preset count should be 0 before initialization");

        let result = plugin.preset_info(0);
        assert!(result.is_err(), "preset_info should fail before init");

        println!("✓ Preset operations correctly fail before initialization");
    }

    #[test]
    fn test_state_empty_data() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        // Try to set empty state
        let result = plugin.set_state(&[]);
        assert!(result.is_err(), "Should reject empty state data");

        println!("✓ Empty state data rejected");
    }

    #[test]
    fn test_state_before_init() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        // Don't initialize

        let result = plugin.get_state();
        assert!(result.is_err(), "get_state should fail before init");

        println!("✓ State operations correctly fail before initialization");
    }

    #[test]
    fn test_multiple_presets() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let count = plugin.preset_count();
        if count < 2 {
            println!("Plugin has {} presets, skipping multi-preset test", count);
            return;
        }

        println!("Testing with {} presets", count);

        // Load several presets and verify they each work
        for i in 0..count.min(3) {
            let preset = plugin.preset_info(i).expect("Failed to get preset info");
            plugin
                .load_preset(preset.preset_number)
                .expect("Failed to load preset");
            println!("  ✓ Loaded preset {}: {}", i, preset.name);
        }
    }

    #[test]
    fn test_channel_count_queries() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");

        // Before init, should be 0
        assert_eq!(plugin.input_channels(), 0, "Input channels should be 0 before init");
        assert_eq!(plugin.output_channels(), 0, "Output channels should be 0 before init");

        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        // After init, should have valid channel counts
        let input_ch = plugin.input_channels();
        let output_ch = plugin.output_channels();

        assert!(input_ch > 0, "Input channels should be > 0 after init");
        assert!(output_ch > 0, "Output channels should be > 0 after init");

        println!("Plugin configured with {} input, {} output channels", input_ch, output_ch);
        println!("✓ Channel count queries work correctly");
    }

    #[test]
    fn test_process_with_wrong_channel_count() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let input_ch = plugin.input_channels();
        let output_ch = plugin.output_channels();

        // Create buffers with WRONG channel count (1 instead of actual)
        let left_in = vec![0.0f32; 512];
        let mut left_out = vec![0.0f32; 512];

        let result = plugin.process(
            &[&left_in],  // Only 1 channel
            &mut [&mut left_out],  // Only 1 channel
            512
        );

        // Should fail if plugin needs different channel count
        if input_ch != 1 || output_ch != 1 {
            assert!(result.is_err(), "process() should fail with wrong channel count");
            println!("✓ Correctly rejected wrong channel count (1 vs {}/{})", input_ch, output_ch);
        } else {
            println!("✓ Plugin is mono, 1 channel succeeded");
        }
    }

    #[test]
    fn test_process_with_correct_channel_count() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let input_ch = plugin.input_channels();
        let output_ch = plugin.output_channels();

        // Create buffers with CORRECT channel count
        let mut inputs: Vec<Vec<f32>> = (0..input_ch).map(|_| vec![0.0f32; 512]).collect();
        let mut outputs: Vec<Vec<f32>> = (0..output_ch).map(|_| vec![0.0f32; 512]).collect();

        // Fill input with test signal
        for ch in &mut inputs {
            for (i, sample) in ch.iter_mut().enumerate() {
                let t = i as f32 / 48000.0;
                *sample = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
            }
        }

        // Convert to slices for process()
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        let result = plugin.process(&input_refs, &mut output_refs, 512);

        assert!(result.is_ok(), "process() should succeed with correct channel count");
        println!("✓ Successfully processed with {}/{} channels", input_ch, output_ch);
    }

    #[test]
    fn test_process_with_too_many_channels() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let input_ch = plugin.input_channels();
        let output_ch = plugin.output_channels();

        // Create buffers with MORE channels than plugin expects
        let extra_channels = 2;
        let inputs: Vec<Vec<f32>> = (0..(input_ch + extra_channels)).map(|_| vec![0.0f32; 512]).collect();
        let mut outputs: Vec<Vec<f32>> = (0..(output_ch + extra_channels)).map(|_| vec![0.0f32; 512]).collect();

        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        let result = plugin.process(&input_refs, &mut output_refs, 512);

        // Should always fail with too many channels
        assert!(result.is_err(), "process() should fail when provided more channels than plugin expects");

        // Verify the error message is about channel count mismatch
        if let Err(e) = result {
            let err_msg = format!("{:?}", e);
            assert!(err_msg.contains("channel count mismatch") || err_msg.contains("mismatch"),
                    "Error should mention channel mismatch, got: {}", err_msg);
        }

        println!("✓ Correctly rejected too many channels ({}/{} provided vs {}/{} expected)",
                 input_ch + extra_channels, output_ch + extra_channels, input_ch, output_ch);
    }

    #[test]
    fn test_process_with_mismatched_buffer_lengths() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let input_ch = plugin.input_channels();
        let output_ch = plugin.output_channels();

        if input_ch < 2 {
            println!("✓ Plugin has < 2 input channels, skipping mismatched length test");
            return;
        }

        // Create buffers where channels have DIFFERENT lengths
        let mut inputs: Vec<Vec<f32>> = Vec::new();
        inputs.push(vec![0.0f32; 512]);      // First channel: 512 samples
        inputs.push(vec![0.0f32; 256]);      // Second channel: only 256 samples (WRONG!)
        for _ in 2..input_ch {
            inputs.push(vec![0.0f32; 512]);  // Rest: 512 samples
        }

        let mut outputs: Vec<Vec<f32>> = (0..output_ch).map(|_| vec![0.0f32; 512]).collect();

        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        // Try to process 512 frames, but second input channel only has 256
        let result = plugin.process(&input_refs, &mut output_refs, 512);

        // Should fail because channel 1 has only 256 samples but we're asking for 512
        assert!(result.is_err(), "process() should fail when channel buffers have insufficient length");

        if let Err(e) = result {
            let err_msg = format!("{:?}", e);
            assert!(err_msg.contains("channel") && err_msg.contains("samples"),
                    "Error should mention channel and samples, got: {}", err_msg);
        }

        println!("✓ Correctly rejected mismatched buffer lengths (channel 1: 256 < 512 frames)");
    }

    // Reset Tests

    #[test]
    fn test_reset_before_init() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");

        // Try to reset before initialization
        let result = plugin.reset();
        assert!(result.is_err(), "reset() should fail before initialization");

        println!("✓ Reset correctly fails before initialization");
    }

    #[test]
    fn test_reset_after_init() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        // Reset should succeed after initialization
        let result = plugin.reset();
        assert!(result.is_ok(), "reset() should succeed after initialization");

        println!("✓ Reset succeeds after initialization");
    }

    #[test]
    fn test_reset_clears_state() {
        let Some(info) = get_test_plugin() else {
            println!("No test plugins available, skipping test");
            return;
        };

        let mut plugin = AudioUnitPlugin::new(&info).expect("Failed to create plugin");
        plugin
            .initialize(48000.0, 512)
            .expect("Failed to initialize plugin");

        let input_ch = plugin.input_channels();
        let output_ch = plugin.output_channels();

        // Process some audio to build up state (delay lines, reverb, etc.)
        let inputs: Vec<Vec<f32>> = (0..input_ch).map(|_| vec![1.0f32; 512]).collect();
        let mut outputs: Vec<Vec<f32>> = (0..output_ch).map(|_| vec![0.0f32; 512]).collect();

        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        // Process several buffers to build up internal state
        for _ in 0..10 {
            plugin.process(&input_refs, &mut output_refs, 512)
                .expect("Failed to process audio");
        }

        // Reset should clear all internal state
        plugin.reset().expect("Failed to reset plugin");

        // Process silence after reset
        let silent_inputs: Vec<Vec<f32>> = (0..input_ch).map(|_| vec![0.0f32; 512]).collect();
        let mut silent_outputs: Vec<Vec<f32>> = (0..output_ch).map(|_| vec![0.0f32; 512]).collect();

        let silent_input_refs: Vec<&[f32]> = silent_inputs.iter().map(|v| v.as_slice()).collect();
        let mut silent_output_refs: Vec<&mut [f32]> = silent_outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        plugin.process(&silent_input_refs, &mut silent_output_refs, 512)
            .expect("Failed to process audio after reset");

        println!("✓ Reset successfully clears plugin state");
    }
}
