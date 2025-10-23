//! Simple synthesizer example
//!
//! This example demonstrates:
//! - Finding an instrument plugin
//! - Sending MIDI note events
//! - Processing audio to render the notes
//! - Analyzing the output with RMS and peak levels
//!
//! Run with: cargo run --example simple_synth

use rack::prelude::*;

fn main() -> Result<()> {
    println!("Rack Simple Synthesizer Example");
    println!("================================\n");

    // Create scanner and scan for plugins
    let scanner = Scanner::new()?;
    let plugins = scanner.scan()?;

    println!("Found {} plugins total", plugins.len());

    // Find an instrument plugin (synthesizer)
    let synth_info = plugins
        .iter()
        .find(|p| p.plugin_type == PluginType::Instrument)
        .ok_or_else(|| Error::Other(
            "No instrument plugins found. Install a synthesizer AudioUnit to run this example.\n\
             macOS includes DLSMusicDevice by default. You can also install free synths like:\n\
             - Dexed (DX7 emulator)\n\
             - Surge XT\n\
             - Vital".to_string()
        ))?;

    println!("Using instrument: {}", synth_info.name);
    println!("Manufacturer: {}", synth_info.manufacturer);
    println!("Type: {:?}\n", synth_info.plugin_type);

    // Load the plugin
    let mut plugin = scanner.load(synth_info)?;

    // Initialize with 48kHz sample rate and 512 frame buffer
    let sample_rate = 48000.0;
    let buffer_frames = 512;
    plugin.initialize(sample_rate, buffer_frames)?;

    println!("Plugin initialized:");
    println!("  Sample rate: {:.1} Hz", sample_rate);
    println!("  Buffer size: {} frames\n", buffer_frames);

    // Create audio buffers (stereo interleaved = frames * 2)
    let buffer_size = buffer_frames * 2;
    let input = AudioBuffer::new(buffer_size);
    let mut output = AudioBuffer::new(buffer_size);

    // Play a C major chord (C-E-G)
    println!("Playing C major chord (notes 60, 64, 67)...");

    let events = vec![
        MidiEvent::note_on(60, 100, 0, 0), // Middle C (C4), velocity 100, channel 0
        MidiEvent::note_on(64, 100, 0, 0), // E4
        MidiEvent::note_on(67, 100, 0, 0), // G4
    ];

    plugin.send_midi(&events)?;
    println!("✓ MIDI note on events sent");

    // Process multiple buffers to let the synth generate audio
    let num_buffers = 10;
    println!("\nProcessing {} buffers...", num_buffers);

    for i in 0..num_buffers {
        // Process audio (synth renders MIDI notes to output)
        plugin.process(&input, &mut output)?;

        // Calculate RMS and peak levels for this buffer
        let (rms_left, rms_right, peak_left, peak_right) = analyze_buffer(&output, buffer_frames);

        println!(
            "  Buffer {:2}: L RMS={:6.4} Peak={:6.4} | R RMS={:6.4} Peak={:6.4}",
            i + 1,
            rms_left,
            peak_left,
            rms_right,
            peak_right
        );
    }

    // Send note off events to release the notes
    println!("\nReleasing notes...");

    let events = vec![
        MidiEvent::note_off(60, 64, 0, 0),
        MidiEvent::note_off(64, 64, 0, 0),
        MidiEvent::note_off(67, 64, 0, 0),
    ];

    plugin.send_midi(&events)?;
    println!("✓ MIDI note off events sent");

    // Process a few more buffers during release phase
    println!("\nProcessing {} buffers during release...", 5);

    for i in 0..5 {
        plugin.process(&input, &mut output)?;

        let (rms_left, rms_right, peak_left, peak_right) = analyze_buffer(&output, buffer_frames);

        println!(
            "  Buffer {:2}: L RMS={:6.4} Peak={:6.4} | R RMS={:6.4} Peak={:6.4}",
            i + 1,
            rms_left,
            peak_left,
            rms_right,
            peak_right
        );
    }

    println!("\n✓ Synthesis complete!");
    println!("\nNote: If all RMS/Peak values are 0.0000, the synth may need");
    println!("      different initialization or parameter setup.");

    Ok(())
}

/// Calculate RMS and peak levels for left and right channels
fn analyze_buffer(
    buffer: &AudioBuffer,
    frames: usize,
) -> (f32, f32, f32, f32) {
    let mut sum_left = 0.0f32;
    let mut sum_right = 0.0f32;
    let mut peak_left = 0.0f32;
    let mut peak_right = 0.0f32;

    for i in 0..frames {
        let left = buffer[i * 2];
        let right = buffer[i * 2 + 1];

        sum_left += left * left;
        sum_right += right * right;

        peak_left = peak_left.max(left.abs());
        peak_right = peak_right.max(right.abs());
    }

    let rms_left = (sum_left / frames as f32).sqrt();
    let rms_right = (sum_right / frames as f32).sqrt();

    (rms_left, rms_right, peak_left, peak_right)
}
