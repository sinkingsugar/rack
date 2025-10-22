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

## Current Phase: AudioUnit Scanner

**Goal:** Enumerate AudioUnit plugins on macOS

**Tasks:**
1. ✅ Set up project structure
2. ✅ Define Rust traits and types
3. 🚧 Implement C++ AudioUnit scanner
4. 🚧 Wire C++ to Rust via FFI
5. ⏳ Test with real AudioUnit plugins
6. ⏳ Implement plugin loading
7. ⏳ Implement audio processing

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

- This is a greenfield project, early stage
- Focus on getting AudioUnit working first
- Don't over-engineer - KISS principle
- Giovanni knows C++ well, leverage that
- Check rack/CLAUDE.md and rack-sys/CLAUDE.md for component-specific details
