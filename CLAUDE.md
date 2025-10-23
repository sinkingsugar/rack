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

## Current Status: Phase 5 Complete

**Completed Phases:**
1. ✅ Phase 1: AudioUnit Scanner (C++)
2. ✅ Phase 2: Rust FFI Integration
3. ✅ Phase 3: Plugin Loading & Initialization
4. ✅ Phase 4: Audio Processing (SIMD optimized)
5. ✅ Phase 5: Parameter Control

**Next Phase:** Phase 6 - MIDI Support

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

- This project has completed Phases 1-5 and is production-ready for AudioUnit parameter control
- Focus continues on AudioUnit on macOS - MIDI support is next
- KISS principle guides all design decisions
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
