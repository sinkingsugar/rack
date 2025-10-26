fn main() {
    // Build C++ wrapper on Apple platforms (macOS and iOS)
    #[cfg(target_vendor = "apple")]
    {
        use std::env;

        // Build rack-sys with CMake using explicit configuration
        let target = env::var("TARGET").unwrap();

        // Detect iOS/visionOS vs macOS (UIKit vs AppKit)
        let is_ios_family = target.contains("ios") || target.contains("vision");

        // Check if ASAN should be enabled
        let enable_asan = env::var("CARGO_FEATURE_ASAN").is_ok() || env::var("ENABLE_ASAN").is_ok();

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

        // Link C++ standard library (required for C++ code)
        // Use libc++ on Apple platforms (default for clang)
        println!("cargo:rustc-link-lib=c++");

        // Link ASAN runtime if enabled
        if enable_asan {
            println!("cargo:rustc-link-arg=-fsanitize=address");
            println!("cargo:rustc-link-arg=-fno-optimize-sibling-calls");
            println!("cargo:rustc-link-arg=-fsanitize-address-use-after-scope");
            println!("cargo:rustc-link-arg=-fno-omit-frame-pointer");
        }

        // Link required frameworks for AudioUnit API (common to macOS and iOS)
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
        println!("cargo:rustc-link-lib=framework=CoreAudio");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=CoreAudioKit");

        // Link platform-specific UI frameworks
        if is_ios_family {
            println!("cargo:rustc-link-lib=framework=UIKit");
            eprintln!("Building rack-sys for iOS/visionOS (GUI provided by app extensions)");
        } else {
            println!("cargo:rustc-link-lib=framework=AppKit");
            eprintln!("Building rack-sys for macOS (GUI enabled)");
        }

        // Rerun build script if C++ sources, headers, or build config changes
        println!("cargo:rerun-if-changed=rack-sys/src");
        println!("cargo:rerun-if-changed=rack-sys/include");
        println!("cargo:rerun-if-changed=rack-sys/CMakeLists.txt");

        // Print target for debugging
        eprintln!("Building rack-sys for target: {}", target);
    }
}
