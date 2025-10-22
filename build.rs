fn main() {
    // TODO: Re-enable C++ build when ready for development
    // For now, this is disabled to allow publishing to crates.io

    // Link macOS frameworks (needed for future AudioUnit implementation)
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
        println!("cargo:rustc-link-lib=framework=CoreAudio");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    }
}
