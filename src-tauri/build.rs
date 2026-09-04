fn main() {
    tauri_build::build();
    // tauri-build embeds the app manifest into bin targets only; test executables
    // linking the Wry/WebView2 stack fail with STATUS_ENTRYPOINT_NOT_FOUND without
    // it (loader binds comctl32 v5, which lacks the v6 entry points).
    let windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows_msvc {
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
