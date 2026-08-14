fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rustc-link-arg-bin=simple-window-manager-rs=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=simple-window-manager-rs=/MANIFESTUAC:level='requireAdministrator' uiAccess='false'"
        );
    }
}
