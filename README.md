# Rack

**A modern Rust library for hosting audio plugins**

> **Status:** AudioUnit support is **production-ready** on macOS (Phases 1-8 complete).
> The API is stabilizing. Other plugin formats (VST3, CLAP) are planned.

[![Crates.io](https://img.shields.io/crates/v/rack.svg)](https://crates.io/crates/rack)
[![Documentation](https://docs.rs/rack/badge.svg)](https://docs.rs/rack)
[![License](https://img.shields.io/crates/l/rack.svg)](https://github.com/sinkingsugar/rack#license)

Rack is a cross-platform library for discovering, loading, and processing audio through VST3, AudioUnit, CLAP, and other plugin formats in Rust applications.

## Features

- ✅ **AudioUnit support** (macOS) - complete with scanning, loading, processing, parameters, MIDI, presets, and GUI
- 🎵 **SIMD-optimized audio processing** - ARM NEON and x86_64 SSE2 for 4x performance
- 🎹 **Zero-allocation MIDI** - SmallVec-based MIDI for real-time performance
- 🎛️ **GUI support** - AUv3, AUv2, and generic fallback UI
- 🎚️ **Clean, safe API** - minimal unsafe code, comprehensive error handling
- 🔌 **VST3 support** - planned
- 🎼 **CLAP support** - planned
- 🔄 **cpal integration** - optional audio I/O helpers
- 🚀 **Zero-cost abstractions** - trait-based design

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
rack = "0.2"
```

### List available plugins

```rust
use rack::prelude::*;

fn main() -> Result<()> {
    let scanner = Scanner::new()?;
    let plugins = scanner.scan()?;

    for plugin in plugins {
        println!("{}", plugin);
    }

    Ok(())
}
```

### Load and process audio

```rust
use rack::prelude::*;

fn main() -> Result<()> {
    let scanner = Scanner::new()?;
    let plugins = scanner.scan()?;

    // Load first plugin
    let mut plugin = scanner.load(&plugins[0])?;
    plugin.initialize(48000.0, 512)?;

    // Process audio with aligned buffers
    let input = AudioBuffer::new(1024);  // 512 frames stereo
    let mut output = AudioBuffer::new(1024);
    plugin.process(&input, &mut output)?;

    Ok(())
}
```

### MIDI Synthesis

See [examples/simple_synth.rs](examples/simple_synth.rs) for a complete MIDI synthesis example.

## Platform Support

| Platform | AudioUnit | VST3 | CLAP | LV2 |
|----------|-----------|------|------|-----|
| macOS    | ✅        | 🚧   | 🚧   | ❌  |
| Windows  | ❌        | 🚧   | 🚧   | ❌  |
| Linux    | ❌        | 🚧   | 🚧   | 🚧  |

- ✅ Supported
- 🚧 Planned
- ❌ Not applicable

## Examples

Run the examples:

```bash
# List all available plugins
cargo run --example list_plugins

# Control parameters
cargo run --example control_parameters

# MIDI synthesis
cargo run --example simple_synth

# Browse and load presets
cargo run --example preset_browser

# Plugin GUI (shows native plugin UI)
cargo run --example plugin_gui
```

### Display Plugin GUI

```rust
use rack::prelude::*;

fn main() -> Result<()> {
    let scanner = Scanner::new()?;
    let plugins = scanner.scan()?;
    let mut plugin = scanner.load(&plugins[0])?;
    plugin.initialize(48000.0, 512)?;

    // Create GUI asynchronously
    plugin.create_gui(|result| {
        match result {
            Ok(gui) => {
                gui.show_window(Some("My Plugin"))?;
                // GUI is now visible!
            }
            Err(e) => eprintln!("GUI creation failed: {}", e),
        }
        Ok(())
    });

    // Keep program running...
    Ok(())
}
```

See [examples/plugin_gui.rs](examples/plugin_gui.rs) for a complete GUI example.

## Architecture

Rack uses a trait-based design for maximum flexibility:

```rust
pub trait PluginScanner {
    type Plugin: PluginInstance;
    fn scan(&self) -> Result<Vec<PluginInfo>>;
    fn load(&self, info: &PluginInfo) -> Result<Self::Plugin>;
}

pub trait PluginInstance: Send {
    fn initialize(&mut self, sample_rate: f64, max_block_size: usize) -> Result<()>;
    fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()>;
    fn get_parameter(&self, index: usize) -> Result<f32>;
    fn set_parameter(&mut self, index: usize, value: f32) -> Result<()>;
    // ... more methods
}
```

This allows different plugin formats to implement the same interface, making your code portable across formats.

## Roadmap

### AudioUnit (macOS) - ✅ COMPLETE
- [x] Plugin scanning and enumeration
- [x] Plugin loading and instantiation
- [x] Audio processing with SIMD optimization (ARM NEON + x86_64 SSE2)
- [x] Parameter control with caching
- [x] MIDI support (zero-allocation, all MIDI 1.0 messages)
- [x] Preset management (factory presets + state serialization)
- [x] GUI hosting (AUv3/AUv2/generic fallback)

### Future Formats
- [ ] VST3 support (cross-platform)
- [ ] CLAP support (cross-platform)
- [ ] LV2 support (Linux)

### Advanced Features
- [ ] Multi-threading support
- [ ] Plugin latency compensation
- [ ] Offline processing
- [ ] Crash isolation
- [ ] Plugin sandboxing

## Contributing

Contributions are welcome!

**Completed**: AudioUnit hosting is production-ready (Phases 1-8 complete)

**Areas where help is needed**:
- VST3 backend (scanner, loader, processor, GUI)
- CLAP backend (scanner, loader, processor, GUI)
- Linux LV2 support
- Advanced features (multi-threading, latency compensation, crash isolation)
- Documentation improvements
- Additional examples
- Cross-platform testing

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

- Inspired by [VCV Rack](https://vcvrack.com/) and the modular synthesis community
- Built on top of [coreaudio-rs](https://github.com/RustAudio/coreaudio-rs)
- Thanks to the Rust audio community at [rust.audio](https://rust.audio)

## Why "Rack"?

The name is inspired by modular synthesizer racks and VCV Rack - the idea of a framework where you can plug in different modules (plugins) and wire them together. Plus, it was available on crates.io! 🎉
