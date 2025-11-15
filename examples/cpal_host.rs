//! Real-time AudioUnit Host using CPAL
//!
//! This is a complete, interactive audio plugin host that demonstrates:
//! - Finding and loading an instrument plugin (synthesizer)
//! - Setting up CPAL for real-time audio output
//! - Processing audio through the plugin and playing it through speakers
//! - Sending MIDI note events with automatic note-off scheduling
//! - Opening and displaying the plugin's native GUI
//! - Loading and switching between plugin presets
//! - Thread-safe plugin access across audio and UI threads
//!
//! This example shows everything you need to build a basic DAW or
//! live performance tool with plugin hosting capabilities.
//!
//! **Note on GUI:** The GUI feature (G key) opens the plugin's native window.
//! The window appears asynchronously (it may take a moment). The example processes
//! the macOS event loop to handle GUI callbacks properly. Mouse events may not work
//! perfectly - for full GUI interactivity, consider using a GUI framework like `winit`.
//!
//! Run with: cargo run --example cpal_host --features cpal
//!
//! Controls:
//! - A S D F G H J K: Play notes (C D E F G A B C) on the musical keyboard
//! - 1-9: Load preset (1 = first preset, 2 = second, etc.)
//! - G: Open plugin GUI window (experimental - see note above)
//! - L: List available presets count
//! - Q: Quit and cleanup

use rack::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Core Foundation bindings for running the macOS event loop
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRunLoopRunInMode(
        mode: CFRunLoopMode,
        seconds: f64,
        return_after_source_handled: bool,
    ) -> i32;

    // Get the kCFRunLoopDefaultMode constant
    static kCFRunLoopDefaultMode: CFRunLoopMode;
}

// CFRunLoopMode is a CFStringRef (opaque pointer)
type CFRunLoopMode = *const std::ffi::c_void;

/// Process macOS main event loop for a short duration
/// This allows dispatch_async callbacks to execute
fn process_main_event_loop(duration_ms: u64) {
    unsafe {
        CFRunLoopRunInMode(
            kCFRunLoopDefaultMode,
            duration_ms as f64 / 1000.0,
            false,
        );
    }
}

