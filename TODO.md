# Rack - Development TODO

## ✅ Completed

### Phase 1: AudioUnit Scanner (C++)
- [x] C++ AudioUnit scanner implementation
- [x] C API wrapper for Rust FFI
- [x] Component enumeration with metadata extraction
- [x] Two-pass scanning (count + fill)
- [x] Comprehensive error handling
- [x] Test program validating scanner
- [x] CI/CD pipeline for macOS
- [x] All PR review issues addressed

**Status**: C++ scanner is production-ready and tested with 128+ AudioUnit plugins.

### Phase 2: Rust FFI Integration
- [x] Manual FFI bindings in `src/au/ffi.rs`
- [x] Safe Rust wrapper in `src/au/scanner.rs`
- [x] Module organization in `src/au/mod.rs`
- [x] Shared utility functions in `src/au/util.rs`
- [x] Build system integration (build.rs, Cargo.toml)
- [x] Updated examples/list_plugins.rs
- [x] Comprehensive test coverage (11 tests)
- [x] Memory safety (MaybeUninit, bounded string conversion)
- [x] Thread safety (explicit Send/!Sync bounds)
- [x] Error handling (rack-specific vs OSStatus errors)
- [x] All code review feedback addressed
- [x] CI passing on macOS

**Status**: FFI integration complete. Scanner successfully enumerates 128+ AudioUnit plugins from Rust.

### Phase 3: Plugin Loading & Initialization
- [x] C++ AudioUnit instance management (`rack-sys/src/au_instance.cpp`)
- [x] AudioComponent lookup by unique_id
- [x] AudioComponentInstance creation and configuration
- [x] Audio format setup (stereo interleaved f32)
- [x] Buffer allocation with 16-byte alignment
- [x] Initialization/deinitialization lifecycle
- [x] Comprehensive error handling and cleanup
- [x] FFI bindings for instance management
- [x] Safe Rust wrapper (`src/au/instance.rs`)
- [x] AudioUnitPlugin with proper Drop implementation
- [x] Thread safety (Send but !Sync with PhantomData)
- [x] C++ tests (test_instance.cpp)
- [x] Rust integration tests (20 tests passing)
- [x] Memory leak prevention
- [x] All PR review issues addressed

**Status**: Plugin loading complete. Successfully loads and initializes AudioUnit plugins with proper resource management.

### Phase 4: Audio Processing with SIMD Optimization
- [x] Audio processing implementation (`rack_au_plugin_process()`)
- [x] Render callback architecture (pull-based)
- [x] Buffer format conversion (interleaved ↔ non-interleaved)
- [x] ARM NEON SIMD optimization (vld2q/vst2q for deinterleave/interleave)
- [x] x86_64 SSE2 SIMD optimization (shuffle/unpack operations)
- [x] 16-byte aligned AudioBuffer type using aligned-vec crate
- [x] C++20 aligned allocation (operator new with std::align_val_t)
- [x] Defensive unaligned SIMD operations for external buffers
- [x] Sample position tracking for AudioTimeStamp
- [x] Buffer size validation
- [x] Thread safety documentation
- [x] C++ tests with signal validation
- [x] Rust tests with real AudioUnit plugins
- [x] Example: process_audio.rs with RMS/peak analysis
- [x] Performance: ~4x speedup from SIMD on both ARM and x86_64
- [x] Memory safety: No leaks, proper cleanup paths
- [x] All PR review issues addressed (alignment, thread safety, buffer validation)

**Status**: Audio processing complete with production-ready SIMD optimizations. End-to-end aligned audio path from Rust → C++ → AudioUnit → C++ → Rust.

**Key Files**:
- `src/buffer.rs` - 16-byte aligned AudioBuffer using AVec
- `rack-sys/src/au_instance.cpp` - SIMD processing with ARM NEON and x86_64 SSE2
- `examples/process_audio.rs` - Complete audio processing demonstration

**Test Results**:
- 20/20 Rust tests passing
- 4/4 C++ tests passing
- 4/4 doctests passing
- All examples working

---

### Phase 5: Parameter Control
- [x] C++ parameter implementation (`rack-sys/src/au_instance.cpp`)
- [x] Parameter enumeration with `kAudioUnitProperty_ParameterList`
- [x] Parameter info with name, min/max, default values
- [x] Get/set parameter with normalization (0.0-1.0)
- [x] FFI bindings for parameter functions
- [x] Safe Rust wrapper (`src/au/instance.rs`)
- [x] Parameter methods in `AudioUnitPlugin`
- [x] C++ tests (Test 5: Parameter operations)
- [x] Rust integration tests (5 new parameter tests)
- [x] Example: `control_parameters.rs` with parameter demo
- [x] All PR review issues addressed

