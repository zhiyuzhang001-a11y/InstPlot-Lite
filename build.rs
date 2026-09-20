fn main() {
    println!("cargo:rerun-if-changed=logo.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("logo.ico")
            .set("ProductName", "InstPlot Lite")
            .set("FileDescription", "InstPlot Lite")
            .compile()
            .expect("failed to embed the Windows application icon");
    }
}
