use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Ensure VST3 SDK is available
    ensure_vst3_sdk();

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    // Check if ASAN should be enabled
    let enable_asan = env::var("CARGO_FEATURE_ASAN").is_ok() || env::var("ENABLE_ASAN").is_ok();

    // Configure CMake build
    let mut config = cmake::Config::new("rack-sys");
    config
        .define("CMAKE_BUILD_TYPE", "Release")
        .define("BUILD_TESTS", "OFF"); // Don't build C++ tests in Rust build

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
fn ensure_vst3_sdk() {
    let vst3_sdk_path = PathBuf::from("rack-sys/external/vst3sdk");

    // Check if SDK exists and has content
    let sdk_exists = vst3_sdk_path.exists()
        && vst3_sdk_path.join("CMakeLists.txt").exists()
        && vst3_sdk_path.join("pluginterfaces").exists();

    if sdk_exists {
        eprintln!("VST3 SDK found at {}", vst3_sdk_path.display());
        return;
    }

    eprintln!("VST3 SDK not found, attempting to clone...");

    // Try to initialize git submodule first (for developers who cloned with submodules)
    let submodule_init = Command::new("git")
        .args(&["submodule", "update", "--init", "--recursive", "rack-sys/external/vst3sdk"])
        .current_dir(env::current_dir().unwrap())
        .output();

    if let Ok(output) = submodule_init {
        if output.status.success() {
            eprintln!("VST3 SDK initialized via git submodule");
            return;
        }
    }

    // Fallback: Clone directly (for cargo users who don't have git submodule setup)
    eprintln!("Cloning VST3 SDK directly...");

    // Create external directory if it doesn't exist
    let external_dir = vst3_sdk_path.parent().unwrap();
    std::fs::create_dir_all(external_dir).expect("Failed to create external directory");

    let clone_result = Command::new("git")
        .args(&[
            "clone",
            "--recursive",
            "--depth=1", // Shallow clone to save time
            "https://github.com/steinbergmedia/vst3sdk.git",
            vst3_sdk_path.to_str().unwrap(),
        ])
        .current_dir(env::current_dir().unwrap())
        .status();

    match clone_result {
        Ok(status) if status.success() => {
            eprintln!("VST3 SDK cloned successfully");
        }
        Ok(status) => {
            eprintln!(
                "Warning: Failed to clone VST3 SDK (exit code: {:?})",
                status.code()
            );
            eprintln!("VST3 support will be disabled.");
            eprintln!("To enable VST3, clone the SDK manually:");
            eprintln!("  cd rack-sys && git clone --recursive https://github.com/steinbergmedia/vst3sdk.git external/vst3sdk");
        }
        Err(e) => {
            eprintln!("Warning: Failed to execute git clone: {}", e);
            eprintln!("VST3 support will be disabled.");
            eprintln!("To enable VST3, ensure git is installed and clone the SDK manually:");
            eprintln!("  cd rack-sys && git clone --recursive https://github.com/steinbergmedia/vst3sdk.git external/vst3sdk");
        }
    }
}
