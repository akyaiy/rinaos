use crate::boot::BootInfo;
use crate::drivers::console;
use crate::drivers::framebuffer;

pub fn init(boot_info: BootInfo) -> ! {
    if let Some(fb) = boot_info.framebuffer {
        framebuffer::init(fb);
        console::init();
        console::write_str("pkl GaY\n");
        console::write_str("pIKi: \x1b[31mG\x1b[0m \x1b[32mA\x1b[0m \x1b[34mY\x1b[0m\n");
    }

    loop {}
}
