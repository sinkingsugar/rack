//! Load and initialize an AudioUnit plugin
//!
//! This example demonstrates:
//! - Scanning for available AudioUnit plugins
//! - Loading a specific plugin instance
//! - Initializing the plugin for audio processing

use rack::prelude::*;

fn main() -> Result<()> {
    println!("AudioUnit Plugin Loading Example");
    println!("=================================\n");

    // Create scanner
    println!("Creating scanner...");
    let scanner = Scanner::new()?;

    // Scan for plugins
    println!("Scanning for AudioUnit plugins...");
    let plugins = scanner.scan()?;

    if plugins.is_empty() {
        println!("No plugins found!");
        return Ok(());
    }

    println!("Found {} plugin(s)\n", plugins.len());

    // Find first instrument plugin
    let instrument = plugins
        .iter()
        .find(|p| p.plugin_type == PluginType::Instrument);

    if let Some(info) = instrument {
        println!("Loading instrument plugin:");
        println!("  Name: {}", info.name);
        println!("  Manufacturer: {}", info.manufacturer);
        println!("  Type: {:?}", info.plugin_type);
        println!("  ID: {}", info.unique_id);
        println!();

        // Create instance
        println!("Creating plugin instance...");
        let mut plugin = scanner.load(info)?;
        println!("✓ Plugin instance created");

        // Initialize for 48kHz, 512 sample buffers
        println!("Initializing plugin (48kHz, 512 samples)...");
        plugin.initialize(48000.0, 512)?;
        println!("✓ Plugin initialized successfully!");

        println!("\nPlugin is ready for audio processing!");
        return Ok(());
    }

    // Fall back to first effect plugin
    let effect = plugins
        .iter()
        .find(|p| p.plugin_type == PluginType::Effect);

    if let Some(info) = effect {
        println!("No instrument found, loading effect plugin:");
        println!("  Name: {}", info.name);
        println!("  Manufacturer: {}", info.manufacturer);
        println!("  Type: {:?}", info.plugin_type);
        println!("  ID: {}", info.unique_id);
        println!();

        // Create instance
        println!("Creating plugin instance...");
        let mut plugin = scanner.load(info)?;
        println!("✓ Plugin instance created");

        // Initialize for 48kHz, 512 sample buffers
        println!("Initializing plugin (48kHz, 512 samples)...");
        plugin.initialize(48000.0, 512)?;
        println!("✓ Plugin initialized successfully!");

        println!("\nPlugin is ready for audio processing!");
        return Ok(());
    }

    println!("No suitable plugin found (need instrument or effect)");
    Ok(())
}
