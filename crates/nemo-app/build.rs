//! Windows build script embedding application logo and version metadata.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/logo/nemo-logo.ico");
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../assets/logo/nemo-logo.ico");
        res.set("ProductName", "NemoMusic");
        res.set("FileDescription", "NemoMusic");
        if let Err(e) = res.compile() {
            eprintln!("Failed to compile Windows resource: {e}");
        }
    }
}
