# External Dependencies

This directory contains external dependencies required to build rack-sys.

## VST3 SDK

The [VST3 SDK](https://github.com/steinbergmedia/vst3sdk) is required for VST3 plugin hosting support.

### For Developers

If you cloned this repository with `--recursive`, the submodule is already initialized:
```bash
git clone --recursive https://github.com/sinkingsugar/rack.git
```

If you cloned without `--recursive`, initialize the submodule:
```bash
git submodule update --init --recursive
```

### For Cargo Users

If you're using rack as a dependency in your `Cargo.toml`, the build script (`build.rs`) will **automatically clone** the VST3 SDK during the first build. No manual setup needed!

```toml
[dependencies]
rack = { git = "https://github.com/sinkingsugar/rack" }
```

The first build will take a bit longer as it downloads the SDK (~50 MB).

### Manual Setup (if automatic cloning fails)

If the automatic cloning fails (e.g., no git available), you can clone manually:

```bash
cd rack-sys/external
git clone --recursive https://github.com/steinbergmedia/vst3sdk.git
```

## License

- **VST3 SDK**: MIT License (see vst3sdk/LICENSE.txt after cloning)
- The VST3 SDK is MIT licensed and can be freely used in commercial projects
