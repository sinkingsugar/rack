use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    // VST3 is only supported on desktop platforms (macOS, Linux, Windows)
    // Skip VST3 SDK setup on iOS/tvOS/watchOS/visionOS
    // Also skip during cargo publish to avoid modifying source directory
    let is_desktop = matches!(target_os.as_str(), "macos" | "linux" | "windows");

    // Detect if we're in a cargo publish verification build
    let current_dir = env::current_dir().unwrap();
    let in_publish_verify = current_dir
        .to_str()
        .map(|s| s.contains("/target/package/"))
        .unwrap_or(false);

    // Determine VST3 SDK path and ensure it's available
    let vst3_sdk_path = if is_desktop && !in_publish_verify {
        ensure_vst3_sdk()
    } else {
        if in_publish_verify {
            eprintln!("Skipping VST3 SDK auto-clone for cargo publish (VST3 support disabled)");
        } else {
            eprintln!("Skipping VST3 SDK setup for {} (VST3 only supported on desktop platforms)", target_os);
        }
        None
    };

    // Check if ASAN should be enabled
    let enable_asan = env::var("CARGO_FEATURE_ASAN").is_ok() || env::var("ENABLE_ASAN").is_ok();

    // Configure CMake build
    let mut config = cmake::Config::new("rack-sys");
    config
        .define("CMAKE_BUILD_TYPE", "Release")
        .define("BUILD_TESTS", "OFF"); // Don't build C++ tests in Rust build

    // Pass VST3 SDK path to CMake if available
    if let Some(sdk_path) = vst3_sdk_path {
        config.define("VST3_SDK_PATH", sdk_path.to_str().unwrap());
        eprintln!("Configuring CMake with VST3 SDK at: {}", sdk_path.display());
    }

    if enable_asan {
        config.define("ENABLE_ASAN", "ON");
        eprintln!("Building with AddressSanitizer enabled");
    }

    let dst = config.build();

    // Library name comes from CMake configuration (librack_sys.a)
    let lib_name = "rack_sys";

    // Tell cargo where to find the library
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static={}", lib_name);

    // Platform-specific linking
    match target_os.as_str() {
        "macos" | "ios" | "tvos" | "watchos" => {
            link_apple_frameworks(&target_os);
            // Link C++ standard library (libc++ on Apple platforms)
            println!("cargo:rustc-link-lib=c++");
        }
        "linux" => {
            // Link C++ standard library (libstdc++ on Linux)
            println!("cargo:rustc-link-lib=stdc++");
            // Link dynamic loader (needed for VST3 module loading)
            println!("cargo:rustc-link-lib=dl");
        }
        "windows" => {
            // Windows uses static linking by default, no extra libs needed for C++
        }
        _ => {
            eprintln!("Warning: Unsupported target OS: {}", target_os);
        }
    }

    // Link ASAN runtime if enabled
    if enable_asan {
        println!("cargo:rustc-link-arg=-fsanitize=address");
        println!("cargo:rustc-link-arg=-fno-optimize-sibling-calls");
        println!("cargo:rustc-link-arg=-fsanitize-address-use-after-scope");
        println!("cargo:rustc-link-arg=-fno-omit-frame-pointer");
    }

    // Rerun build script if C++ sources, headers, or build config changes
    println!("cargo:rerun-if-changed=rack-sys/src");
    println!("cargo:rerun-if-changed=rack-sys/include");
    println!("cargo:rerun-if-changed=rack-sys/CMakeLists.txt");
    println!("cargo:rerun-if-changed=rack-sys/external/vst3sdk");

    // Print target for debugging
    eprintln!(
        "Building rack-sys for target: {} ({})",
        env::var("TARGET").unwrap(),
        target_os
    );
}

