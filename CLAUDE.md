# Rack - Audio Plugin Hosting Library

## Project Overview

**Rack** is a modern Rust library for hosting audio plugins (VST3, AudioUnit, CLAP, etc.) in applications.

**Current Status:** Early development - AudioUnit support for macOS in progress

## Architecture

The project is split into two main components:

1. **rack-sys/** - C++ wrapper around native plugin APIs (AudioUnit, VST3, etc.)
2. **rack/** - Rust library providing safe, idiomatic API

This hybrid approach allows us to:
- Use native C++ APIs the way they were designed
- Leverage C++ expertise for complex FFI work
- Provide clean, safe Rust API to users
- Debug C++ independently before Rust integration

## Project Structure

```
~/devel/rack/
├── CLAUDE.md              # This file - project overview
├── README.md              # User-facing documentation
├── rack-sys/              # C++ wrapper (see rack-sys/CLAUDE.md)
│   ├── CMakeLists.txt
│   ├── include/
│   └── src/
└── rack/                  # Rust library (see rack/CLAUDE.md)
    ├── Cargo.toml
    ├── build.rs
    ├── src/
    └── examples/
```

## Development Philosophy

**KISS (Keep It Simple, Stupid)**
- Start with AudioUnit on macOS only
- Get one thing working before adding more
- Iterate fast, validate early
- Add complexity only when needed

## Current Status: Phase 8 Complete - AudioUnit Support COMPLETE

**Completed Phases:**
1. ✅ Phase 1: AudioUnit Scanner (C++)
2. ✅ Phase 2: Rust FFI Integration
3. ✅ Phase 3: Plugin Loading & Initialization
4. ✅ Phase 4: Audio Processing (SIMD optimized)
5. ✅ Phase 5: Parameter Control
6. ✅ Phase 6: MIDI Support (complete MIDI 1.0, zero-allocation)
7. ✅ Phase 7: Preset Management (factory presets + state serialization)
8. ✅ Phase 8: GUI Support (AUv3/AUv2/generic fallback)

**Status:** Rack provides complete AudioUnit hosting capabilities on macOS. All core features implemented and production-ready.

**Next Phase:** Phase 9 (Advanced Features) or Phase 10 (Additional Plugin Formats - VST3/CLAP)

See TODO.md for detailed phase breakdown and implementation status.

## Key Design Decisions

### Why C++ wrapper?
- AudioUnit API is C/C++ native
- Easier to debug and iterate
- Can use Apple's example code directly
- Giovanni is C++ expert

### Why cmake crate?
- Mature, handles cross-compilation
- Integrates well with Cargo
- Handles framework linking on macOS

### Why trait-based design?
- Easy to add VST3, CLAP later
- Users can write format-agnostic code
- Clean abstraction boundaries

## Performance and Optimization Principles

**Real-time audio requires zero-allocation hot paths:**
- Prefer fixing performance issues over documenting them
- Use stack allocation (SmallVec, arrays) over heap when possible
- Measure and optimize critical paths (audio processing, MIDI)
- Real-time code must be lock-free and allocation-free

**Examples of this principle:**
- MIDI events use `SmallVec<[_; 16]>` for zero-allocation in typical cases
- Audio buffers use planar format with zero-copy pointer assignment (no memcpy in hot path)
- AudioBufferList structures point directly at caller's buffers (eliminated 2 of 3 memcpy operations)
- Pre-allocated pointer arrays reused across process() calls
- Future: Consider object pools for large event batches

## Building

```bash
# Build everything (Rust + C++)
cd ~/devel/rack/rack
cargo build

# Run examples
cargo run --example list_plugins
cargo run --example simple_host --features cpal
```

## Testing

```bash
# Rust tests
cargo test

# C++ tests (when we add them)
cd rack-sys/build
cmake --build . --target test
```

## Next Steps

1. Implement AudioComponent enumeration in C++
2. Create C API wrapper
3. Generate Rust bindings
4. Wire up scanner
5. Test with system AudioUnits

## Notes for Future Claude Sessions

- **AudioUnit support is COMPLETE** (Phases 1-8): scanning, loading, processing, parameters, MIDI, presets, GUI
- Phase 8 (GUI) implements AUv3 → AUv2 → Generic UI fallback with async design
- **Critical thread-safety fix**: Global mutex (`g_audio_unit_cleanup_mutex`) in `au_instance.cpp` serializes AudioUnit lifecycle operations (AudioComponentInstanceNew, AudioUnitInitialize, AudioUnitUninitialize, AudioComponentInstanceDispose) to prevent Apple AudioUnit framework race conditions
- Init/deinit are cold paths (already allocate/do I/O), mutex overhead is negligible
- AudioUnitRender stays lock-free (hot path unaffected)
- **Zero-copy audio processing** (v0.3.0): Planar audio API with pointer assignment instead of memcpy
  - AudioBufferList structures point directly at caller's buffers (eliminated 2 of 3 memcpy operations)
  - Only remaining copy is in input_render_callback (unavoidable - AudioUnit provides the buffer)
  - Pre-allocated pointer arrays (`Vec<*const f32>`, `Vec<*mut f32>`) reused across process() calls
  - Dynamic channel count support (no hardcoded stereo limitations)
  - **Validation in Rust layer**: All input validation happens in Rust (public API), C++ trusts validated inputs
- Phase 6 (MIDI) uses SmallVec for zero-allocation performance
- KISS principle guides all design decisions
- **Performance over documentation**: Fix performance issues, don't just document them
- Giovanni (user) knows C++ well, leverage that expertise
- Check rack/CLAUDE.md and rack-sys/CLAUDE.md for component-specific details
- See TODO.md for current phase status and next steps

## Multi-Claude Review Process

This project uses a **multi-Claude feedback loop** for code quality:

1. **Implementation Claude** writes the initial code and tests
2. **Review Claude** performs detailed code review, identifying:
   - Critical issues (memory leaks, undefined behavior, performance)
   - Medium priority issues (documentation, edge cases, API design)
   - Low priority enhancements (test coverage, minor improvements)
3. **Implementation Claude** addresses all feedback systematically
4. Process repeats until code quality standards are met

**Why this works:**
- Different Claude instances have independent perspectives
- Catches issues that single-pass development might miss
- Ensures thorough testing, documentation, and edge case handling
- Results in production-ready code with comprehensive coverage

**Note for Claude instances:** When you see review feedback from "another Claude", it's part of this iterative quality process. Address all issues systematically, commit fixes, and update this file if the feedback introduces new patterns or learnings.
