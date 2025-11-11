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
//! - **AudioUnit support** (macOS, iOS) - built-in
//! - **VST3 support** (Windows, macOS, Linux) - built-in
//! - **CLAP support** - coming soon
//! - **cpal integration** - optional, enable with `cpal` feature
//!
//! ## Platform Support
//!
//! - **macOS**: AudioUnit (default) and VST3
//! - **iOS**: AudioUnit only (VST3 not available on mobile)
//! - **Windows**: VST3 (default)
//! - **Linux**: VST3 (default)
//!
//! AudioUnit provides the best integration on Apple platforms (native GUI support).
//! VST3 is the default on Windows and Linux, and also available on macOS.

pub mod error;
pub mod midi;
pub mod plugin_info;
pub mod traits;

pub use error::{Error, Result};
pub use midi::{MidiEvent, MidiEventKind};
pub use plugin_info::{ParameterInfo, PluginInfo, PluginType, PresetInfo};
pub use traits::{PluginInstance, PluginScanner};

// Platform-specific implementations
// AudioUnit is available on both macOS and iOS
#[cfg(target_vendor = "apple")]
pub mod au;

// VST3 is available on desktop platforms (Windows, macOS, Linux)
// Explicitly disabled on mobile platforms (iOS, tvOS, watchOS, visionOS)
#[cfg(all(
    not(target_os = "ios"),
    not(target_os = "tvos"),
    not(target_os = "watchos"),
    not(target_os = "visionos")
))]
pub mod vst3;

// Re-export the default scanner and plugin types for the platform
// On Apple platforms, default to AudioUnit (better integration, GUI support)
#[cfg(target_vendor = "apple")]
pub use au::{AudioUnitGui, AudioUnitPlugin as Plugin, AudioUnitScanner as Scanner};

// On non-Apple desktop platforms, default to VST3
#[cfg(all(
    not(target_vendor = "apple"),
    not(target_os = "ios"),
    not(target_os = "tvos"),
    not(target_os = "watchos"),
    not(target_os = "visionos")
))]
pub use vst3::{Vst3Plugin as Plugin, Vst3Scanner as Scanner};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{
        Error, MidiEvent, MidiEventKind, ParameterInfo, PluginInfo, PluginInstance,
        PluginScanner, PluginType, PresetInfo, Result,
    };

    // Platform-specific exports
    #[cfg(target_vendor = "apple")]
    pub use crate::{AudioUnitGui, Plugin, Scanner};

    #[cfg(all(
        not(target_vendor = "apple"),
        not(target_os = "ios"),
        not(target_os = "tvos"),
        not(target_os = "watchos"),
        not(target_os = "visionos")
    ))]
    pub use crate::{Plugin, Scanner};
}
