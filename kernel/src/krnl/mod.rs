use crate::boot::BootInfo;
use crate::arch;
use crate::drivers::console;
use crate::drivers::framebuffer;
use crate::println;
use x86_64;

pub mod panic_handler;

pub fn init(boot_info: BootInfo) -> ! {
    if let Some(fb) = boot_info.framebuffer {
        framebuffer::init(fb);
        console::init();
        println!("starting kernel");
        arch::init();
    }

    panic!("piskua");

    loop {
        x86_64::instructions::interrupts::disable();
        x86_64::instructions::hlt();
    }
}
