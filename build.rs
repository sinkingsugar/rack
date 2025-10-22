fn main() {
    // Only build C++ wrapper on macOS
    #[cfg(target_os = "macos")]
    {
        use std::env;

        // Build rack-sys with CMake using explicit configuration
        let target = env::var("TARGET").unwrap();
        let dst = cmake::Config::new("rack-sys")
            .define("CMAKE_BUILD_TYPE", "Release")
            .define("BUILD_TESTS", "OFF") // Don't build C++ tests in Rust build
            .build();

        // Library name comes from CMake configuration (librack_sys.a)
        let lib_name = "rack_sys";

        // Tell cargo where to find the library
        println!("cargo:rustc-link-search=native={}/lib", dst.display());
        println!("cargo:rustc-link-lib=static={}", lib_name);

        // Link C++ standard library (required for C++ code)
        // Use libc++ on macOS (default for clang)
        println!("cargo:rustc-link-lib=c++");

        // Link required macOS frameworks for AudioUnit API
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
        println!("cargo:rustc-link-lib=framework=CoreAudio");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");

        // Rerun build script if C++ sources, headers, or build config changes
        println!("cargo:rerun-if-changed=rack-sys/src");
        println!("cargo:rerun-if-changed=rack-sys/include");
        println!("cargo:rerun-if-changed=rack-sys/CMakeLists.txt");

        // Print target for debugging
        eprintln!("Building rack-sys for target: {}", target);
    }
}
