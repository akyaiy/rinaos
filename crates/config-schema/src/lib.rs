#![no_std]

#[cfg(any(feature = "serde", feature = "codegen"))]
extern crate alloc;

#[cfg(feature = "codegen")]
extern crate std;

#[cfg(feature = "codegen")]
extern crate self as rinasys_config_schema;

#[cfg(feature = "codegen")]
pub use rinasys_config_derive::ConfigEmit;

// config

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "codegen", derive(ConfigEmit))]
pub struct KernelConfig<File> {
    pub serial: SerialConfig,
    
    pub framebuffer: FramebufferConfig<File>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "codegen", derive(ConfigEmit))]
pub struct SerialConfig {
    #[cfg_attr(feature = "serde", serde(default))]
    pub enabled: bool,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "codegen", derive(ConfigEmit))]
pub struct FramebufferConfig<File> {
    pub font: File,
    
    #[cfg_attr(feature = "serde", serde(default))]
    pub columns: usize,
    
    #[cfg_attr(feature = "serde", serde(default))]
    pub rows: usize,
    
    pub foreground: FramebufferColorConfig,
    
    pub background: FramebufferColorConfig,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "codegen", derive(ConfigEmit))]
pub struct FramebufferColorConfig {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

// end of config

#[derive(Debug, Clone, Copy)]
pub struct EmbeddedFile {
    pub bytes: &'static [u8],
}

#[cfg(any(feature = "serde", feature = "codegen"))]
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[serde(transparent)]
pub struct EmbeddedFilePath {
    pub path: alloc::string::String,
}

#[cfg(feature = "codegen")]
pub struct EmitContext {
    base_dir: std::path::PathBuf,
    cfg_prefix: &'static str,
}

#[cfg(feature = "codegen")]
impl EmitContext {
    pub fn new(base_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
            cfg_prefix: "rinasys",
        }
    }

    pub fn absolutize(&self, path: impl AsRef<std::path::Path>) -> std::path::PathBuf {
        let path = path.as_ref();
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_dir.join(path)
        }
    }

    pub fn cfg_name(&self, path: &[&str]) -> std::string::String {
        let mut name = std::string::String::from(self.cfg_prefix);

        for segment in path {
            name.push('_');
            name.push_str(segment);
        }

        name.replace('-', "_")
    }
}

#[cfg(feature = "codegen")]
pub trait ConfigEmit {
    fn emit_config(&self, context: &EmitContext) -> proc_macro2::TokenStream;

    fn emit_cfgs(&self, _context: &EmitContext, _path: &mut std::vec::Vec<&'static str>) {}

    fn emit_rerun_if_changed(&self, _context: &EmitContext) {}
}

#[cfg(feature = "codegen")]
macro_rules! impl_config_emit_literal {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ConfigEmit for $ty {
                fn emit_config(&self, _context: &EmitContext) -> proc_macro2::TokenStream {
                    let value = *self;
                    quote::quote! { #value }
                }
            }
        )*
    };
}

#[cfg(feature = "codegen")]
impl_config_emit_literal!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

#[cfg(feature = "codegen")]
impl ConfigEmit for bool {
    fn emit_config(&self, _context: &EmitContext) -> proc_macro2::TokenStream {
        let value = *self;
        quote::quote! { #value }
    }

    fn emit_cfgs(&self, context: &EmitContext, path: &mut std::vec::Vec<&'static str>) {
        let cfg_name = context.cfg_name(path);

        std::println!("cargo:rustc-check-cfg=cfg({cfg_name})");

        if *self {
            std::println!("cargo:rustc-cfg={cfg_name}");
        }
    }
}

#[cfg(feature = "codegen")]
impl ConfigEmit for EmbeddedFilePath {
    fn emit_config(&self, context: &EmitContext) -> proc_macro2::TokenStream {
        let path = context.absolutize(&self.path);
        let path = proc_macro2::Literal::string(&path.to_string_lossy());

        quote::quote! {
            ::rinasys_config_schema::EmbeddedFile {
                bytes: include_bytes!(#path),
            }
        }
    }

    fn emit_rerun_if_changed(&self, context: &EmitContext) {
        let path = context.absolutize(&self.path);
        std::println!("cargo:rerun-if-changed={}", path.display());
    }
}
