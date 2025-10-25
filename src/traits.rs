use crate::{MidiEvent, ParameterInfo, PluginInfo, Result};

/// Trait for scanning and discovering audio plugins
pub trait PluginScanner {
    /// The type of plugin instance this scanner produces
    type Plugin: PluginInstance;

    /// Scan for plugins in default system locations
    fn scan(&self) -> Result<Vec<PluginInfo>>;

    /// Scan for plugins in a specific directory
    fn scan_path(&self, path: &std::path::Path) -> Result<Vec<PluginInfo>>;

    /// Load a plugin from PluginInfo
    fn load(&self, info: &PluginInfo) -> Result<Self::Plugin>;
}

/// Trait for an instantiated audio plugin
///
/// # Thread Safety
///
/// - All methods except `process()` should be called from **non-realtime threads**
/// - `initialize()` and `Drop` are **globally serialized** (mutex protected) for AudioUnit safety
/// - Other methods (reset, parameters, etc.) are safe but should not be called from audio thread
/// - Only `process()` is designed for realtime/audio thread usage
pub trait PluginInstance: Send {
    /// Initialize the plugin with the given sample rate and maximum block size
    ///
    /// # Thread Safety
    ///
    /// This method is **globally serialized** across all plugin instances to work around
    /// thread-safety issues in Apple's AudioUnit framework. Call from a non-realtime thread.
    fn initialize(&mut self, sample_rate: f64, max_block_size: usize) -> Result<()>;

    /// Reset the plugin's internal state
    ///
    /// Clears all internal buffers, delay lines, and state without changing parameters.
    /// Useful for:
    /// - Clearing reverb/delay tails when stopping playback
    /// - Resetting plugin state between songs
    /// - Clearing artifacts after preset changes
    ///
    /// # Notes
    ///
    /// - Plugin must be initialized before calling reset
    /// - Parameters are NOT reset (use set_parameter or load_preset for that)
    /// - Sample rate and buffer size remain unchanged
    ///
    /// # Thread Safety
    ///
    /// Call from a **non-realtime thread**. This is a runtime state operation (not globally serialized).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example(mut plugin: impl PluginInstance) -> Result<()> {
    /// plugin.initialize(48000.0, 512)?;
    /// // ... process audio ...
    /// plugin.reset()?; // Clear reverb tail, delay lines, etc.
    /// # Ok(())
    /// # }
    /// ```
    fn reset(&mut self) -> Result<()>;

    /// Process audio through the plugin
    ///
    /// Uses planar (non-interleaved) audio format - each channel is a separate buffer.
    /// This matches the internal format of VST3, AudioUnit, CLAP, and AAX plugins,
    /// enabling zero-copy processing in effect chains.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Array of input channel buffers (e.g., `&[left, right]` for stereo)
    /// * `outputs` - Array of output channel buffers (e.g., `&mut [left, right]` for stereo)
    /// * `num_frames` - Number of audio frames to process (must be ≤ max_block_size)
    ///
    /// # Channel Formats
    ///
    /// * Mono: `inputs = &[&mono]`, `outputs = &mut [&mut mono]`
    /// * Stereo: `inputs = &[&left, &right]`, `outputs = &mut [&mut left, &mut right]`
    /// * 5.1 Surround: 6 channels (L, R, C, LFE, SL, SR)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example(mut plugin: impl PluginInstance) -> Result<()> {
    /// // Stereo processing
    /// let left_in = vec![0.0f32; 512];
    /// let right_in = vec![0.0f32; 512];
    /// let mut left_out = vec![0.0f32; 512];
    /// let mut right_out = vec![0.0f32; 512];
    ///
    /// plugin.process(
    ///     &[&left_in, &right_in],
    ///     &mut [&mut left_out, &mut right_out],
    ///     512
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    fn process(
        &mut self,
        inputs: &[&[f32]],
        outputs: &mut [&mut [f32]],
        num_frames: usize,
    ) -> Result<()>;

    /// Get the number of parameters
    fn parameter_count(&self) -> usize;

    /// Get information about a parameter
    fn parameter_info(&self, index: usize) -> Result<ParameterInfo>;

    /// Get the current value of a parameter (normalized 0.0 to 1.0)
    fn get_parameter(&self, index: usize) -> Result<f32>;

    /// Set the value of a parameter (normalized 0.0 to 1.0)
    fn set_parameter(&mut self, index: usize, value: f32) -> Result<()>;

    /// Send MIDI events to the plugin
    ///
    /// This is primarily useful for instrument plugins (synthesizers, samplers).
    /// Effect plugins typically don't respond to MIDI events.
    ///
    /// Events are processed immediately. For sample-accurate timing,
    /// set the `sample_offset` field in each event.
    ///
    /// # Performance Note
    ///
    /// This method is **zero-allocation** for typical use cases (≤16 events).
    /// Events are stored on the stack using `SmallVec`, avoiding heap allocation
    /// for chords, sequences, and most real-time MIDI scenarios. Only batches
    /// exceeding 16 events will allocate from the heap.
    ///
    /// # MIDI Message Support
    ///
    /// Comprehensive MIDI 1.0 support:
    ///
    /// **Channel Messages:**
    /// - Note On/Off
    /// - Polyphonic Aftertouch (per-key pressure)
    /// - Control Change (CC)
    /// - Program Change
    /// - Channel Aftertouch (channel pressure)
    /// - Pitch Bend (14-bit resolution)
    ///
    /// **System Real-Time:**
    /// - Timing Clock, Start, Continue, Stop
    /// - Active Sensing, System Reset
    ///
    /// # Arguments
    ///
    /// * `events` - Slice of MIDI events to send
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The plugin doesn't support MIDI (e.g., most effect plugins)
    /// - Any event has invalid data (channel > 15, etc.)
    /// - The plugin is not initialized
    fn send_midi(&mut self, events: &[MidiEvent]) -> Result<()>;

    /// Get plugin info
    fn info(&self) -> &PluginInfo;

    /// Check if the plugin is initialized
    fn is_initialized(&self) -> bool;
}
