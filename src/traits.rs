use crate::{AudioBuffer, ParameterInfo, PluginInfo, Result};

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

    /// Get plugin info
    fn info(&self) -> &PluginInfo;

    /// Check if the plugin is initialized
    fn is_initialized(&self) -> bool;
}
