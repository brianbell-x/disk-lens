fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app.ico");
        res.set("ProductName", "Disk Lens");
        res.set("FileDescription", "Fast Folder Size Explorer");
        if let Err(err) = res.compile() {
            eprintln!("Warning: failed to compile Windows icon resource: {err}");
        }
    }
}
