#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

mod boot;
mod config;
mod drivers;
mod krnl;
mod sync;
mod arch;

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    main()
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn main() -> ! {
    let boot_info = boot::init();

    krnl::init(boot_info);
}
