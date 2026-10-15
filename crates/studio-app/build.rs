fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-light".into());
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint build failed");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=assets/pixel-studio.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/pixel-studio.ico")
            .set("ProductName", "Pixel Studio")
            .set("FileDescription", "Pixel Studio - image editor")
            .set("LegalCopyright", "MIT License");
        res.compile().expect("failed to embed Windows resources");
    }
}
