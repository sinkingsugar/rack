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

---

## 🚧 Next Phase: Plugin Loading & Initialization

### High Priority - Plugin Instance Management

**Goal**: Load individual AudioUnit plugins and prepare them for audio processing

**Files to implement**:
- `rack-sys/src/au_instance.cpp` - C++ AudioUnit instance wrapper
- `rack/src/au/instance.rs` - Rust plugin instance (update existing stub)
- Update `rack/src/au/ffi.rs` - Add instance FFI bindings

**Tasks**:

#### 1. Implement C++ Instance Management (`rack-sys/src/au_instance.cpp`)

Create C++ wrapper for AudioUnit plugin instances:

```cpp
struct RackAUPlugin {
    AudioComponentInstance instance;
    AudioStreamBasicDescription format;
    bool initialized;
    // ... audio buffers, etc.
};

extern "C" {
    RackAUPlugin* rack_au_plugin_new(const char* unique_id);
    void rack_au_plugin_free(RackAUPlugin* plugin);
    int rack_au_plugin_initialize(RackAUPlugin* plugin, double sample_rate, uint32_t max_block_size);
    int rack_au_plugin_is_initialized(RackAUPlugin* plugin);
}
```

**Key implementation details**:
- Look up AudioComponent by unique_id (type/subtype/manufacturer)
- Create AudioComponentInstance
- Configure audio format (stereo interleaved f32)
- Allocate audio buffers
- Handle initialization/deinitialization
- Comprehensive error handling

#### 2. Add FFI Bindings (`src/au/ffi.rs`)

The bindings already exist in ffi.rs, just need to be tested:

```rust
extern "C" {
    pub fn rack_au_plugin_new(unique_id: *const c_char) -> *mut RackAUPlugin;
    pub fn rack_au_plugin_free(plugin: *mut RackAUPlugin);
    pub fn rack_au_plugin_initialize(
        plugin: *mut RackAUPlugin,
        sample_rate: f64,
        max_block_size: u32,
    ) -> c_int;
    pub fn rack_au_plugin_is_initialized(plugin: *mut RackAUPlugin) -> c_int;
}
```

#### 3. Implement Safe Rust Wrapper (`src/au/instance.rs`)

Update the existing stub to provide safe API:

```rust
pub struct AudioUnitPlugin {
    inner: NonNull<ffi::RackAUPlugin>,
    info: PluginInfo,
}

impl AudioUnitPlugin {
    pub fn new(info: &PluginInfo) -> Result<Self>;
    pub fn initialize(&mut self, sample_rate: f64, max_block_size: u32) -> Result<()>;
    pub fn is_initialized(&self) -> bool;
}

impl Drop for AudioUnitPlugin {
    fn drop(&mut self) {
        unsafe { ffi::rack_au_plugin_free(self.inner.as_ptr()); }
    }
}

impl PluginInstance for AudioUnitPlugin {
    fn initialize(&mut self, sample_rate: f64, max_block_size: u32) -> Result<()>;
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()>;
    // ... parameter methods (Phase 5)
}
```

**Key considerations**:
- Use `CString` for unique_id conversion
- Proper error handling (null checks, error codes)
- Drop implementation for cleanup
- State tracking (initialized flag)
- Thread safety (Send but not Sync)

#### 4. Add C++ Tests (`rack-sys/test/test_instance.cpp`)

```cpp
void test_plugin_new();
void test_plugin_initialize();
void test_plugin_lifecycle();
void test_invalid_unique_id();
```

#### 5. Add Rust Tests (`src/au/instance.rs`)

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_plugin_creation();

    #[test]
    fn test_plugin_initialization();

    #[test]
    fn test_plugin_not_initialized();

    #[test]
    fn test_plugin_lifecycle();
}
```

#### 6. Create Example (`examples/load_plugin.rs`)

```rust
use rack::prelude::*;

fn main() -> Result<()> {
    // Scan for plugins
    let scanner = Scanner::new()?;
    let plugins = scanner.scan()?;

    // Load first instrument plugin
    let plugin_info = plugins
        .iter()
        .find(|p| p.plugin_type == PluginType::Instrument)
        .expect("No instrument plugins found");

    println!("Loading: {}", plugin_info.name);

    // Create instance
    let mut plugin = scanner.load(plugin_info)?;

    // Initialize for 48kHz, 512 sample buffers
    plugin.initialize(48000.0, 512)?;

    println!("Plugin initialized successfully!");

    Ok(())
}
```

**Success Criteria**:
- [ ] `cargo build` succeeds
- [ ] C++ tests pass (`rack-sys/build/test_rack_sys`)
- [ ] Rust tests pass (`cargo test`)
- [ ] `cargo run --example load_plugin` successfully loads a plugin
- [ ] No memory leaks (Instruments on macOS)
- [ ] Proper cleanup on Drop

---

## 📋 Future Phases

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

**Start Here** (Phase 3 - Plugin Loading):

1. Implement `rack-sys/src/au_instance.cpp` - C++ AudioUnit instance wrapper
   - Parse unique_id format (type-subtype-manufacturer)
   - Look up AudioComponent
   - Create and configure AudioComponentInstance
   - Set up audio format (stereo interleaved f32)

2. Add C++ tests in `rack-sys/test/test_instance.cpp`
   - Test plugin creation
   - Test initialization
   - Test error cases (invalid unique_id, etc.)

3. Update `rack/src/au/instance.rs` - Safe Rust wrapper
   - Implement `AudioUnitPlugin::new()`
   - Implement `AudioUnitPlugin::initialize()`
   - Add proper Drop implementation
   - Add thread safety markers (Send/!Sync)

4. Add Rust tests
   - Test plugin lifecycle
   - Test initialization state
   - Test error handling

5. Create `examples/load_plugin.rs` example

**Commands**:
```bash
# C++ tests
cd rack-sys/build
cmake --build .
./test_rack_sys

# Rust build and test
cd ~/devel/rack
cargo build --verbose
cargo test --verbose

# Try loading a plugin
cargo run --example load_plugin

# Expected: Successfully loads and initializes an AudioUnit plugin
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
