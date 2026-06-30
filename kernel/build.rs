use std::{env, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let config_path = env::var("RINASYS_KERNEL_CONFIG")
        .map(PathBuf::from)
        .expect("RINASYS_KERNEL_CONFIG must point to a kernel config TOML");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    rinasys_config_codegen::generate_kernel_config(
        &manifest_dir,
        &config_path,
        &out_dir.join("config.rs"),
    )
    .expect("failed to generate kernel config");
}
