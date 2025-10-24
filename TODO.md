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
- [x] **Planar audio API refactor (v0.3.0)**: Non-interleaved format throughout
- [x] **Zero-copy optimization**: AudioBufferList points directly at caller's buffers
- [x] Eliminated 2 of 3 memcpy operations (only input_render_callback remains)
- [x] Pre-allocated pointer arrays reused across process() calls
- [x] Dynamic channel count support (removed hardcoded stereo limitations)
- [x] Channel count validation moved to Rust layer (C++ trusts validated inputs)
- [x] Comprehensive test coverage for channel mismatches and buffer length validation

**Status**: Audio processing complete with production-ready SIMD optimizations and zero-copy performance. Planar API with minimal memcpy overhead.

**Key Files**:
- `src/au/instance.rs` - Zero-copy process() with pre-allocated pointer arrays
- `rack-sys/src/au_instance.cpp` - Zero-copy AudioBufferList + SIMD processing
- `examples/process_audio.rs` - Complete audio processing demonstration

**Test Results**:
- 53/53 Rust tests passing (includes channel validation and buffer length tests)
- All examples working
- Zero allocations in hot path (process() call)

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

### Phase 7: Preset Management
- [x] C++ preset implementation (`rack-sys/src/au_instance.cpp`)
- [x] Factory preset enumeration with `kAudioUnitProperty_FactoryPresets`
- [x] Preset loading with `kAudioUnitProperty_PresentPreset`
- [x] State serialization with `kAudioUnitProperty_ClassInfo`
- [x] Get/set state with CFPropertyList binary serialization
- [x] FFI bindings for preset functions
- [x] Safe Rust wrapper (`src/au/instance.rs`)
- [x] PresetInfo struct in `src/plugin_info.rs`
- [x] C++ tests (Test 7: Preset operations)
- [x] Rust integration tests (9 preset tests)
- [x] Example: `preset_browser.rs` with preset demo
- [x] All PR review issues addressed

**Status**: Preset management complete. Successfully enumerate, load factory presets, and save/restore full plugin state with CFPropertyList serialization.

**Key Files**:
- `rack-sys/src/au_instance.cpp` - Preset implementation with AudioUnit API
- `src/plugin_info.rs` - PresetInfo struct
- `src/au/instance.rs` - Safe Rust wrapper for preset operations
- `examples/preset_browser.rs` - Complete preset browsing demonstration

**Test Results**:
- 53/53 Rust tests passing (added 9 preset tests)
- 7/7 C++ tests passing (added preset operations test)
- All examples working (including new preset_browser example)

**Implementation Details**:
- Factory presets via `kAudioUnitProperty_FactoryPresets` (CFArrayRef of AUPreset)
- Preset loading via `kAudioUnitProperty_PresentPreset`
- Full state serialization via `kAudioUnitProperty_ClassInfo` (CFPropertyList)
- Binary format using `kCFPropertyListBinaryFormat_v1_0`
- Two-pass state access: get size, then get data
- CFString/CFPropertyList memory management with proper CFRelease

**Code Review Fixes**:
- CFStringGetCString return value validation (buffer overflow protection)
- Integer overflow protection for state size (INT_MAX bounds checking)
- RAII memory management in C++ tests (std::unique_ptr)
- Removed strict CFPropertyList type validation (was causing SIGBUS in CI)

---

### Phase 8: GUI Support
- [x] C++ GUI implementation (`rack-sys/src/au_gui.mm` - Objective-C++)
- [x] AUv3 modern GUI support (AUAudioUnit + requestViewController)
- [x] AUv2 legacy GUI support (kAudioUnitProperty_CocoaUI)
- [x] Generic parameter UI fallback (NSStackView with sliders)
- [x] Three-tier fallback: AUv3 → AUv2 → Generic UI
- [x] Async GUI creation with callbacks
- [x] NSView/NSViewController lifecycle management
- [x] Window management (show/hide/destroy)
- [x] View size queries
- [x] FFI bindings for GUI functions
- [x] Safe Rust wrapper (`src/au/gui.rs`)
- [x] AudioUnitGui with proper Drop implementation
- [x] Thread safety (Send but !Sync)
- [x] Main thread enforcement for GUI operations
- [x] Global mutex for AudioUnit lifecycle (init/uninit/dispose)
- [x] C++ tests (test_gui.cpp)
- [x] Rust integration tests (2 trait tests)
- [x] Example: `plugin_gui.rs` with async GUI demo
- [x] All build errors resolved

**Status**: GUI support complete. Successfully creates and displays AudioUnit plugin GUIs with AUv3/AUv2 support and generic fallback. Global mutex prevents Apple AudioUnit framework thread-safety crashes.

**Key Files**:
- `rack-sys/src/au_gui.mm` - GUI implementation with AUv3/AUv2/generic support
- `rack-sys/include/rack_au.h` - C API for GUI functions
- `src/au/gui.rs` - Safe Rust wrapper for GUI operations
- `src/au/instance.rs` - create_gui() method with async callback
- `examples/plugin_gui.rs` - Complete GUI demonstration

**Test Results**:
- 55/55 Rust tests passing
- Parallel test execution fixed (AudioUnit lifecycle mutex)
- All examples working (including new plugin_gui example)

**Implementation Details**:
- **AUv3 support**: Uses `AUAudioUnit.instantiateWithComponentDescription` + `requestViewControllerWithCompletionHandler`
- **AUv2 support**: Uses `kAudioUnitProperty_CocoaUI` + NSBundle dynamic loading
- **Generic UI**: Creates NSStackView with NSSlider for each parameter (up to 20 params)
- **Async design**: All GUI operations dispatch to main queue (AppKit requirement)
- **Memory management**: ARC for Objective-C, proper ownership tracking for views/controllers
- **Thread safety**: Global `std::mutex` serializes AudioComponentInstanceNew, AudioUnitInitialize, AudioUnitUninitialize, AudioComponentInstanceDispose

**Critical Fix - Thread Safety**:
- Apple's AudioUnit framework has race conditions in Component Manager during concurrent init/deinit
- Added global mutex (`g_audio_unit_cleanup_mutex`) to serialize lifecycle operations
- Init/deinit are cold paths (already allocate/do I/O), mutex overhead is negligible
- AudioUnitRender stays lock-free (hot path unaffected)
- Prevents intermittent SIGSEGV crashes in parallel test execution

---

## 📋 Future Phases

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

**Rationale**: AudioUnit support is now complete (Phases 1-8). VST3/CLAP support can be added as additional plugin formats using the same architecture pattern.

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

**AudioUnit Support: ✅ COMPLETE** (Phases 1-8)

Rack now provides complete AudioUnit hosting capabilities:
- ✅ Plugin scanning and enumeration
- ✅ Plugin loading and initialization
- ✅ Audio processing with SIMD optimization
- ✅ Parameter control with caching
- ✅ MIDI support (zero-allocation)
- ✅ Preset management
- ✅ GUI support (AUv3/AUv2/generic fallback)

**Next Priority**: Phase 9 (Advanced Features) or Phase 10 (Additional Plugin Formats)

## 📝 Recent Updates

**2025-10-24**: Phase 8 (GUI Support) merged to main
- Complete AUv3/AUv2/Generic UI support
- Interactive parameter sliders in generic UI
- Critical thread-safety fixes (global mutex for AudioUnit lifecycle)
- All memory safety issues addressed
- Production-ready GUI hosting
