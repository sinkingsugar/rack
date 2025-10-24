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

    // Create audio buffers (planar format - separate buffer per channel)
    let left_in = vec![0.0f32; buffer_frames];
    let right_in = vec![0.0f32; buffer_frames];
    let mut left_out = vec![0.0f32; buffer_frames];
    let mut right_out = vec![0.0f32; buffer_frames];

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
        // Process audio (synth renders MIDI notes to output, planar format)
        plugin.process(
            &[&left_in, &right_in],
            &mut [&mut left_out, &mut right_out],
            buffer_frames
        )?;

        // Calculate RMS and peak levels for this buffer
        let (rms_left, rms_right, peak_left, peak_right) = analyze_buffer(&left_out, &right_out);

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
        plugin.process(
            &[&left_in, &right_in],
            &mut [&mut left_out, &mut right_out],
            buffer_frames
        )?;

        let (rms_left, rms_right, peak_left, peak_right) = analyze_buffer(&left_out, &right_out);

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

/// Calculate RMS and peak levels for left and right channels (planar format)
fn analyze_buffer(
    left: &[f32],
    right: &[f32],
) -> (f32, f32, f32, f32) {
    let frames = left.len();

    let mut sum_left = 0.0f32;
    let mut sum_right = 0.0f32;
    let mut peak_left = 0.0f32;
    let mut peak_right = 0.0f32;

    for i in 0..frames {
        sum_left += left[i] * left[i];
        sum_right += right[i] * right[i];

        peak_left = peak_left.max(left[i].abs());
        peak_right = peak_right.max(right[i].abs());
    }

    let rms_left = (sum_left / frames as f32).sqrt();
    let rms_right = (sum_right / frames as f32).sqrt();

    (rms_left, rms_right, peak_left, peak_right)
}
