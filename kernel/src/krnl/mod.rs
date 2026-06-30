use crate::arch;
use crate::boot::BootInfo;
use crate::drivers::console;
use crate::drivers::framebuffer;
use crate::drivers::serial;
use crate::mm;
use crate::{klog_debug, klog_error, klog_info};
use x86_64;

pub mod klog;
pub mod panic_handler;
pub mod time;

pub fn init(boot_info: BootInfo) -> ! {
    serial::init();
    klog::configure_from_cmdline(boot_info.cmdline);
    klog_info!("starting kernel");
    klog_debug!("disabling interrupts for kernel initializing");
    x86_64::instructions::interrupts::disable();
    serial::init();
    klog_debug!("serial initialized");
    if let Some(fb) = boot_info.framebuffer {
        framebuffer::init(fb);
    } else {
        klog_error!("no framebuffer found");
    }
    console::init();
    klog_debug!("console initialized");
    klog_debug!("flushing klog");
    console::flush_klog();
    arch::init(&boot_info);
    mm::init(&boot_info);
    klog_debug!("memory manager initialized");
    klog_debug!("main subsystems initialized");
    x86_64::instructions::interrupts::enable();
    klog_debug!("interrupts enabled");
    klog_info!("halting kernel proccess");
    halt()
}

fn halt() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
