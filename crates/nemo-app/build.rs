//! Windows build script embedding application logo.

fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../assets/logo/nemo-logo.ico");
        if let Err(e) = res.compile() {
            eprintln!("Failed to compile Windows resource: {e}");
        }
    }
}
