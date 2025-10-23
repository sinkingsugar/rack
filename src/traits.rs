use crate::{AudioBuffer, MidiEvent, ParameterInfo, PluginInfo, Result};

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
pub trait PluginInstance: Send {
    /// Initialize the plugin with the given sample rate and maximum block size
    fn initialize(&mut self, sample_rate: f64, max_block_size: usize) -> Result<()>;

    /// Process audio through the plugin
    ///
    /// Buffers must be 16-byte aligned for optimal SIMD performance.
    /// Use `AudioBuffer` to ensure alignment.
    ///
    /// For effects: input contains the audio to process, output receives the processed audio
    /// For instruments: input may be empty, output receives the generated audio
    fn process(&mut self, input: &AudioBuffer, output: &mut AudioBuffer) -> Result<()>;

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
    /// Currently supports:
    /// - Note On/Off
    /// - Control Change (CC)
    /// - Program Change
    ///
    /// Not yet supported (planned for future phases):
    /// - Pitch Bend
    /// - Aftertouch (polyphonic and channel)
    /// - System messages (clock, start/stop)
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
