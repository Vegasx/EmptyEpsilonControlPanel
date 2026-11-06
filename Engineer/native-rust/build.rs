use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=app.rc");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("app.o");
    let compiler =
        env::var_os("EPSILON_WINDRES").expect("Build with build.ps1 (EPSILON_WINDRES missing)");
    assert!(Command::new(compiler)
        .args(["app.rc", "-O", "coff", "-o"])
        .arg(&output)
        .status()
        .unwrap()
        .success());
    println!("cargo:rustc-link-arg={}", output.display());
}
