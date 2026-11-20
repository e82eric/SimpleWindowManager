use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rustc-link-arg-bin=simple-window-manager-rs=/MANIFEST:EMBED");
    // SWM asks for administrator rights when it starts, unless it is built with
    // SWM_AS_INVOKER set (`build.ps1 -AsInvoker`): then it runs as whoever starts
    // it, and cannot move the windows of programs that run elevated.
    println!("cargo:rerun-if-env-changed=SWM_AS_INVOKER");
    let as_invoker = std::env::var_os("SWM_AS_INVOKER").is_some_and(|set| !set.is_empty() && set != "0");
    let level = if as_invoker { "asInvoker" } else { "requireAdministrator" };
    println!(
        "cargo:rustc-link-arg-bin=simple-window-manager-rs=/MANIFESTUAC:level='{level}' uiAccess='false'"
    );
    // SWM runs elevated: resolve the DLLs it links against from System32 only,
    // never from the executable's folder, where a planted copy would run as admin.
    // 0x800 is LOAD_LIBRARY_SEARCH_SYSTEM32.
    println!("cargo:rustc-link-arg-bin=simple-window-manager-rs=/DEPENDENTLOADFLAG:0x800");

    println!("cargo:rerun-if-env-changed=SWM_CONFIG_RS");
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let configured = std::env::var_os("SWM_CONFIG_RS")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("default_user_config.rs"));
    let configured = if configured.is_absolute() {
        configured
    } else {
        manifest_dir.join(configured)
    };
    let configured = configured.canonicalize().unwrap_or_else(|error| {
        panic!(
            "unable to resolve SWM_CONFIG_RS file {}: {error}",
            configured.display()
        )
    });
    if !Path::new(&configured).is_file() {
        panic!("SWM_CONFIG_RS is not a file: {}", configured.display());
    }
    let configured = configured.to_str().unwrap_or_else(|| {
        panic!(
            "SWM_CONFIG_RS path is not valid Unicode: {}",
            configured.display()
        )
    });
    println!("cargo:rerun-if-changed={configured}");
    println!("cargo:rustc-env=SWM_CONFIG_RS_RESOLVED={configured}");
}
