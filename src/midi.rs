//! MIDI event types and utilities
//!
//! This module provides safe, idiomatic Rust types for MIDI events.
//! These are converted to FFI types when sending to plugin instances.
//!
//! ## Supported MIDI Messages
//!
//! Phase 6 implements the most common MIDI messages for instrument control:
//!
//! - **Note On/Off** - Trigger and release notes with velocity
//! - **Control Change (CC)** - Modulation, expression, pedals, etc.
//! - **Program Change** - Switch between instrument patches
//!
//! ## Planned for Future Phases
//!
//! Additional MIDI message types will be added in Phase 6.1 or Phase 9:
//!
//! - **Pitch Bend** - Continuous pitch modulation (very common in synthesizers)
//! - **Aftertouch** - Pressure sensitivity (polyphonic and channel)
//! - **System Messages** - MIDI clock, start/stop for sequencers
//!
//! ## Sample-Accurate Timing
//!
//! All events support sample-accurate timing via the `sample_offset` field,
//! which specifies the frame offset within the current audio buffer where
//! the event should be applied (0 = start of buffer).

/// A MIDI event with sample-accurate timing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiEvent {
    /// Sample offset within the current audio buffer (0 = start of buffer)
    pub sample_offset: u32,
    /// The type of MIDI event and its data
    pub kind: MidiEventKind,
}

/// Type and data for a MIDI event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiEventKind {
    /// Note On event
    NoteOn {
        /// MIDI note number (0-127, where 60 = middle C)
        note: u8,
        /// Velocity (0-127, where 0 is typically treated as Note Off)
        velocity: u8,
        /// MIDI channel (0-15)
        channel: u8,
    },
    /// Note Off event
    NoteOff {
        /// MIDI note number (0-127)
        note: u8,
        /// Release velocity (0-127)
        velocity: u8,
        /// MIDI channel (0-15)
        channel: u8,
    },
    /// Control Change (CC) event
    ControlChange {
        /// Controller number (0-127)
        controller: u8,
        /// Controller value (0-127)
        value: u8,
        /// MIDI channel (0-15)
        channel: u8,
    },
    /// Program Change event
    ProgramChange {
        /// Program number (0-127)
        program: u8,
        /// MIDI channel (0-15)
        channel: u8,
    },
}

impl MidiEvent {
    /// Create a new Note On event
    ///
    /// # Arguments
    ///
    /// * `note` - MIDI note number (0-127, clamped if out of range)
    /// * `velocity` - Note velocity (0-127, clamped if out of range)
    /// * `channel` - MIDI channel (0-15, clamped if out of range)
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    ///
    /// # Examples
    ///
    /// ```
    /// use rack::midi::MidiEvent;
    ///
    /// // Middle C (note 60) on channel 0
    /// let event = MidiEvent::note_on(60, 100, 0, 0);
    /// ```
    pub fn note_on(note: u8, velocity: u8, channel: u8, sample_offset: u32) -> Self {
        Self {
            sample_offset,
            kind: MidiEventKind::NoteOn {
                note: note.min(127),
                velocity: velocity.min(127),
                channel: channel.min(15),
            },
        }
    }

    /// Create a new Note Off event
    ///
    /// # Arguments
    ///
    /// * `note` - MIDI note number (0-127, clamped if out of range)
    /// * `velocity` - Release velocity (0-127, clamped if out of range)
    /// * `channel` - MIDI channel (0-15, clamped if out of range)
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    ///
    /// # Examples
    ///
    /// ```
    /// use rack::midi::MidiEvent;
    ///
    /// // Release middle C on channel 0
    /// let event = MidiEvent::note_off(60, 64, 0, 0);
    /// ```
    pub fn note_off(note: u8, velocity: u8, channel: u8, sample_offset: u32) -> Self {
        Self {
            sample_offset,
            kind: MidiEventKind::NoteOff {
                note: note.min(127),
                velocity: velocity.min(127),
                channel: channel.min(15),
            },
        }
    }

    /// Create a new Control Change event
    ///
    /// # Arguments
    ///
    /// * `controller` - Controller number (0-127, clamped if out of range)
    /// * `value` - Controller value (0-127, clamped if out of range)
    /// * `channel` - MIDI channel (0-15, clamped if out of range)
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    ///
    /// # Examples
    ///
    /// ```
    /// use rack::midi::MidiEvent;
    ///
    /// // Set modulation wheel (CC 1) to half on channel 0
    /// let event = MidiEvent::control_change(1, 64, 0, 0);
    /// ```
    pub fn control_change(controller: u8, value: u8, channel: u8, sample_offset: u32) -> Self {
        Self {
            sample_offset,
            kind: MidiEventKind::ControlChange {
                controller: controller.min(127),
                value: value.min(127),
                channel: channel.min(15),
            },
        }
    }

    /// Create a new Program Change event
    ///
    /// # Arguments
    ///
    /// * `program` - Program number (0-127, clamped if out of range)
    /// * `channel` - MIDI channel (0-15, clamped if out of range)
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    ///
    /// # Examples
    ///
    /// ```
    /// use rack::midi::MidiEvent;
    ///
    /// // Select program 5 on channel 0
    /// let event = MidiEvent::program_change(5, 0, 0);
    /// ```
    pub fn program_change(program: u8, channel: u8, sample_offset: u32) -> Self {
        Self {
            sample_offset,
            kind: MidiEventKind::ProgramChange {
                program: program.min(127),
                channel: channel.min(15),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_on_creation() {
        let event = MidiEvent::note_on(60, 100, 0, 0);
        assert_eq!(event.sample_offset, 0);
        match event.kind {
            MidiEventKind::NoteOn { note, velocity, channel } => {
                assert_eq!(note, 60);
                assert_eq!(velocity, 100);
                assert_eq!(channel, 0);
            }
            _ => panic!("Expected NoteOn event"),
        }
    }

    #[test]
    fn test_note_off_creation() {
        let event = MidiEvent::note_off(60, 64, 0, 100);
        assert_eq!(event.sample_offset, 100);
        match event.kind {
            MidiEventKind::NoteOff { note, velocity, channel } => {
                assert_eq!(note, 60);
                assert_eq!(velocity, 64);
                assert_eq!(channel, 0);
            }
            _ => panic!("Expected NoteOff event"),
        }
    }

    #[test]
    fn test_control_change_creation() {
        let event = MidiEvent::control_change(1, 64, 0, 0);
        match event.kind {
            MidiEventKind::ControlChange { controller, value, channel } => {
                assert_eq!(controller, 1);
                assert_eq!(value, 64);
                assert_eq!(channel, 0);
            }
            _ => panic!("Expected ControlChange event"),
        }
    }

    #[test]
    fn test_program_change_creation() {
        let event = MidiEvent::program_change(5, 0, 0);
        match event.kind {
            MidiEventKind::ProgramChange { program, channel } => {
                assert_eq!(program, 5);
                assert_eq!(channel, 0);
            }
            _ => panic!("Expected ProgramChange event"),
        }
    }

    #[test]
    fn test_value_clamping() {
        // Test that values are clamped to valid MIDI ranges
        let event = MidiEvent::note_on(200, 200, 20, 0);
        match event.kind {
            MidiEventKind::NoteOn { note, velocity, channel } => {
                assert_eq!(note, 127);  // Clamped from 200
                assert_eq!(velocity, 127);  // Clamped from 200
                assert_eq!(channel, 15);  // Clamped from 20
            }
            _ => panic!("Expected NoteOn event"),
        }
    }
}
