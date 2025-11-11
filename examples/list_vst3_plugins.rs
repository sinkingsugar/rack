//! List all available VST3 plugins on the system
//!
//! This example demonstrates VST3 plugin scanning. It will find VST3 plugins
//! in standard system locations:
//! - macOS: /Library/Audio/Plug-Ins/VST3, ~/Library/Audio/Plug-Ins/VST3
//! - Windows: %CommonProgramFiles%\VST3
//! - Linux: /usr/lib/vst3, /usr/local/lib/vst3, ~/.vst3

#[cfg(all(
    not(target_os = "ios"),
    not(target_os = "tvos"),
    not(target_os = "watchos"),
    not(target_os = "visionos")
))]
use rack::vst3::Vst3Scanner;
#[cfg(all(
    not(target_os = "ios"),
    not(target_os = "tvos"),
    not(target_os = "watchos"),
    not(target_os = "visionos")
))]
use rack::{PluginScanner, Result};

#[cfg(all(
    not(target_os = "ios"),
    not(target_os = "tvos"),
    not(target_os = "watchos"),
    not(target_os = "visionos")
))]
fn main() -> Result<()> {
    println!("Scanning for VST3 plugins...\n");

    let scanner = Vst3Scanner::new()?;
    let plugins = scanner.scan()?;

    if plugins.is_empty() {
        println!("No VST3 plugins found.");
        println!("Make sure you have VST3 plugins installed in the standard locations.");
        return Ok(());
    }

    println!("Found {} VST3 plugin(s):\n", plugins.len());

    // Group by type for better organization
    let mut effects = Vec::new();
    let mut instruments = Vec::new();
    let mut analyzers = Vec::new();
    let mut spatial = Vec::new();
    let mut others = Vec::new();

    for plugin in &plugins {
        match plugin.plugin_type {
            rack::PluginType::Effect => effects.push(plugin),
            rack::PluginType::Instrument => instruments.push(plugin),
            rack::PluginType::Analyzer => analyzers.push(plugin),
            rack::PluginType::Spatial => spatial.push(plugin),
            _ => others.push(plugin),
        }
    }

    // Print by category
    if !instruments.is_empty() {
        println!("=== Instruments ({}) ===\n", instruments.len());
        for plugin in instruments {
            print_plugin_info(plugin);
        }
    }

    if !effects.is_empty() {
        println!("=== Effects ({}) ===\n", effects.len());
        for plugin in effects {
            print_plugin_info(plugin);
        }
    }

    if !analyzers.is_empty() {
        println!("=== Analyzers ({}) ===\n", analyzers.len());
        for plugin in analyzers {
            print_plugin_info(plugin);
        }
    }

    if !spatial.is_empty() {
        println!("=== Spatial ({}) ===\n", spatial.len());
        for plugin in spatial {
            print_plugin_info(plugin);
        }
    }

    if !others.is_empty() {
        println!("=== Other ({}) ===\n", others.len());
        for plugin in others {
            print_plugin_info(plugin);
        }
    }

    Ok(())
}

#[cfg(all(
    not(target_os = "ios"),
    not(target_os = "tvos"),
    not(target_os = "watchos"),
    not(target_os = "visionos")
))]
fn print_plugin_info(plugin: &rack::PluginInfo) {
    println!("{} by {}", plugin.name, plugin.manufacturer);
    println!("  Version: v{}", plugin.version);
    println!("  Path: {}", plugin.path.display());
    println!("  UID: {}", plugin.unique_id);
    println!();
}

#[cfg(any(
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
fn main() {
    println!("VST3 is not available on mobile platforms.");
    println!("This example only works on desktop platforms (macOS, Windows, Linux).");
}