fn link_apple_frameworks(target_os: &str) {
    // AudioUnit frameworks (macOS and iOS)
    println!("cargo:rustc-link-lib=framework=AudioToolbox");
    println!("cargo:rustc-link-lib=framework=CoreAudio");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=CoreAudioKit");

    // Link platform-specific UI frameworks
    let is_ios_family = target_os == "ios" || target_os.contains("vision");
    if is_ios_family {
        println!("cargo:rustc-link-lib=framework=UIKit");
        eprintln!("Building rack-sys for iOS/visionOS (GUI provided by app extensions)");
    } else {
        println!("cargo:rustc-link-lib=framework=AppKit");
        eprintln!("Building rack-sys for macOS (GUI enabled)");
    }
}

/// Ensure VST3 SDK is available, cloning it if necessary
/// Returns the path to the VST3 SDK, or None if unavailable
fn ensure_vst3_sdk() -> Option<PathBuf> {
    let vst3_sdk_path = PathBuf::from("rack-sys/external/vst3sdk");

    // Check if SDK exists and has content (in source tree)
    let sdk_exists = vst3_sdk_path.exists()
        && vst3_sdk_path.join("CMakeLists.txt").exists()
        && vst3_sdk_path.join("pluginterfaces").exists();

    if sdk_exists {
        eprintln!("VST3 SDK found at {}", vst3_sdk_path.display());
        return Some(vst3_sdk_path);
    }

    eprintln!("VST3 SDK not found in source tree, attempting to clone...");

    // Determine where to clone the SDK
    // When building from crates.io, the source dir is read-only, so clone to OUT_DIR
    let current_dir = env::current_dir().unwrap();
    let in_cargo_registry = current_dir
        .to_str()
        .map(|s| s.contains("/.cargo/registry/"))
        .unwrap_or(false);

    let clone_target = if in_cargo_registry {
        // Building from crates.io - clone to OUT_DIR (writable)
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let sdk_path = out_dir.join("vst3sdk");
        eprintln!("Building from crates.io - will clone VST3 SDK to OUT_DIR: {}", sdk_path.display());
        sdk_path
    } else {
        // Building from git checkout - clone to source tree
        vst3_sdk_path.clone()
    };

    // Check if already cloned to target location
    if clone_target.exists() && clone_target.join("CMakeLists.txt").exists() {
        eprintln!("VST3 SDK already exists at {}", clone_target.display());
        return Some(clone_target);
    }

    // Try to initialize git submodule first (for developers who cloned with submodules)
    if !in_cargo_registry {
        let submodule_init = Command::new("git")
            .args(&["submodule", "update", "--init", "--recursive", "rack-sys/external/vst3sdk"])
            .current_dir(&current_dir)
            .output();

        if let Ok(output) = submodule_init {
            if output.status.success() && vst3_sdk_path.join("CMakeLists.txt").exists() {
                eprintln!("VST3 SDK initialized via git submodule");
                return Some(vst3_sdk_path);
            }
        }
    }

    // Fallback: Clone directly
    eprintln!("Cloning VST3 SDK to {}...", clone_target.display());

    // Create parent directory if it doesn't exist
    if let Some(parent) = clone_target.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let clone_result = Command::new("git")
        .args(&[
            "clone",
            "--recursive",
            "--depth=1", // Shallow clone to save time
            "https://github.com/steinbergmedia/vst3sdk.git",
            clone_target.to_str().unwrap(),
        ])
        .status();

    match clone_result {
        Ok(status) if status.success() => {
            eprintln!("VST3 SDK cloned successfully to {}", clone_target.display());
            Some(clone_target)
        }
        Ok(status) => {
            eprintln!(
                "Warning: Failed to clone VST3 SDK (exit code: {:?})",
                status.code()
            );
            eprintln!("VST3 support will be disabled.");
            None
        }
        Err(e) => {
            eprintln!("Warning: Failed to execute git clone: {}", e);
            eprintln!("VST3 support will be disabled.");
            eprintln!("Ensure git is installed to enable VST3 support.");
            None
        }
    }
}
