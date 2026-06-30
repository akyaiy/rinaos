pub mod boot_info;
mod limine;

pub use boot_info::*;

pub fn init() -> BootInfo {
    limine::init()
}