**Status**: Parameter control complete. Successfully enumerate, read, and write plugin parameters with automatic normalization.

**Key Files**:
- `rack-sys/src/au_instance.cpp` - Parameter implementation with AudioUnit API
- `src/au/instance.rs` - Safe Rust wrapper for parameter operations
- `examples/control_parameters.rs` - Complete parameter control demonstration

**Test Results**:
- 29/29 Rust tests passing (added 9 parameter tests total)
- 5/5 C++ tests passing (added parameter operations test)
- 4/4 doctests passing
- All examples working (including new control_parameters example)

**Performance Improvements**:
- Parameter info caching eliminates ~67% of API calls during automation
- Fast path uses cached info, falls back to on-demand queries
- Critical for real-time parameter automation scenarios

**Additional Enhancements**:
- Parameter unit support (Hz, dB, %, cents, etc.) - 27 unit types
- Thread-safety documentation (Send-but-not-Sync model)
- Parameter range validation and edge case handling
- CFString memory management documentation with Apple docs reference

---

## 📋 Future Phases

### Phase 6: MIDI Support
**Goal**: Send MIDI events to instrument plugins

Tasks:
- [ ] Implement MIDI event sending
- [ ] Note on/off support
- [ ] Control change (CC) support
- [ ] Program change support
- [ ] MIDI timing and scheduling
- [ ] Example: simple_synth.rs

### Phase 7: Preset Management
**Goal**: Load and save plugin presets

Tasks:
- [ ] Enumerate factory presets
- [ ] Load presets
- [ ] Save user presets
- [ ] Preset serialization
- [ ] Example: preset_browser.rs

### Phase 8: GUI Support (AudioUnit Focus)
**Goal**: Embed AudioUnit plugin GUIs in host applications

Tasks:
- [ ] Query `kAudioUnitProperty_CocoaUI` for custom views
- [ ] NSView integration for macOS
- [ ] Window/view lifecycle management
- [ ] GUI resize and scaling handling
- [ ] Generic parameter UI fallback
- [ ] Event handling (parameter changes from GUI)
- [ ] Example: plugin_gui.rs

**Note**: Focus on AudioUnit GUI first before adding other plugin formats. This provides complete AudioUnit hosting capabilities.

### Phase 9: Advanced Features
**Goal**: Production-ready hosting features

Tasks:
- [ ] Multi-threading support
- [ ] Plugin latency compensation
- [ ] Offline processing
- [ ] Plugin state serialization
- [ ] Crash isolation
- [ ] Plugin sandboxing
- [ ] Performance profiling and optimization

### Phase 10: Additional Plugin Formats (Deferred)
**Goal**: Support VST3, CLAP, and other formats

**Rationale**: Complete AudioUnit support first (including GUI) before adding other formats. This ensures a solid reference implementation.

Tasks:
- [ ] VST3 scanner
- [ ] VST3 plugin loading
- [ ] VST3 processing
- [ ] VST3 GUI support
- [ ] CLAP support (optional)
- [ ] Common trait abstraction across formats
- [ ] Format-agnostic examples

---

## 🎯 Immediate Next Steps

**Start Here** (Phase 6 - MIDI Support):

### Goal
Enable sending MIDI events to AudioUnit instrument plugins for note playback and control changes.

### 1. Implement C++ MIDI Functions

**File**: `rack-sys/include/rack_au.h`

```cpp
// MIDI event types
typedef enum {
    RACK_AU_MIDI_NOTE_ON = 0x90,
    RACK_AU_MIDI_NOTE_OFF = 0x80,
    RACK_AU_MIDI_CONTROL_CHANGE = 0xB0,
    RACK_AU_MIDI_PROGRAM_CHANGE = 0xC0,
} RackAUMidiEventType;

// MIDI event struct
typedef struct {
    uint32_t sample_offset;  // Sample offset within buffer
    uint8_t status;          // MIDI status byte
    uint8_t data1;           // First data byte (note/CC number)
    uint8_t data2;           // Second data byte (velocity/value)
    uint8_t channel;         // MIDI channel (0-15)
} RackAUMidiEvent;

// Send MIDI events to plugin
int rack_au_plugin_send_midi(
    RackAUPlugin* plugin,
    const RackAUMidiEvent* events,
    uint32_t event_count
);
```

**File**: `rack-sys/src/au_instance.cpp`

Implementation steps:
1. Use `MusicDeviceMIDIEvent()` for simple note on/off
2. Handle MIDI channel routing
3. Support sample-accurate timing with `AudioUnitRender` timestamps
4. Queue MIDI events for next `process()` call
5. Handle polyphony and note stealing

