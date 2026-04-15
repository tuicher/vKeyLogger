fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_manifest_file("keyviz.exe.manifest");
        res.compile().unwrap();
    }
    tauri_build::build()
}
