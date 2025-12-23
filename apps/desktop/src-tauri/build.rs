fn main() {
    // Add rpath for Swift runtime libraries (needed by screencapturekit crate)
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }

    tauri_build::build();
}
