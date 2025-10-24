//! # Rack - Audio Plugin Hosting for Rust
//!
//! Rack is a modern Rust library for hosting audio plugins in your applications.
//! It provides a clean, safe API for discovering, loading, and processing audio
//! through VST3, AudioUnit, CLAP, and other plugin formats.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use rack::prelude::*;
//!
//! # fn main() -> rack::Result<()> {
//! // Scan for plugins
//! let scanner = Scanner::new()?;
//! let plugins = scanner.scan()?;
//!
//! println!("Found {} plugins", plugins.len());
//!
//! // Load a plugin
//! let mut plugin = scanner.load(&plugins[0])?;
//! plugin.initialize(48000.0, 512)?;
//!
//! // Process audio (planar format - one buffer per channel)
//! let left_in = vec![0.0f32; 512];
//! let right_in = vec![0.0f32; 512];
//! let mut left_out = vec![0.0f32; 512];
//! let mut right_out = vec![0.0f32; 512];
//!
//! plugin.process(
//!     &[&left_in, &right_in],
//!     &mut [&mut left_out, &mut right_out],
//!     512
//! )?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Features
//!
//! - **AudioUnit support** (macOS) - built-in
//! - **VST3 support** - coming soon
//! - **CLAP support** - coming soon
//! - **cpal integration** - optional, enable with `cpal` feature
//!
//! ## Platform Support
//!
//! Currently, AudioUnit hosting is only available on macOS.
//! VST3 and CLAP support will be cross-platform.

pub mod error;
pub mod midi;
pub mod plugin_info;
pub mod traits;

pub use error::{Error, Result};
pub use midi::{MidiEvent, MidiEventKind};
pub use plugin_info::{ParameterInfo, PluginInfo, PluginType, PresetInfo};
pub use traits::{PluginInstance, PluginScanner};

// Platform-specific implementations
#[cfg(target_os = "macos")]
pub mod au;

// Re-export the default scanner and plugin types for the platform
#[cfg(target_os = "macos")]
pub use au::{AudioUnitGui, AudioUnitPlugin as Plugin, AudioUnitScanner as Scanner};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{
        Error, MidiEvent, MidiEventKind, ParameterInfo, PluginInfo, PluginInstance,
        PluginScanner, PluginType, PresetInfo, Result,
    };

    #[cfg(target_os = "macos")]
    pub use crate::{AudioUnitGui, Plugin, Scanner};
}