### 2. Add FFI Bindings

**File**: `src/au/ffi.rs`

```rust
#[repr(C)]
pub struct RackAUMidiEvent {
    pub sample_offset: u32,
    pub status: u8,
    pub data1: u8,
    pub data2: u8,
    pub channel: u8,
}

extern "C" {
    pub fn rack_au_plugin_send_midi(
        plugin: *mut RackAUPlugin,
        events: *const RackAUMidiEvent,
        event_count: u32,
    ) -> c_int;
}
```

### 3. Create Safe Rust API

**File**: `src/midi.rs` (new file)

```rust
pub struct MidiEvent {
    pub sample_offset: u32,
    pub kind: MidiEventKind,
}

pub enum MidiEventKind {
    NoteOn { note: u8, velocity: u8, channel: u8 },
    NoteOff { note: u8, velocity: u8, channel: u8 },
    ControlChange { controller: u8, value: u8, channel: u8 },
    ProgramChange { program: u8, channel: u8 },
}
```

**Update `src/traits.rs`**:
```rust
fn send_midi(&mut self, events: &[MidiEvent]) -> Result<()>;
```

### 4. Add Tests

**C++ tests** (`rack-sys/test/test_instance.cpp`):
```cpp
void test_midi_note_on_off();
void test_midi_control_change();
void test_midi_timing();
void test_midi_polyphony();
```

**Rust tests** (`src/au/instance.rs`):
```rust
#[test]
fn test_send_midi_note();

#[test]
fn test_send_midi_cc();

#[test]
fn test_midi_event_timing();
```

### 5. Create Example

**File**: `examples/simple_synth.rs`

```rust
use rack::prelude::*;
use rack::midi::*;

fn main() -> Result<()> {
    let scanner = Scanner::new()?;
    let plugins = scanner.scan()?;

    // Find an instrument plugin
    let synth = plugins.iter()
        .find(|p| p.plugin_type == PluginType::Instrument)
        .expect("No instrument plugins found");

    let mut plugin = scanner.load(synth)?;
    plugin.initialize(48000.0, 512)?;

    // Play a C major chord
    let events = vec![
        MidiEvent { sample_offset: 0, kind: MidiEventKind::NoteOn { note: 60, velocity: 100, channel: 0 }},
        MidiEvent { sample_offset: 0, kind: MidiEventKind::NoteOn { note: 64, velocity: 100, channel: 0 }},
        MidiEvent { sample_offset: 0, kind: MidiEventKind::NoteOn { note: 67, velocity: 100, channel: 0 }},
    ];

    plugin.send_midi(&events)?;

    // Process audio to render the notes
    let mut output = AudioBuffer::new(512 * 2);
    plugin.process(&AudioBuffer::new(512 * 2), &mut output)?;

    Ok(())
}
```

### Commands

```bash
# Build C++ changes
cd rack-sys/build
cmake --build .
./rack_sys_test_instance

# Build and test Rust
cd ~/devel/rack
cargo test

# Run example
cargo run --example simple_synth
```

---

## 📝 MIDI Implementation Notes

### AudioUnit MIDI API

AudioUnit provides two main methods for MIDI:
1. **MusicDeviceMIDIEvent()** - Simple, immediate MIDI events
2. **Render callback with MIDI** - Sample-accurate timing

Start with MusicDeviceMIDIEvent for simplicity.

### Sample-Accurate Timing

For proper timing:
- Queue MIDI events with sample offsets
- Send events during `AudioUnitRender` callback
- Respect buffer boundaries

### Polyphony Handling

- AudioUnit handles polyphony internally
- No need to track voice allocation
- Some plugins have polyphony limits

### Testing Strategy

1. **Unit tests**: Test MIDI event conversion
2. **Integration tests**: Test with real instrument plugins
3. **Manual testing**: Verify note playback sounds correct

---

## 📚 Resources

### AudioUnit MIDI Documentation
- [Music Device Properties](https://developer.apple.com/documentation/audiounit/music_effects)
- [MusicDeviceMIDIEvent Reference](https://developer.apple.com/documentation/audiounit/musicdevicemidievent)
- [Core MIDI Overview](https://developer.apple.com/documentation/coremidi)

### Success Criteria
- [ ] Send note on/off to instrument plugins
- [ ] Control change (CC) support
- [ ] Sample-accurate timing
- [ ] All tests passing (C++ and Rust)
- [ ] Example demonstrates chord playback
- [ ] No MIDI message loss or corruption
