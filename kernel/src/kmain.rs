#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![allow(dead_code)]

extern crate alloc;

mod arch;
mod boot;
mod config;
mod drivers;
mod krnl;
mod mm;
mod sync;

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    main()
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    krnl::panic_handler::handle(_info)
}

fn main() -> ! {
    let boot_info = boot::init();

    krnl::init(boot_info);
}
