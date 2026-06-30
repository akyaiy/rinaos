use core::panic::PanicInfo;
use x86_64;

use crate::println;

pub fn handle(_info: &PanicInfo) -> ! {
    println!("===> kernel panic: {}", _info.message());
    loop {
        x86_64::instructions::interrupts::disable();
        x86_64::instructions::hlt();
    }
}
