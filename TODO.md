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
- 25/25 Rust tests passing (added 5 parameter tests)
- 5/5 C++ tests passing (added parameter operations test)
- 4/4 doctests passing
- All examples working (including new control_parameters example)

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

### Phase 8: Additional Plugin Formats
**Goal**: Support VST3, CLAP, and other formats

Tasks:
- [ ] VST3 scanner
- [ ] VST3 plugin loading
- [ ] VST3 processing
- [ ] CLAP support (optional)
- [ ] Common trait abstraction across formats
- [ ] Format-agnostic examples

### Phase 9: GUI Support
**Goal**: Embed plugin GUIs in host applications

Tasks:
- [ ] Cocoa view support (macOS)
- [ ] Window/view management
- [ ] GUI resize handling
- [ ] Generic parameter UI fallback

### Phase 10: Advanced Features
**Goal**: Production-ready hosting features

Tasks:
- [ ] Multi-threading support
- [ ] Plugin latency compensation
- [ ] Offline processing
- [ ] Plugin state serialization
- [ ] Crash isolation
- [ ] Plugin sandboxing

---

## 🎯 Immediate Next Steps

**Start Here** (Phase 5 - Parameter Control):

### 1. Implement C++ Parameter Functions

**File**: `rack-sys/src/au_instance.cpp`

```cpp
// Add to rack_au.h
extern "C" {
    int rack_au_plugin_parameter_count(RackAUPlugin* plugin);

    int rack_au_plugin_parameter_info(
        RackAUPlugin* plugin,
        uint32_t index,
        char* name_out,
        uint32_t name_len,
        float* min_value,
        float* max_value,
        float* default_value
    );

    int rack_au_plugin_get_parameter(
        RackAUPlugin* plugin,
        uint32_t index,
        float* value_out
    );

    int rack_au_plugin_set_parameter(
        RackAUPlugin* plugin,
        uint32_t index,
        float value
    );
}
```

Implementation steps:
1. Query `kAudioUnitProperty_ParameterList` for parameter count
2. Query `kAudioUnitProperty_ParameterInfo` for each parameter
3. Use `AudioUnitGetParameter()` for getting values
4. Use `AudioUnitSetParameter()` for setting values
5. Handle parameter scope (global/input/output)
6. Normalize values to 0.0-1.0 range

### 2. Add FFI Bindings

**File**: `src/au/ffi.rs`

Add extern declarations for parameter functions.

### 3. Update Rust Wrapper

**File**: `src/au/instance.rs`

Implement `PluginInstance` trait methods:
- `parameter_count()` - already in trait
- `parameter_info(index)` - already in trait
- `get_parameter(index)` - already in trait
- `set_parameter(index, value)` - already in trait

### 4. Add Tests

**C++ tests** (`rack-sys/test/test_instance.cpp`):
```cpp
void test_parameter_enumeration();
void test_parameter_get_set();
void test_parameter_out_of_bounds();
```

**Rust tests** (`src/au/instance.rs`):
```rust
#[test]
fn test_parameter_count();

#[test]
fn test_parameter_info();

#[test]
fn test_get_set_parameter();

#[test]
fn test_parameter_range_validation();
```

### 5. Create Example

**File**: `examples/control_parameters.rs`

Demonstrate:
- Listing all parameters
- Getting current values
- Setting new values
- Parameter name/range display

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
cargo run --example control_parameters

# Expected output:
# Parameters:
#   [0] Frequency: 440.00 Hz (20.00 - 20000.00)
#   [1] Resonance: 0.50 (0.00 - 1.00)
#   ...
# Set parameter 0 to 0.75
# New value: 15005.00 Hz
```

---

## 📝 Implementation Notes

### Parameter Normalization

AudioUnit parameters have different ranges (e.g., frequency 20-20000 Hz, resonance 0-1). The API normalizes all parameters to 0.0-1.0:

```rust
// Internal conversion
normalized = (value - min) / (max - min);
actual = min + (normalized * (max - min));
```

### Thread Safety

Parameter changes should be thread-safe and can be called from any thread. However:
- Parameter changes during `process()` are safe but may cause audio glitches
- Consider implementing parameter smoothing in future
- Document that `process()` itself is still !Sync

### Parameter Types

AudioUnit parameters can have different types:
- Generic (continuous float)
- Indexed (discrete selection)
- Boolean (on/off)
- String (text input)

Start with generic parameters, add others in future phases.

### Testing Strategy

1. **Unit tests**: Test parameter functions in isolation
2. **Integration tests**: Test with real AudioUnit plugins
3. **Example**: Interactive demonstration
4. **Manual testing**: Verify parameter changes affect audio output

---

## 🔍 Design Decisions

### Why Normalize Parameters?

- **Consistency**: All parameters use same 0.0-1.0 range regardless of actual units
- **Automation**: Easier to automate without knowing parameter ranges
- **UI**: Simpler to build generic parameter controls
- **Serialization**: Normalized values are more portable

### Why Start with Get/Set?

Before implementing automation, presets, or MIDI CC mapping, we need basic get/set functionality. This is the foundation for all advanced features.

### Thread Safety Model

- Parameters can be changed from any thread (Send)
- But not from multiple threads simultaneously (!Sync)
- This matches the AudioUnit API design

---

## 📚 Resources

### AudioUnit Documentation
- [Audio Unit Programming Guide - Parameters](https://developer.apple.com/library/archive/documentation/MusicAudio/Conceptual/AudioUnitProgrammingGuide/TheAudioUnit/TheAudioUnit.html#//apple_ref/doc/uid/TP40003278-CH12-SW18)
- [AudioUnitProperties.h](https://developer.apple.com/documentation/audiounit/audiounitpropertyid)

### Current Implementation
- C++ implementation: `rack-sys/src/au_instance.cpp`
- Rust wrapper: `src/au/instance.rs`
- Trait definition: `src/traits.rs`
- Examples: `examples/process_audio.rs`

### Test Coverage
- Current: 20 Rust tests, 4 C++ tests
- Target: Add 8+ parameter tests (4 C++, 4+ Rust)
