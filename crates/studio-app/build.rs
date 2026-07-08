use std::env;

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon/pack/windows/pixel-studio.ico");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("../../assets/icon/pack/windows/pixel-studio.ico");
    resource
        .compile()
        .expect("failed to compile Pixel Studio Windows icon resource");
}
