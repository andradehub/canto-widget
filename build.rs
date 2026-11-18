use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/app.manifest");
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-env-changed=WINDRES");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            root.join("assets/app.manifest").display()
        );
    } else {
        let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("app-resource.o");
        let status = Command::new(env::var_os("WINDRES").unwrap_or_else(|| "windres".into()))
            .current_dir(&root)
            .args([
                "--include-dir=assets",
                "-i",
                "assets/app.rc",
                "-O",
                "coff",
                "-o",
            ])
            .arg(&output)
            .status()
            .expect("Windows resource compiler (windres) is required");
        assert!(status.success(), "Could not embed the Windows manifest");
        println!("cargo:rustc-link-arg-bins={}", output.display());
    }
}
