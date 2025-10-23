//! Browse and load AudioUnit plugin factory presets
//!
//! This example demonstrates:
//! - Enumerating plugin factory presets
//! - Loading presets
//! - Saving and restoring plugin state
//! - Verifying state changes with parameters

use rack::prelude::*;

fn main() -> Result<()> {
    println!("AudioUnit Preset Browser");
    println!("========================\n");

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

    // Find first plugin with presets
    // NOTE: This is a demonstration example that loads plugins to check for presets.
    // In production code, consider caching preset availability during scanning or
    // allowing the user to select a plugin manually to avoid loading many plugins.
    let plugin_with_presets = plugins.iter().find(|info| {
        // Try to check if plugin has presets by creating a temporary instance
        if let Ok(mut plugin) = scanner.load(info) {
            if plugin.initialize(48000.0, 512).is_ok() {
                return plugin.preset_count() > 0;
            }
        }
        false
    });

    let info = if let Some(info) = plugin_with_presets {
        info
    } else {
        println!("No plugins with factory presets found!");
        println!("Demonstrating state save/restore with first plugin instead...\n");
        &plugins[0]
    };

    println!("Plugin: {}", info.name);
    println!("Manufacturer: {}", info.manufacturer);
    println!("Type: {:?}", info.plugin_type);
    println!();

    // Create and initialize plugin
    println!("Creating plugin instance...");
    let mut plugin = scanner.load(info)?;
    println!("✓ Plugin instance created");

    println!("Initializing plugin (48kHz, 512 samples)...");
    plugin.initialize(48000.0, 512)?;
    println!("✓ Plugin initialized successfully!\n");

    // Get preset count
    let preset_count = plugin.preset_count();
    println!("Factory Presets: {}", preset_count);
    println!();

    if preset_count > 0 {
        // List all factory presets
        println!("Available Presets:");
        println!("{:-<60}", "");
        for i in 0..preset_count {
            let preset = plugin.preset_info(i)?;
            println!("[{}] {} (preset #{:3})", i, preset.name, preset.preset_number);
        }
        println!("{:-<60}", "");
        println!();

        // Demonstrate loading presets and showing parameter changes
        let param_count = plugin.parameter_count();
        if param_count > 0 {
            println!("Demonstrating preset loading with parameter 0:");
            println!();

            let param_info = plugin.parameter_info(0)?;
            println!("Parameter: {}", param_info.name);
            if !param_info.unit.is_empty() {
                println!("Unit: {}", param_info.unit);
            }
            println!("Range: {} - {}", param_info.min, param_info.max);
            println!();

            // Show parameter value for first few presets
            let presets_to_demo = preset_count.min(5);
            for i in 0..presets_to_demo {
                let preset = plugin.preset_info(i)?;
                plugin.load_preset(preset.preset_number)?;

                let value = plugin.get_parameter(0)?;
                let actual = param_info.min + (value * (param_info.max - param_info.min));

                println!("Preset '{}': parameter = {:.4} (actual: {:.2})",
                         preset.name, value, actual);
            }
            println!();
        }
    } else {
        println!("This plugin has no factory presets.\n");
    }

    // Demonstrate state save/restore
    println!("State Serialization Demo:");
    println!("{:-<60}", "");

    // Save current state
    let saved_state = plugin.get_state()?;
    println!("✓ Saved plugin state ({} bytes)", saved_state.len());

    // Make changes (if plugin has parameters)
    let param_count = plugin.parameter_count();
    if param_count > 0 {
        println!();
        println!("Making changes to plugin parameters...");

        let original_value = plugin.get_parameter(0)?;
        println!("  Parameter 0 original: {:.4}", original_value);

        // Change parameter
        plugin.set_parameter(0, 0.75)?;
        let changed_value = plugin.get_parameter(0)?;
        println!("  Parameter 0 changed:  {:.4}", changed_value);

        // Restore state
        println!();
        println!("Restoring saved state...");
        plugin.set_state(&saved_state)?;
        println!("✓ State restored");

        let restored_value = plugin.get_parameter(0)?;
        println!("  Parameter 0 restored: {:.4}", restored_value);

        // Verify restoration
        if (restored_value - original_value).abs() < 0.01 {
            println!();
            println!("✓ Parameter successfully restored to original value!");
        } else {
            println!();
            println!("⚠ Parameter value differs slightly (may be rounding)");
        }
    } else {
        println!("Plugin has no parameters to demonstrate state changes.");
    }

    println!();
    println!("{:-<60}", "");
    println!();
    println!("✓ Preset browser demonstration complete!");

    Ok(())
}
