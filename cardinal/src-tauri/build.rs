fn main() {
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=framework=QuickLook");

    tauri_build::build()
}
