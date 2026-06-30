use std::{env, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let config_path = env::var("RINASYS_KERNEL_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest_dir.join("config/qemu.toml"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    rinasys_config_codegen::generate_kernel_config(
        &manifest_dir,
        &config_path,
        &out_dir.join("config.rs"),
    )
    .expect("failed to generate kernel config");
}
