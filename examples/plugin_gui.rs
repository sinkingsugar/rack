//! AudioUnit Plugin GUI Example
//!
//! This example demonstrates how to create and display a plugin's GUI.
//!
//! **IMPORTANT**: This example must be run on the main thread. On macOS, the
//! main thread is required for all GUI operations (AppKit requirement).
//!
//! # Usage
//!
//! ```bash
//! cargo run --example plugin_gui
//! ```

use rack::prelude::*;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    println!("AudioUnit Plugin GUI Example");
    println!("=============================\n");

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

    // Find first instrument or effect plugin (more likely to have interesting GUIs)
    let plugin_info = plugins
        .iter()
        .find(|p| {
            p.plugin_type == PluginType::Instrument || p.plugin_type == PluginType::Effect
        })
        .or_else(|| plugins.first());

    let Some(info) = plugin_info else {
        println!("No suitable plugin found");
        return Ok(());
    };

    println!("Selected plugin:");
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
    println!("✓ Plugin initialized successfully!\n");

    // Create GUI asynchronously
    println!("Creating plugin GUI...");
    println!("This may take a moment as we try AUv3 → AUv2 → generic UI...\n");

    // Use Arc<Mutex> to share GUI handle across threads/callbacks
    let gui_handle: Arc<Mutex<Option<AudioUnitGui>>> = Arc::new(Mutex::new(None));
    let gui_clone = gui_handle.clone();
    let plugin_name = info.name.clone();

    // Create GUI with async callback
    plugin.create_gui(move |result| {
        match result {
            Ok(gui) => {
                println!("✓ GUI created successfully!");

                // Get GUI size
                if let Ok((width, height)) = gui.get_size() {
                    println!("  GUI size: {:.0}x{:.0} points", width, height);
                }

                // Get native view pointer (for advanced embedding scenarios)
                if let Some(view_ptr) = gui.get_native_view() {
                    println!("  NSView pointer: {:?}", view_ptr);
                }

                // Show window
                println!("\nShowing plugin window...");
                if let Err(e) = gui.show_window(Some(&plugin_name)) {
                    eprintln!("Failed to show window: {}", e);
                    return Err(e);
                }

                println!("✓ Window is now visible!");
                println!("\nThe plugin GUI window should now be visible.");
                println!("Close the window or press Ctrl+C to exit.\n");

                // Store GUI handle so it stays alive
                *gui_clone.lock().unwrap() = Some(gui);
            }
            Err(e) => {
                eprintln!("✗ GUI creation failed: {}", e);
                return Err(e);
            }
        }
        Ok(())
    });

    // Keep the program running so the window stays open
    println!("GUI creation initiated...");
    println!("Waiting for window to be created and displayed...\n");

    // Simple event loop - in a real application, you'd use a proper event loop
    // or integrate with your UI framework
    let mut iterations = 0;
    loop {
        thread::sleep(Duration::from_millis(100));

        // Check if GUI was created
        {
            let gui = gui_handle.lock().unwrap();
            if gui.is_some() {
                iterations += 1;

                // After 50 iterations (5 seconds), show some info
                if iterations == 50 {
                    println!("The window should still be visible.");
                    println!("You can interact with the plugin GUI.");
                    println!("Press Ctrl+C to exit.\n");
                }

                // Optional: break after some time for automated testing
                // Uncomment to make example exit automatically:
                // if iterations > 100 {
                //     println!("Example complete. Exiting...");
                //     break;
                // }
            } else if iterations > 30 {
                // Timeout after 3 seconds if GUI wasn't created
                println!("Timeout waiting for GUI creation");
                break;
            }
        }
    }

    // GUI will be automatically destroyed when gui_handle goes out of scope
    println!("\nCleaning up...");
    println!("✓ Example complete!");

    Ok(())
}
