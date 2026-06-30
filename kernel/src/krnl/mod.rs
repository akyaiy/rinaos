use crate::boot::BootInfo;
use crate::arch;
use crate::drivers::console;
use crate::drivers::framebuffer;

pub fn init(boot_info: BootInfo) -> ! {
    if let Some(fb) = boot_info.framebuffer {
        framebuffer::init(fb);
        console::init();
        console::write_str("starting kernel\n");
        arch::init();
    }

    loop {}
}
