# rack - Rust Audio Plugin Hosting Library

## Purpose

This is the Rust library that provides a safe, idiomatic API for hosting audio plugins. It wraps the C++ layer (rack-sys) with Rust types and error handling.

## Current Status

**Phase 1: AudioUnit Scanner**
- ✅ Trait definitions
- ✅ Type system (PluginInfo, ParameterInfo, etc.)
- ✅ Error handling
- 🚧 FFI bindings to rack-sys
- 🚧 AudioUnit scanner implementation
- ⏳ AudioUnit plugin instance
- ⏳ Audio processing

## Architecture

```
rack/
├── Cargo.toml           # Dependencies, features
├── build.rs             # Builds rack-sys via cmake crate
├── src/
│   ├── lib.rs           # Main entry, re-exports
│   ├── error.rs         # Error types
│   ├── plugin_info.rs   # PluginInfo, ParameterInfo structs
│   ├── traits.rs        # PluginScanner, PluginInstance traits
│   └── au/              # AudioUnit implementation
│       ├── mod.rs       # Module exports
│       ├── ffi.rs       # Raw FFI bindings to rack-sys
│       ├── scanner.rs   # Scanner impl
│       └── instance.rs  # Plugin instance impl
└── examples/
    ├── list_plugins.rs  # List available plugins
    └── simple_host.rs   # Audio playback with cpal
```

## Key Design Principles

### 1. Trait-Based API

Users interact with traits, not concrete types:

```rust
pub trait PluginScanner {
    type Plugin: PluginInstance;
    fn scan(&self) -> Result<Vec<PluginInfo>>;
    fn load(&self, info: &PluginInfo) -> Result<Self::Plugin>;
}

pub trait PluginInstance: Send {
    fn initialize(&mut self, sample_rate: f64, max_block_size: usize) -> Result<()>;
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()>;
    // ... etc
}
```

This allows:
- Format-agnostic user code
- Easy addition of new backends (VST3, CLAP)
- Testing with mock implementations

### 2. Platform-Specific Defaults

```rust
#[cfg(target_os = "macos")]
pub use au::{AudioUnitScanner as Scanner, AudioUnitPlugin as Plugin};

#[cfg(target_os = "windows")]
pub use vst3::{Vst3Scanner as Scanner, Vst3Plugin as Plugin};
```

Users can just use `Scanner` and `Plugin` and get the right implementation for their platform.

### 3. Safe Rust API

- No `unsafe` in user code
- All `unsafe` is contained in `ffi.rs`
- Proper error handling with `Result<T, Error>`
- RAII for resource management (Drop impls)

### 4. Zero-Cost Abstractions

- Traits compile to direct function calls
- No runtime overhead for abstraction
- Inline hints where appropriate

## FFI Strategy

### Opaque Pointers

C++ types are opaque pointers in Rust:

```rust
#[repr(C)]
pub struct RackAUScanner {
    _private: [u8; 0],
}

#[repr(C)]
pub struct RackAUPlugin {
    _private: [u8; 0],
}
```

### Error Handling

C++ returns int error codes, Rust converts to `Result`:

```rust
fn check_error(code: i32) -> Result<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(Error::from_os_status(code))
    }
}
```

### String Handling

C++ allocates strings, Rust takes ownership:

```rust
extern "C" {
    fn rack_au_get_name(plugin: *mut RackAUPlugin, buf: *mut c_char, len: usize) -> i32;
}

// Rust wrapper:
pub fn get_name(&self) -> Result<String> {
    let mut buf = vec![0u8; 256];
    unsafe {
        check_error(rack_au_get_name(self.ptr, buf.as_mut_ptr() as *mut c_char, 256))?;
    }
    Ok(CStr::from_bytes_until_nul(&buf)?.to_string_lossy().into_owned())
}
```

## Build Process

`build.rs` uses the `cmake` crate to build rack-sys:

```rust
fn main() {
    let dst = cmake::build("../rack-sys");
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=rack_sys");
}
```

This:
1. Runs CMake on rack-sys
2. Builds the C++ library
3. Links it statically into the Rust library

## Testing

```bash
# Run tests
cargo test

# Run with output
cargo test -- --nocapture

# Test specific module
cargo test au::scanner
```

## Examples

Examples serve as:
- Documentation
- Integration tests
- User reference

Run them with:

```bash
cargo run --example list_plugins
cargo run --example simple_host --features cpal
```

## Feature Flags

- `default` - AudioUnit on macOS (or VST3 on other platforms)
- `cpal` - Integration helpers for cpal audio I/O
- `vst3` - VST3 support (future)
- `clap` - CLAP support (future)

## Error Handling

All public functions return `Result<T, Error>`:

```rust
pub enum Error {
    AudioUnit(i32),
    PluginNotFound(String),
    InvalidParameter(usize),
    NotInitialized,
    // ... etc
}
```

Use `thiserror` for automatic `Display` and `Error` trait impls.

## Thread Safety

- `PluginInstance` is `Send` - can be moved between threads
- Audio processing happens on audio thread
- Parameter changes may need atomic operations (future)

## Performance Considerations

- Audio processing is in the hot path
- No allocations in `process()`
- Use `#[inline]` for small wrapper functions
- Profile before optimizing

## Next Steps

1. Implement FFI bindings in `ffi.rs`
2. Wire up scanner in `scanner.rs`
3. Test with real AudioUnits
4. Implement plugin loading
5. Implement audio processing
6. Add parameter handling

## Common Tasks

### Adding a new FFI function

1. Declare in rack-sys/include/rack_au.h
2. Implement in rack-sys/src/rack_au.cpp
3. Add binding in rack/src/au/ffi.rs
4. Wrap safely in scanner.rs or instance.rs

### Adding a new example

1. Create file in examples/
2. Add `[[example]]` entry in Cargo.toml
3. Document in README.md

### Debugging FFI issues

1. Check C++ side compiles: `cd rack-sys/build && make`
2. Check Rust side compiles: `cargo build`
3. Use `RUST_BACKTRACE=1` for stack traces
4. Use `lldb` to debug C++ code
5. Add `println!` debugging in Rust

## Resources

- [The Rustonomicon (FFI)](https://doc.rust-lang.org/nomicon/ffi.html)
- [cmake crate docs](https://docs.rs/cmake/)
- [thiserror crate docs](https://docs.rs/thiserror/)
