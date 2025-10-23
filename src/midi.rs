//! MIDI event types and utilities
//!
//! This module provides safe, idiomatic Rust types for MIDI events.
//! These are converted to FFI types when sending to plugin instances.
//!
//! ## Supported MIDI Messages
//!
//! Phase 6 implements comprehensive MIDI 1.0 message support:
//!
//! ### Channel Messages
//! - **Note On/Off** - Trigger and release notes with velocity
//! - **Polyphonic Aftertouch** - Per-key pressure sensitivity
//! - **Control Change (CC)** - Modulation, expression, pedals, etc.
//! - **Program Change** - Switch between instrument patches
//! - **Channel Aftertouch** - Channel-wide pressure sensitivity
//! - **Pitch Bend** - Continuous pitch modulation (14-bit resolution)
//!
//! ### System Real-Time Messages
//! - **Timing Clock** - 24 pulses per quarter note for tempo sync
//! - **Start** - Start sequencer playback
//! - **Continue** - Resume sequencer playback
//! - **Stop** - Stop sequencer playback
//! - **Active Sensing** - Connection status monitoring
//! - **System Reset** - Reset all devices to power-on state
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
    /// Polyphonic Aftertouch (Key Pressure) event
    PolyphonicAftertouch {
        /// MIDI note number (0-127)
        note: u8,
        /// Pressure value (0-127)
        pressure: u8,
        /// MIDI channel (0-15)
        channel: u8,
    },
    /// Channel Aftertouch (Channel Pressure) event
    ChannelAftertouch {
        /// Pressure value (0-127)
        pressure: u8,
        /// MIDI channel (0-15)
        channel: u8,
    },
    /// Pitch Bend event
    PitchBend {
        /// Pitch bend value (0-16383, where 8192 = center/no bend)
        /// Lower values bend down, higher values bend up
        value: u16,
        /// MIDI channel (0-15)
        channel: u8,
    },
    /// MIDI Timing Clock (system real-time message)
    TimingClock,
    /// MIDI Start (system real-time message)
    Start,
    /// MIDI Continue (system real-time message)
    Continue,
    /// MIDI Stop (system real-time message)
    Stop,
    /// MIDI Active Sensing (system real-time message)
    ActiveSensing,
    /// MIDI System Reset (system real-time message)
    SystemReset,
}

impl MidiEvent {
    /// Pitch bend center value (no pitch change)
    pub const PITCH_BEND_CENTER: u16 = 8192;

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

    /// Create a new Polyphonic Aftertouch event
    ///
    /// # Arguments
    ///
    /// * `note` - MIDI note number (0-127, clamped if out of range)
    /// * `pressure` - Pressure value (0-127, clamped if out of range)
    /// * `channel` - MIDI channel (0-15, clamped if out of range)
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    pub fn polyphonic_aftertouch(note: u8, pressure: u8, channel: u8, sample_offset: u32) -> Self {
        Self {
            sample_offset,
            kind: MidiEventKind::PolyphonicAftertouch {
                note: note.min(127),
                pressure: pressure.min(127),
                channel: channel.min(15),
            },
        }
    }

    /// Create a new Channel Aftertouch event
    ///
    /// # Arguments
    ///
    /// * `pressure` - Pressure value (0-127, clamped if out of range)
    /// * `channel` - MIDI channel (0-15, clamped if out of range)
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    pub fn channel_aftertouch(pressure: u8, channel: u8, sample_offset: u32) -> Self {
        Self {
            sample_offset,
            kind: MidiEventKind::ChannelAftertouch {
                pressure: pressure.min(127),
                channel: channel.min(15),
            },
        }
    }

    /// Create a new Pitch Bend event
    ///
    /// # Arguments
    ///
    /// * `value` - Pitch bend value (0-16383, clamped if out of range, 8192 = center)
    /// * `channel` - MIDI channel (0-15, clamped if out of range)
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    ///
    /// # Examples
    ///
    /// ```
    /// use rack::midi::MidiEvent;
    ///
    /// // No pitch bend (center position)
    /// let event = MidiEvent::pitch_bend(8192, 0, 0);
    ///
    /// // Maximum pitch bend up
    /// let event = MidiEvent::pitch_bend(16383, 0, 0);
    ///
    /// // Maximum pitch bend down
    /// let event = MidiEvent::pitch_bend(0, 0, 0);
    /// ```
    pub fn pitch_bend(value: u16, channel: u8, sample_offset: u32) -> Self {
        Self {
            sample_offset,
            kind: MidiEventKind::PitchBend {
                value: value.min(16383),
                channel: channel.min(15),
            },
        }
    }

    /// Create a pitch bend event with no bend (centered)
    ///
    /// # Arguments
    ///
    /// * `channel` - MIDI channel (0-15, clamped if out of range)
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    ///
    /// # Examples
    ///
    /// ```
    /// use rack::midi::MidiEvent;
    ///
    /// // Reset pitch bend to center (no bend)
    /// let event = MidiEvent::pitch_bend_center(0, 0);
    /// ```
    pub fn pitch_bend_center(channel: u8, sample_offset: u32) -> Self {
        Self::pitch_bend(Self::PITCH_BEND_CENTER, channel, sample_offset)
    }

    /// Create a MIDI Timing Clock event
    ///
    /// # Arguments
    ///
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    pub fn timing_clock(sample_offset: u32) -> Self {
        Self {
            sample_offset,
            kind: MidiEventKind::TimingClock,
        }
    }

    /// Create a MIDI Start event
    ///
    /// # Arguments
    ///
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    pub fn start(sample_offset: u32) -> Self {
        Self {
            sample_offset,
            kind: MidiEventKind::Start,
        }
    }

    /// Create a MIDI Continue event
    ///
    /// # Arguments
    ///
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    pub fn continue_playback(sample_offset: u32) -> Self {
        Self {
            sample_offset,
            kind: MidiEventKind::Continue,
        }
    }

    /// Create a MIDI Stop event
    ///
    /// # Arguments
    ///
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    pub fn stop(sample_offset: u32) -> Self {
        Self {
            sample_offset,
            kind: MidiEventKind::Stop,
        }
    }

    /// Create a MIDI Active Sensing event
    ///
    /// # Arguments
    ///
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    pub fn active_sensing(sample_offset: u32) -> Self {
        Self {
            sample_offset,
            kind: MidiEventKind::ActiveSensing,
        }
    }

    /// Create a MIDI System Reset event
    ///
    /// # Arguments
    ///
    /// * `sample_offset` - Sample offset within buffer (0 = start of buffer)
    pub fn system_reset(sample_offset: u32) -> Self {
        Self {
            sample_offset,
            kind: MidiEventKind::SystemReset,
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

    #[test]
    fn test_pitch_bend_center() {
        // Test the PITCH_BEND_CENTER constant
        assert_eq!(MidiEvent::PITCH_BEND_CENTER, 8192);

        // Test pitch_bend_center helper
        let event = MidiEvent::pitch_bend_center(0, 0);
        match event.kind {
            MidiEventKind::PitchBend { value, channel } => {
                assert_eq!(value, 8192);
                assert_eq!(channel, 0);
            }
            _ => panic!("Expected PitchBend event"),
        }

        // Test that manual creation is equivalent to helper
        let manual = MidiEvent::pitch_bend(MidiEvent::PITCH_BEND_CENTER, 5, 100);
        let helper = MidiEvent::pitch_bend_center(5, 100);
        assert_eq!(manual.kind, helper.kind);
        assert_eq!(manual.sample_offset, helper.sample_offset);
    }
}
