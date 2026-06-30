use std::{fs, path::Path};

use rinasys_config_schema::{ConfigEmit, EmbeddedFilePath, EmitContext, KernelConfig};

pub fn generate_kernel_config(
    manifest_dir: &Path,
    config_path: &Path,
    out_path: &Path,
) -> Result<(), String> {
    let context = EmitContext::new(manifest_dir);
    let config_path = context.absolutize(config_path);

    println!("cargo:rerun-if-env-changed=RINASYS_KERNEL_CONFIG");
    println!("cargo:rerun-if-changed={}", config_path.display());

    let raw = fs::read_to_string(&config_path)
        .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
    let config: KernelConfig<EmbeddedFilePath> = toml::from_str(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", config_path.display()))?;

    let font_path = context.absolutize(&config.tty.font.path);
    println!("cargo:rerun-if-changed={}", font_path.display());

    config.emit_cfgs(&context, &mut Vec::new());

    let config_tokens = config.emit_config(&context);
    let tokens = quote::quote! {
        pub const CONFIG: ::rinasys_config_schema::KernelConfig<::rinasys_config_schema::EmbeddedFile> =
            #config_tokens;
    };

    fs::write(out_path, tokens.to_string())
        .map_err(|error| format!("failed to write {}: {error}", out_path.display()))
}
