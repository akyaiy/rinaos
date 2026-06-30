use crate::arch;
use crate::boot::BootInfo;
use crate::drivers::console;
use crate::drivers::framebuffer;
use crate::drivers::keyboard;
use crate::drivers::serial;
use crate::mm;
use crate::sched;
use crate::vfs;
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
    keyboard::init();
    klog_debug!("keyboard initialized");
    klog_debug!("flushing klog");
    console::flush_klog();
    arch::init(&boot_info);
    mm::init(&boot_info);
    klog_debug!("memory manager initialized");
    vfs::init();
    klog_debug!("vfs initialized");
    smoke_test_vfs();
    sched::init();
    sched::spawn_kernel(printer_task_a);
    sched::spawn_kernel(printer_task_b);
    klog_debug!("main subsystems initialized");
    x86_64::instructions::interrupts::enable();
    klog_debug!("interrupts enabled");
    sched::yield_now();
    klog_info!("halting kernel proccess");
    halt()
}

fn smoke_test_vfs() {
    match vfs::open("/dev/console", vfs::OpenFlags::WRONLY) {
        Ok(mut console) => {
            if console.write(b"vfs: /dev/console ready\n").is_err() {
                klog_error!("vfs: failed to write to /dev/console");
            }
        }
        Err(_) => klog_error!("vfs: failed to open /dev/console"),
    }
}

fn printer_task_a() -> ! {
    loop {
        crate::println!("\x1b[31mpkl gay\x1b[0m");
        for _ in 0..5_000 {
            core::hint::spin_loop();
        }
        sched::yield_now();
    }
}

fn printer_task_b() -> ! {
    loop {
        crate::println!("\x1b[34mpkl gay\x1b[0m");
        for _ in 0..5_000 {
            core::hint::spin_loop();
        }
        sched::yield_now();
    }
}

fn halt() -> ! {
    loop {
        keyboard::poll();
        console::flush_klog();
        x86_64::instructions::hlt();
    }
}
