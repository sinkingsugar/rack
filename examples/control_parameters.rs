//! Control AudioUnit plugin parameters
//!
//! This example demonstrates:
//! - Enumerating plugin parameters
//! - Getting parameter information (name, range, default)
//! - Reading parameter values
//! - Setting parameter values
//! - Parameter normalization (0.0-1.0)

use rack::prelude::*;

fn main() -> Result<()> {
    println!("AudioUnit Parameter Control Example");
    println!("====================================\n");

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

    // Find first effect plugin (effects typically have parameters)
    let effect = plugins
        .iter()
        .find(|p| p.plugin_type == PluginType::Effect);

    if let Some(info) = effect {
        println!("Loading plugin:");
        println!("  Name: {}", info.name);
        println!("  Manufacturer: {}", info.manufacturer);
        println!("  Type: {:?}", info.plugin_type);
        println!();

        // Create and initialize plugin
        println!("Creating plugin instance...");
        let mut plugin = scanner.load(info)?;
        println!("✓ Plugin instance created");

        println!("Initializing plugin (48kHz, 512 samples)...");
        plugin.initialize(48000.0, 512)?;
        println!("✓ Plugin initialized successfully!");
        println!();

        // Get parameter count
        let param_count = plugin.parameter_count();
        println!("Parameter Count: {}", param_count);
        println!();

        if param_count == 0 {
            println!("This plugin has no parameters.");
            return Ok(());
        }

        // List all parameters with their info
        println!("Parameter List:");
        println!("{:-<80}", "");
        for i in 0..param_count {
            let param = plugin.parameter_info(i)?;
            let value = plugin.get_parameter(i)?;

            print!("[{}] {}", i, param.name);
            if !param.unit.is_empty() {
                print!(" ({})", param.unit);
            }
            println!();
            println!("    Range: {:.2} - {:.2}", param.min, param.max);
            println!("    Default: {:.2}", param.default);
            println!("    Current: {:.2} (normalized: {:.4})",
                     denormalize(value, param.min, param.max),
                     value);
            println!();
        }
        println!("{:-<80}", "");
        println!();

        // Demonstrate parameter control on first parameter
        if param_count > 0 {
            println!("Demonstrating parameter control on parameter 0:");
            let param = plugin.parameter_info(0)?;
            println!("  Parameter: {}", param.name);
            println!();

            // Get original value
            let original_value = plugin.get_parameter(0)?;
            println!("  Original value: {:.4} (normalized)", original_value);
            println!("  Actual value: {:.2}", denormalize(original_value, param.min, param.max));
            println!();

            // Set to different values
            let test_values = vec![0.0, 0.25, 0.5, 0.75, 1.0];
            println!("  Testing different values:");
            for &normalized_value in &test_values {
                plugin.set_parameter(0, normalized_value)?;
                let actual = plugin.get_parameter(0)?;
                let denorm = denormalize(actual, param.min, param.max);

                println!("    Set to {:.2} → Read back: {:.4} (actual: {:.2})",
                         normalized_value, actual, denorm);
            }
            println!();

            // Restore original value
            plugin.set_parameter(0, original_value)?;
            println!("  ✓ Restored original value: {:.4}", original_value);
            println!();
        }

        println!("✓ Parameter control demonstration complete!");

        return Ok(());
    }

    println!("No effect plugin found to demonstrate parameters");
    Ok(())
}

/// Helper function to denormalize a 0.0-1.0 value to actual parameter range
fn denormalize(normalized: f32, min: f32, max: f32) -> f32 {
    min + (normalized * (max - min))
}
