//! Process audio through an AudioUnit effect plugin
//!
//! This example demonstrates:
//! - Loading an effect plugin
//! - Initializing it for audio processing
//! - Processing audio buffers through the plugin
//! - Analyzing the output

use rack::prelude::*;

fn main() -> Result<()> {
    println!("AudioUnit Audio Processing Example");
    println!("===================================\n");

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

    // Find first effect plugin
    let effect = plugins
        .iter()
        .find(|p| p.plugin_type == PluginType::Effect);

    if let Some(info) = effect {
        println!("Loading effect plugin:");
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
        println!();

        // Create aligned test buffers (512 frames of stereo audio = 1024 samples)
        let frames = 512;
        let mut input = AudioBuffer::new(frames * 2);
        let mut output = AudioBuffer::new(frames * 2);

        println!("Generating test signal (440 Hz sine wave)...");
        let frequency = 440.0f32; // A4
        let sample_rate = 48000.0f32;

        for i in 0..frames {
            let t = i as f32 / sample_rate;
            let sample = (2.0 * std::f32::consts::PI * frequency * t).sin() * 0.5;
            input[i * 2] = sample; // Left channel
            input[i * 2 + 1] = sample; // Right channel
        }

        println!("✓ Test signal generated ({} frames, stereo)", frames);
        println!();

        // Process audio
        println!("Processing audio through plugin...");
        plugin.process(&input, &mut output)?;
        println!("✓ Audio processing complete!");
        println!();

        // Analyze output
        println!("Analyzing output:");

        // Calculate RMS (Root Mean Square) level
        let rms: f32 = output
            .iter()
            .map(|&sample| sample * sample)
            .sum::<f32>()
            / output.len() as f32;
        let rms = rms.sqrt();

        println!("  RMS level: {:.6}", rms);

        // Find peak level
        let peak = output.iter().fold(0.0f32, |max, &sample| max.max(sample.abs()));
        println!("  Peak level: {:.6}", peak);

        // Check if output has signal
        let has_signal = output.iter().any(|&sample| sample != 0.0);
        if has_signal {
            println!("  Signal: ✓ Output contains audio");
        } else {
            println!("  Signal: ✗ Output is silent");
        }

        // Compare input and output
        let input_rms: f32 = input
            .iter()
            .map(|&sample| sample * sample)
            .sum::<f32>()
            / input.len() as f32;
        let input_rms = input_rms.sqrt();

        println!();
        println!("Comparison:");
        println!("  Input RMS:  {:.6}", input_rms);
        println!("  Output RMS: {:.6}", rms);
        println!("  Gain change: {:.2} dB", 20.0 * (rms / input_rms).log10());

        println!();
        println!("✓ Audio processing demonstration complete!");

        return Ok(());
    }

    println!("No effect plugin found to demonstrate processing");
    Ok(())
}