fn main() -> Result<()> {
    println!("Rack CPAL Host Example");
    println!("======================\n");

    // Create scanner and scan for plugins
    let scanner = Scanner::new()?;
    let plugins = scanner.scan()?;

    println!("Found {} plugins total", plugins.len());

    // Find an instrument plugin (synthesizer)
    // Note: Instruments generally have more stable GUIs than some Apple effects
    let synth_info = plugins
        .iter()
        .find(|p| p.plugin_type == PluginType::Instrument)
        .ok_or_else(|| {
            Error::Other(
                "No instrument plugins found. Install a synthesizer AudioUnit to run this example.\n\
                 macOS includes DLSMusicDevice by default. You can also install free synths like:\n\
                 - Dexed (DX7 emulator)\n\
                 - Surge XT\n\
                 - Vital"
                    .to_string(),
            )
        })?;

    println!("\n🎹 Using instrument: {}", synth_info.name);
    println!("   Manufacturer: {}", synth_info.manufacturer);

    // Load the plugin
    let mut plugin = scanner.load(synth_info)?;

    // Get the default audio output device
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| Error::Other("No output device available".to_string()))?;

    println!("   Audio device: {}", device.name().unwrap_or_default());

    // Get the default output config
    let config = device
        .default_output_config()
        .map_err(|e| Error::Other(format!("Failed to get default output config: {}", e)))?;

    println!("   Sample rate: {} Hz", config.sample_rate().0);
    println!("   Channels: {}", config.channels());
    println!("   Format: {:?}", config.sample_format());

    let sample_rate = config.sample_rate().0 as f64;
    let channels = config.channels() as usize;

    // Initialize plugin with device's sample rate
    // Note: We'll use a fixed buffer size for simplicity
    let buffer_frames = 512;
    plugin.initialize(sample_rate, buffer_frames)?;

    println!("\n✓ Plugin initialized");
    println!("  Sample rate: {:.0} Hz", sample_rate);
    println!("  Buffer size: {} frames", buffer_frames);

    // Wrap plugin in Arc<Mutex<>> for sharing between threads
    let plugin = Arc::new(Mutex::new(plugin));
    let plugin_clone = plugin.clone();

    // Create input/output buffers (planar format - separate buffer per channel)
    let left_in = vec![0.0f32; buffer_frames];
    let right_in = vec![0.0f32; buffer_frames];
    let input = Arc::new((left_in, right_in));
    let input_clone = input.clone();

    // Build the audio stream
    println!("\n🔊 Starting audio stream...");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(
            &device,
            &config.into(),
            plugin_clone,
            input_clone,
            channels,
            buffer_frames,
        )?,
        cpal::SampleFormat::I16 => build_stream::<i16>(
            &device,
            &config.into(),
            plugin_clone,
            input_clone,
            channels,
            buffer_frames,
        )?,
        cpal::SampleFormat::U16 => build_stream::<u16>(
            &device,
            &config.into(),
            plugin_clone,
            input_clone,
            channels,
            buffer_frames,
        )?,
        _ => {
            return Err(Error::Other(
                "Unsupported sample format".to_string(),
            ))
        }
    };

    stream
        .play()
        .map_err(|e| Error::Other(format!("Failed to play stream: {}", e)))?;

    println!("✓ Audio stream started\n");

    // GUI handle storage
    let gui_handle: Arc<Mutex<Option<AudioUnitGui>>> = Arc::new(Mutex::new(None));

    // Print controls
    println!("Controls:");
    println!("  A S D F G H J K - Play notes (C D E F G A B C)");
    println!("  1-9 - Select preset");
    println!("  G - Open plugin GUI window (may take a moment)");
    println!("  L - List available presets");
    println!("  Q - Quit\n");

    // Simple keyboard input loop
    // Note: This is blocking I/O. For a real app, use a proper event system
    let stdin = io::stdin();
    let mut input_line = String::new();

    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        input_line.clear();

        stdin.read_line(&mut input_line).unwrap();
        let input = input_line.trim().to_lowercase();

        if input == "q" {
            break;
        } else if input == "g" {
            // Open GUI
            println!("\n🎨 Opening plugin GUI...");

            // Check if GUI is already open
            {
                let gui = gui_handle.lock().unwrap();
                if gui.is_some() {
                    println!("GUI is already open!\n");
                    continue;
                }
            }

            let mut plugin = plugin.lock().unwrap();
            let plugin_name = plugin.info().name.clone();
            let gui_clone = gui_handle.clone();

            plugin.create_gui(move |result| {
                match result {
                    Ok(gui) => {
                        println!("✓ GUI created successfully!");

                        // Get GUI size
                        if let Ok((width, height)) = gui.get_size() {
                            println!("  GUI size: {:.0}x{:.0} points", width, height);
                        }

                        // Show window
                        if let Err(e) = gui.show_window(Some(&plugin_name)) {
                            eprintln!("Failed to show window: {}", e);
                            return Err(e);
                        }

                        println!("✓ Plugin window is now visible!\n");
                        print!("> ");
                        io::stdout().flush().unwrap();

                        // Store GUI handle so it stays alive
                        *gui_clone.lock().unwrap() = Some(gui);
                    }
                    Err(e) => {
                        eprintln!("✗ GUI creation failed: {}\n", e);
                        print!("> ");
                        io::stdout().flush().unwrap();
                        return Err(e);
                    }
                }
                Ok(())
            });

            drop(plugin); // Release lock

            // Process main run loop to allow GUI creation callback to execute
            // On macOS, GUI operations are queued on the main thread's run loop
            println!("GUI creation in progress (processing events)...");

            // Process events for up to 2 seconds or until GUI is created
            for _ in 0..20 {
                // Process the main event loop to allow dispatch_async callbacks to run
                process_main_event_loop(100);

                // Check if GUI was created
                let gui = gui_handle.lock().unwrap();
                if gui.is_some() {
                    break;
                }
            }
        } else if input == "l" {
            // List presets
            let plugin = plugin.lock().unwrap();
            match plugin.preset_count() {
                Ok(preset_count) if preset_count > 0 => {
                    println!("\nPlugin has {} presets", preset_count);
                    println!("Use 1-9 to select presets\n");
                }
                _ => {
                    println!("\nNo presets available for this plugin\n");
                }
            }
        } else if let Some(ch) = input.chars().next() {
            // Handle keyboard input
            if let Some(note) = key_to_note(ch) {
                let events = vec![MidiEvent::note_on(note, 100, 0, 0)];
                {
                    let mut plugin = plugin.lock().unwrap();
                    if let Err(e) = plugin.send_midi(&events) {
                        println!("Error sending MIDI: {}", e);
                    } else {
                        println!("♪ Note {} on", note);
                    }
                }

                // Schedule note off after 500ms
                let plugin_clone = Arc::clone(&plugin);
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(500));
                    let mut plugin = plugin_clone.lock().unwrap();
                    let events = vec![MidiEvent::note_off(note, 64, 0, 0)];
                    let _ = plugin.send_midi(&events);
                });
            } else if let Some(preset_num) = ch.to_digit(10) {
                if preset_num > 0 {
                    let mut plugin = plugin.lock().unwrap();
                    match plugin.load_preset((preset_num - 1) as i32) {
                        Ok(_) => println!("✓ Loaded preset {}", preset_num),
                        Err(e) => println!("Error loading preset: {}", e),
                    }
                }
            }
        }
    }

    println!("\n👋 Shutting down...");
    drop(stream);
    println!("✓ Done!");

    Ok(())
}

