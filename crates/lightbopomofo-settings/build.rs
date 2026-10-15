//! Windows: embed the application manifest (common controls 6, per-monitor
//! DPI awareness) and, when building on Windows, the icon resource.
fn main() {
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=app.rc");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/app.manifest");
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{manifest}");
    // The resource compiler only exists on Windows; type-checking elsewhere
    // (cargo check --target …-windows-msvc) needs no icon.
    if cfg!(windows) {
        embed_resource::compile("app.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
