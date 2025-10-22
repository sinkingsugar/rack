use crate::{PluginInfo, PluginScanner, Result};

use super::instance::AudioUnitPlugin;

/// Scanner for AudioUnit plugins on macOS
pub struct AudioUnitScanner;

impl AudioUnitScanner {
    /// Create a new AudioUnit scanner
    pub fn new() -> Self {
        Self
    }

    /// Scan for AudioUnit components
    fn scan_components(&self) -> Result<Vec<PluginInfo>> {
        let plugins = Vec::new();

        // TODO: Implement actual AudioComponent enumeration
        // For now, this is a placeholder that will be implemented using:
        // - AudioComponentFindNext to iterate through components
        // - AudioComponentGetDescription to get component info
        // - AudioComponentCopyName to get the name

        Ok(plugins)
    }
}

impl Default for AudioUnitScanner {
    fn default() -> Self {
        Self::new()
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