/// Build an audio stream for the given sample format
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    plugin: Arc<Mutex<Plugin>>,
    input: Arc<(Vec<f32>, Vec<f32>)>,
    channels: usize,
    buffer_frames: usize,
) -> Result<cpal::Stream>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    // Output buffers (planar format)
    let mut left_out = vec![0.0f32; buffer_frames];
    let mut right_out = vec![0.0f32; buffer_frames];

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let mut plugin = plugin.lock().unwrap();

                // Process audio through the plugin (planar format)
                if let Err(e) = plugin.process(
                    &[&input.0, &input.1],
                    &mut [&mut left_out, &mut right_out],
                    buffer_frames
                ) {
                    eprintln!("Error processing audio: {}", e);
                    return;
                }

                // Copy plugin output (planar) to CPAL buffer (interleaved)
                let frames = data.len() / channels;
                for i in 0..frames.min(buffer_frames) {
                    let left = left_out[i];
                    let right = right_out[i];

                    // Write to output based on channel count
                    if channels == 2 {
                        data[i * 2] = cpal::Sample::from_sample(left);
                        data[i * 2 + 1] = cpal::Sample::from_sample(right);
                    } else if channels == 1 {
                        // Mono: average left and right
                        let mono = (left + right) * 0.5;
                        data[i] = cpal::Sample::from_sample(mono);
                    } else {
                        // Multi-channel: duplicate stereo to all channels
                        for ch in 0..channels {
                            let sample = if ch % 2 == 0 { left } else { right };
                            data[i * channels + ch] = cpal::Sample::from_sample(sample);
                        }
                    }
                }
            },
            move |err| {
                eprintln!("Stream error: {}", err);
            },
            None,
        )
        .map_err(|e| Error::Other(format!("Failed to build output stream: {}", e)))?;

    Ok(stream)
}

/// Map keyboard keys to MIDI note numbers
fn key_to_note(key: char) -> Option<u8> {
    match key {
        'a' => Some(60), // C4
        's' => Some(62), // D4
        'd' => Some(64), // E4
        'f' => Some(65), // F4
        'g' => Some(67), // G4
        'h' => Some(69), // A4
        'j' => Some(71), // B4
        'k' => Some(72), // C5
        _ => None,
    }
}
