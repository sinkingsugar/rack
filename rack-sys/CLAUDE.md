# rack-sys - C++ Wrapper for Native Plugin APIs

## Purpose

This is the C++ layer that wraps native audio plugin APIs (AudioUnit, VST3, etc.) and exposes a clean C API for Rust FFI.

## Current Focus: AudioUnit on macOS

We're implementing AudioUnit hosting support first. The AudioUnit API is part of Apple's CoreAudio framework.

## Architecture

```
rack-sys/
├── CMakeLists.txt         # Build configuration
├── include/
│   └── rack_au.h          # C API header (for Rust FFI)
└── src/
    ├── rack_au.cpp        # C API implementation
    ├── au_scanner.cpp     # AudioComponent enumeration
    └── au_instance.cpp    # AudioUnit lifecycle & processing
```

## C API Design

The C API is designed to be:
- **Simple** - minimal surface area
- **Safe** - opaque pointers, clear ownership
- **Efficient** - zero-copy where possible
- **Rust-friendly** - matches Rust's Result<T, E> pattern

### Naming Convention

- `rack_au_*` - AudioUnit-specific functions
- `rack_vst3_*` - VST3-specific (future)
- `rack_clap_*` - CLAP-specific (future)

### Memory Management

- Rust owns the scanner/plugin structs
- C++ allocates with `new`, Rust frees with `_free()` functions
- String data is copied to Rust-owned buffers

## AudioUnit Implementation Notes

### Component Enumeration

Use `AudioComponentFindNext()` to iterate through registered AudioUnits:

```cpp
AudioComponent comp = nullptr;
while ((comp = AudioComponentFindNext(comp, &desc)) != nullptr) {
    // Get component info
    // Add to results
}
```

### Component Description

```cpp
AudioComponentDescription desc = {0};
desc.componentType = kAudioUnitType_Effect;  // or MusicDevice, etc.
desc.componentManufacturer = kAudioUnitManufacturer_Apple;
```

### Getting Component Info

```cpp
CFStringRef name = nullptr;
AudioComponentCopyName(comp, &name);
// Convert to C string for Rust
```

### Creating Instance

```cpp
AudioComponentInstance instance;
AudioComponentInstanceNew(comp, &instance);
AudioUnitInitialize(instance);
```

### Setting Sample Rate

```cpp
AudioStreamBasicDescription format = {0};
format.mSampleRate = sample_rate;
format.mFormatID = kAudioFormatLinearPCM;
// ... set other fields
AudioUnitSetProperty(instance, kAudioUnitProperty_StreamFormat, ...);
```

### Render Callback

```cpp
AURenderCallbackStruct callback;
callback.inputProc = renderCallback;
callback.inputProcRefCon = userData;
AudioUnitSetProperty(instance, kAudioUnitProperty_SetRenderCallback, ...);
```

## Building

CMake handles the build:

```bash
cd rack-sys
mkdir build
cd build
cmake ..
make
```

The cmake crate in rack/build.rs will do this automatically.

## Frameworks Required

- **AudioToolbox.framework** - AudioComponent APIs
- **CoreAudio.framework** - AudioUnit types
- **CoreFoundation.framework** - CFString, etc.

CMake links these automatically on macOS.

## Testing Strategy

1. **Standalone C++ test** - Build a simple CLI tool that uses the C++ code directly
2. **C API test** - Test the C API from C++ before Rust integration
3. **Rust integration test** - Test through Rust FFI

## Common Pitfalls

- **CFString management** - Remember to CFRelease
- **OSStatus errors** - Check return codes, map to meaningful errors
- **Thread safety** - AudioUnit render callback is on audio thread
- **Buffer formats** - Interleaved vs non-interleaved audio

## Debugging Tips

- Use `lldb` to debug C++ code
- Print OSStatus codes with `AudioUnitGetLastError()`
- Check Console.app for AudioUnit errors
- Use `auval` command-line tool to validate plugins

## Next Steps

1. Implement `rack_au_scanner_scan()` - enumerate components
2. Implement `rack_au_plugin_new()` - create instance
3. Implement `rack_au_plugin_initialize()` - set up for processing
4. Implement `rack_au_plugin_process()` - render audio
5. Add parameter get/set functions

## Resources

- [Audio Unit Programming Guide](https://developer.apple.com/library/archive/documentation/MusicAudio/Conceptual/AudioUnitProgrammingGuide/)
- [Core Audio Overview](https://developer.apple.com/library/archive/documentation/MusicAudio/Conceptual/CoreAudioOverview/)
- [AudioToolbox Framework Reference](https://developer.apple.com/documentation/audiotoolbox)
