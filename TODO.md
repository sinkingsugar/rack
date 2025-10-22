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

**Current Status**: C++ scanner is production-ready and tested with 128+ AudioUnit plugins.

---

## 🚧 Next Phase: Rust FFI Integration

### High Priority - Scanner FFI Bindings

**Goal**: Wire the C++ scanner to Rust through FFI

**Files to implement**:
- `rack/src/au/ffi.rs` - FFI bindings (bindgen or manual)
- `rack/src/au/scanner.rs` - Safe Rust wrapper
- `rack/src/au/mod.rs` - Module organization

**Tasks**:

#### 1. Create FFI Bindings (`src/au/ffi.rs`)
```rust
// Option A: Use bindgen in build.rs
// Option B: Manual bindings (recommended for small API)

use std::os::raw::{c_char, c_int, c_void};

#[repr(C)]
pub struct RackAUScanner {
    _private: [u8; 0],
}

#[repr(C)]
pub struct RackAUPluginInfo {
    pub name: [c_char; 256],
    pub manufacturer: [c_char; 256],
    pub path: [c_char; 1024],
    pub unique_id: [c_char; 64],
    pub version: u32,
    pub plugin_type: RackAUPluginType,
}

// ... extern "C" declarations
```

#### 2. Implement Safe Rust Wrapper (`src/au/scanner.rs`)
```rust
pub struct AudioUnitScanner {
    inner: NonNull<ffi::RackAUScanner>,
}

impl AudioUnitScanner {
    pub fn new() -> Result<Self, Error>;
    pub fn scan(&mut self) -> Result<Vec<PluginInfo>, Error>;
}

// Convert from C struct to Rust PluginInfo
impl From<ffi::RackAUPluginInfo> for PluginInfo { ... }
```

**Key considerations**:
- Proper Drop implementation for scanner cleanup
- Safe conversion of C strings to Rust String/CString
- Error handling for negative return codes
- Tests for FFI boundary

#### 3. Update build.rs
- Ensure CMake builds C++ library before Rust compilation
- Link against librack_sys.a
- Add proper framework linking for macOS

#### 4. Update examples/list_plugins.rs
- Replace placeholder implementation
- Use new AudioUnitScanner
- Pretty-print results

#### 5. Add Integration Tests
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_scanner_creation() { ... }

    #[test]
    fn test_scan_returns_plugins() { ... }

    #[test]
    fn test_plugin_info_conversion() { ... }
}
```

**Success Criteria**:
- ✅ `cargo build` succeeds
- ✅ `cargo test` passes
- ✅ `cargo run --example list_plugins` shows AudioUnit plugins
- ✅ No memory leaks (valgrind/instruments)

---

## 📋 Future Phases

### Phase 3: Plugin Loading & Initialization
**Files**: `rack-sys/src/au_instance.cpp`, `rack/src/au/plugin.rs`

Tasks:
- [ ] Implement `rack_au_plugin_new()` - create instance from unique_id
- [ ] Implement `rack_au_plugin_initialize()` - set sample rate, buffer size
- [ ] Implement `rack_au_plugin_is_initialized()` - check state
- [ ] Wire up Rust `AudioUnitPlugin` struct
- [ ] Add tests for plugin lifecycle

### Phase 4: Audio Processing
**Files**: `rack-sys/src/au_instance.cpp`, `rack/src/au/plugin.rs`

Tasks:
- [ ] Implement `rack_au_plugin_process()` - render audio
- [ ] Handle interleaved stereo format
- [ ] Add example demonstrating audio processing
- [ ] Performance testing & optimization

### Phase 5: Parameter Control
**Files**: `rack-sys/src/au_instance.cpp`, `rack/src/au/plugin.rs`

Tasks:
- [ ] Implement parameter enumeration
- [ ] Implement get/set parameter
- [ ] Parameter automation support
- [ ] GUI parameter controls (future)

### Phase 6: Additional Formats
**Files**: New `rack-sys/src/vst3_*`, `rack/src/vst3/`

Tasks:
- [ ] VST3 scanner
- [ ] VST3 plugin loading
- [ ] CLAP support (optional)
- [ ] Common trait abstraction across formats

---

## 🎯 Immediate Next Steps

**Start Here** (after PR merge):

1. Create `rack/src/au/ffi.rs` with manual bindings
2. Implement `rack/src/au/scanner.rs` safe wrapper
3. Update `rack/examples/list_plugins.rs` to use real scanner
4. Run tests and verify functionality

**Commands**:
```bash
# Should work after implementation:
cargo build --verbose
cargo test --verbose
cargo run --example list_plugins

# Expected output: List of ~128 AudioUnit plugins
```

---

## 📝 Notes

### Design Decisions
- **Why manual FFI bindings?** Small API surface, better control, easier to review
- **Why two-pass scanning?** Allows proper memory allocation, no guessing array sizes
- **Why C++ wrapper?** AudioUnit API is native C++, easier to debug, leverage existing code

### Resources
- C API: `rack-sys/include/rack_au.h`
- C++ impl: `rack-sys/src/au_scanner.cpp`
- C++ tests: `rack-sys/test/test_scanner.cpp`
- Rust placeholders: `rack/src/au/scanner.rs`

### Contact
See `CLAUDE.md` files in root and subdirectories for detailed component documentation.
