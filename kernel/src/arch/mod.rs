mod x86_64;

pub fn init(boot_info: &crate::boot::BootInfo) {
    x86_64::init(boot_info);
}
