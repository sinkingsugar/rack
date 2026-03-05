//! Simple VST3 audio processing example
//!
//! This example demonstrates:
//! - Loading a VST3 plugin
//! - Initializing it for audio processing
//! - Processing audio through the plugin
//! - Accessing parameters

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
use rack::{PluginInstance, PluginScanner, Result};

#[cfg(all(
    not(target_os = "ios"),
    not(target_os = "tvos"),
    not(target_os = "watchos"),
    not(target_os = "visionos")
))]
fn main() -> Result<()> {
    println!("VST3 Audio Processing Example\n");

    // Scan for VST3 plugins
    let scanner = Vst3Scanner::new()?;
    let plugins = scanner.scan()?;

    if plugins.is_empty() {
        println!("No VST3 plugins found.");
        return Ok(());
    }

    // Find an effect plugin (not an instrument)
    let plugin_info = plugins
        .iter()
        .find(|p| p.plugin_type == rack::PluginType::Effect)
        .or_else(|| plugins.first())
        .unwrap();

    println!("Loading plugin: {}", plugin_info.name);
    println!("Manufacturer: {}", plugin_info.manufacturer);
    println!();

    // Load the plugin
    let mut plugin = scanner.load(plugin_info)?;

    // Initialize for 48kHz, 512 samples per buffer
    let sample_rate = 48000.0;
    let buffer_size = 512;

    println!("Initializing plugin...");
    plugin.initialize(sample_rate, buffer_size)?;

    println!("Plugin initialized successfully!");
    println!();

    // Show parameter information
    let param_count = plugin.parameter_count();
    println!("Plugin has {} parameters", param_count);

    if param_count > 0 {
        println!("\nFirst 5 parameters:");
        for i in 0..param_count.min(5) {
            if let Ok(info) = plugin.parameter_info(i) {
                let value = plugin.get_parameter(i).unwrap_or(0.0);
                println!(
                    "  [{}] {} = {:.2} (range: {:.2} - {:.2}) {}",
                    i, info.name, value, info.min, info.max, info.unit
                );
            }
        }
    }

    println!("\nProcessing audio...");

    // Create test audio buffers (stereo, silence)
    let mut left_in = vec![0.0f32; buffer_size];
    let mut right_in = vec![0.0f32; buffer_size];
    let mut left_out = vec![0.0f32; buffer_size];
    let mut right_out = vec![0.0f32; buffer_size];

    // Generate a simple test signal (sine wave at 440 Hz)
    for i in 0..buffer_size {
        let t = i as f32 / sample_rate as f32;
        let sample = (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5;
        left_in[i] = sample;
        right_in[i] = sample;
    }

    // Process audio through the plugin
    plugin.process(
        &[&left_in, &right_in],
        &mut [&mut left_out, &mut right_out],
        buffer_size,
    )?;

    println!("Audio processed successfully!");

    // Check if the output is different from input (plugin is working)
    let input_rms: f32 = left_in.iter().map(|x| x * x).sum::<f32>() / buffer_size as f32;
    let output_rms: f32 = left_out.iter().map(|x| x * x).sum::<f32>() / buffer_size as f32;

    println!(
        "Input RMS: {:.6}, Output RMS: {:.6}",
        input_rms.sqrt(),
        output_rms.sqrt()
    );

    // Try changing a parameter if available
    if param_count > 0 {
        println!("\nTesting parameter control...");
        if let Ok(info) = plugin.parameter_info(0) {
            let original_value = plugin.get_parameter(0)?;
            let new_value = (info.min + info.max) / 2.0; // Set to middle

            println!(
                "Parameter '{}': {:.2} -> {:.2}",
                info.name, original_value, new_value
            );
            plugin.set_parameter(0, new_value)?;

            let read_back = plugin.get_parameter(0)?;
            println!("Read back: {:.2}", read_back);
        }
    }

    println!("\nExample completed successfully!");

    Ok(())
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
