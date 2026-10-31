fn main() {
    // Tauri only puts its Windows manifest into the app itself. The file picker needs
    // Common Controls 6 from that manifest, so without it the test executable fails to
    // start (STATUS_ENTRYPOINT_NOT_FOUND). Embed the same manifest into every binary,
    // tests included, instead.
    // The app icon shown for Notcher.exe in Explorer, the taskbar and the tray.
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest()
        .window_icon_path("icons/icon.ico");
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run the Tauri build script");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed=windows-app-manifest.xml");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
}
