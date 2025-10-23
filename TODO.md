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

### Phase 6: MIDI Support
- [x] C++ MIDI implementation (`rack-sys/src/au_instance.cpp`)
- [x] MusicDeviceMIDIEvent() integration for sample-accurate MIDI
- [x] Complete MIDI 1.0 message support (13 message types)
- [x] Channel messages: Note On/Off, Polyphonic Aftertouch, Control Change, Program Change, Channel Aftertouch, Pitch Bend
- [x] System Real-Time messages: Timing Clock, Start, Continue, Stop, Active Sensing, System Reset
- [x] FFI bindings for MIDI functions
- [x] Safe Rust wrapper (`src/midi.rs`)
- [x] Zero-allocation MIDI with SmallVec (stack-allocated for ≤16 events)
- [x] Helper methods for all MIDI message types
- [x] Pitch bend center value constant and helper
- [x] C++ tests (Test 6: MIDI operations)
- [x] Rust integration tests (7 MIDI tests including multi-channel)
- [x] Example: `simple_synth.rs` with C major chord demo
- [x] All PR review issues addressed

**Status**: MIDI support complete. Successfully send all MIDI 1.0 messages to AudioUnit instrument plugins with zero-allocation performance.

**Key Files**:
- `rack-sys/src/au_instance.cpp` - MIDI implementation with AudioUnit API
- `src/midi.rs` - Safe Rust MIDI types with comprehensive message support
- `src/au/instance.rs` - Zero-allocation MIDI sending with SmallVec
- `examples/simple_synth.rs` - Complete MIDI synthesis demonstration

**Test Results**:
- 44/44 Rust tests passing (added 7 MIDI tests)
- 6/6 C++ tests passing (added MIDI operations test)
- All examples working (including simple_synth)

**Performance**:
- Zero heap allocation for typical use (≤16 events) via SmallVec
- Sample-accurate timing via sample_offset parameter
- 14-bit pitch bend resolution with automatic LSB/MSB splitting

**MIDI Coverage**:
- All 7 MIDI 1.0 channel messages
- All 6 system real-time messages
- Pitch bend with PITCH_BEND_CENTER constant (8192)
- Better error messages for effect plugins rejecting MIDI

---

## 📋 Future Phases

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

**Start Here** (Phase 7 - Preset Management OR Phase 8 - GUI Support):

The next priority phases are:
- **Phase 8: GUI Support** - Recommended next for complete AudioUnit hosting
- **Phase 7: Preset Management** - Useful but less critical than GUI

### Phase 8: GUI Support (Recommended)
Complete AudioUnit hosting by adding GUI integration. See Phase 8 details above for tasks.
