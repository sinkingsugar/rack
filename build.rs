use cmake;

fn main() {
    // Only build C++ wrapper on macOS
    #[cfg(target_os = "macos")]
    {
        // Build rack-sys with CMake
        let dst = cmake::build("rack-sys");

        // Tell cargo where to find the library
        println!("cargo:rustc-link-search=native={}/lib", dst.display());
        println!("cargo:rustc-link-lib=static=rack_sys");

        // Link C++ standard library
        println!("cargo:rustc-link-lib=c++");

        // Link macOS frameworks
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
        println!("cargo:rustc-link-lib=framework=CoreAudio");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");

        // Rerun if C++ files change
        println!("cargo:rerun-if-changed=rack-sys/src");
        println!("cargo:rerun-if-changed=rack-sys/include");
        println!("cargo:rerun-if-changed=rack-sys/CMakeLists.txt");
    }
}
