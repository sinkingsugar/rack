//! AudioUnit Plugin GUI Example
//!
//! This example demonstrates how to create and display a plugin's GUI.
//!
//! **IMPORTANT**: This example must be run on the main thread. On macOS, the
//! main thread is required for all GUI operations (AppKit requirement).
//!
//! **Note**: This example uses instrument plugins (synths) which generally have
//! more stable GUIs. Some Apple effect plugins (like AUBandpass) have buggy
//! generic UIs that crash in Apple's CoreAudioKit framework.
//!
//! **Limitation**: Mouse events may not work perfectly in this terminal-based example
//! because we're using a minimal event loop. For full interactivity, integrate with
//! a proper GUI framework like `winit` or embed the view in a native macOS app.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example plugin_gui
//! ```

use rack::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Core Foundation and AppKit bindings for running the macOS event loop
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRunLoopRunInMode(
        mode: CFRunLoopMode,
        seconds: f64,
        return_after_source_handled: bool,
    ) -> i32;

    static kCFRunLoopDefaultMode: CFRunLoopMode;
}

type CFRunLoopMode = *const std::ffi::c_void;

/// Process macOS main event loop for a short duration
fn process_main_event_loop(duration_ms: u64) {
    unsafe {
        CFRunLoopRunInMode(
            kCFRunLoopDefaultMode,
            duration_ms as f64 / 1000.0,
            false,
        );
    }
}

/// Run the main event loop indefinitely
/// This is required for GUI windows to actually appear and be interactive
fn run_event_loop() {
    println!("Starting event loop...");

    // Just keep processing events in a loop
    // The window will stay alive as long as we keep processing
    loop {
        unsafe {
            // Process events for 1 second at a time
            // This allows the window to be interactive
            CFRunLoopRunInMode(
                kCFRunLoopDefaultMode,
                1.0,
                true,  // Return after source handled
            );
        }

        // Small sleep to avoid busy-waiting
        std::thread::sleep(Duration::from_millis(10));
    }
}

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

    // Find first instrument plugin (instruments usually have better/more stable GUIs)
    // Avoid AUBandpass and other Apple effects that have buggy generic UIs
    let plugin_info = plugins
        .iter()
        .find(|p| {
            p.plugin_type == PluginType::Instrument
        })
        .or_else(|| {
            // If no instrument, try to find an effect that's NOT AUBandpass
            plugins.iter().find(|p| {
                p.plugin_type == PluginType::Effect && !p.name.contains("Bandpass")
            })
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

    // Wait a moment for the GUI to be created
    println!("Processing events...");
    for _ in 0..30 {
        process_main_event_loop(100);

        let gui = gui_handle.lock().unwrap();
        if gui.is_some() {
            println!("\n✓ GUI window should now be visible!");
            println!("The window will stay open. Press Ctrl+C to exit.\n");
            drop(gui);
            break;
        }
    }

    // Check if GUI was created
    {
        let gui = gui_handle.lock().unwrap();
        if gui.is_none() {
            println!("Timeout waiting for GUI creation");
            return Ok(());
        }
    }

    // Now run the main event loop to keep the window alive and interactive
    // This will block until the user closes the window or presses Ctrl+C
    println!("Running event loop (window is now interactive)...");
    run_event_loop();

    // Cleanup when event loop exits
    println!("\nCleaning up...");
    {
        let mut gui = gui_handle.lock().unwrap();
        *gui = None;
    }

    println!("✓ Example complete!");

    Ok(())
}
