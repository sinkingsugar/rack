# Rack

**A modern Rust library for hosting audio plugins**

> ⚠️ **WARNING: WORK IN PROGRESS** ⚠️
> This library is in **early development** and **NOT YET FUNCTIONAL**.
> The API is unstable and will change. Do not use in production.
> Published to reserve the crate name and test the build process.

[![Crates.io](https://img.shields.io/crates/v/rack.svg)](https://crates.io/crates/rack)
[![Documentation](https://docs.rs/rack/badge.svg)](https://docs.rs/rack)
[![License](https://img.shields.io/crates/l/rack.svg)](https://github.com/fragcolor-xyz/rack#license)

Rack is a cross-platform library for discovering, loading, and processing audio through VST3, AudioUnit, CLAP, and other plugin formats in Rust applications.

## Features

- 🎵 **AudioUnit support** (macOS) - built-in
- 🔌 **VST3 support** - coming soon
- 🎛️ **CLAP support** - coming soon  
- 🎚️ **Clean, safe API** - no unsafe code in your application
- 🔄 **cpal integration** - optional audio I/O helpers
- 🚀 **Zero-cost abstractions** - trait-based design

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
rack = "0.1"
```

### List available plugins

```rust
use rack::prelude::*;

fn main() -> Result<()> {
    let scanner = Scanner::new();
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
    let scanner = Scanner::new();
    let plugins = scanner.scan()?;
    
    // Load first plugin
    let mut plugin = scanner.load(&plugins[0])?;
    plugin.initialize(48000.0, 512)?;
    
    // Process audio
    let input = vec![0.0; 512];
    let mut output = vec![0.0; 512];
    plugin.process(&input, &mut output)?;
    
    Ok(())
}
```

### With cpal for audio I/O

Enable the `cpal` feature:

```toml
[dependencies]
rack = { version = "0.1", features = ["cpal"] }
cpal = "0.15"
```

See [examples/simple_host.rs](examples/simple_host.rs) for a complete example.

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

# Simple host with audio playback (requires cpal feature)
cargo run --example simple_host --features cpal
```

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

- [x] AudioUnit scanning (macOS)
- [ ] AudioUnit loading and instantiation
- [ ] AudioUnit audio processing
- [ ] AudioUnit parameter handling
- [ ] VST3 support (cross-platform)
- [ ] CLAP support (cross-platform)
- [ ] LV2 support (Linux)
- [ ] GUI hosting
- [ ] Preset management
- [ ] MIDI support

## Contributing

Contributions are welcome! This is an early-stage project and there's lots to do.

Areas where help is needed:
- AudioUnit implementation (FFI work)
- VST3 backend
- CLAP backend
- Documentation
- Examples
- Testing

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
