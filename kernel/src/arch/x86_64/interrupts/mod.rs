use core::sync::atomic::{AtomicU8, Ordering};

use crate::{boot::BootInfo, println};

use super::acpi;

mod ioapic;
mod lapic;
mod pic;

pub const IRQ_BASE: u8 = 0x20;
pub const TIMER_VECTOR: u8 = IRQ_BASE;
pub const KEYBOARD_VECTOR: u8 = IRQ_BASE + 1;
pub const LOCAL_TIMER_VECTOR: u8 = 0xef;
pub const SPURIOUS_VECTOR: u8 = 0xff;

const BACKEND_NONE: u8 = 0;
const BACKEND_PIC: u8 = 1;
const BACKEND_APIC: u8 = 2;

static BACKEND: AtomicU8 = AtomicU8::new(BACKEND_NONE);

pub fn init(boot_info: &BootInfo) {
    x86_64::instructions::interrupts::disable();

    if unsafe { init_apic(boot_info) } {
        BACKEND.store(BACKEND_APIC, Ordering::Release);
    } else {
        unsafe {
            pic::init();
            pic::enable_irq(0);
            pic::enable_irq(1);
        }
        BACKEND.store(BACKEND_PIC, Ordering::Release);
        println!("interrupts: using legacy PIC fallback");
    }
}

pub unsafe fn eoi(vector: u8) {
    match BACKEND.load(Ordering::Acquire) {
        BACKEND_APIC => lapic::eoi(),
        BACKEND_PIC if vector >= IRQ_BASE && vector < IRQ_BASE + 16 => {
            pic::eoi(vector - IRQ_BASE);
        }
        _ => {}
    }
}

pub unsafe fn local_timer_eoi() {
    if BACKEND.load(Ordering::Acquire) == BACKEND_APIC {
        lapic::eoi();
    }
}

unsafe fn init_apic(boot_info: &BootInfo) -> bool {
    let Some(rsdp_addr) = boot_info.rsdp_addr else {
        return false;
    };

    let Some(madt) = acpi::find_madt(rsdp_addr, boot_info.hhdm_offset) else {
        return false;
    };

    if madt.io_apic_count == 0 {
        return false;
    }

    pic::mask_all();
    lapic::init(madt.local_apic_addr, boot_info.hhdm_offset);

    if !ioapic::init(&madt, boot_info.hhdm_offset, lapic::id()) {
        return false;
    }

    ioapic::enable_irq(0);
    ioapic::enable_irq(1);

    match lapic::calibrate_timer() {
        Some(ticks_per_ms) => {
            lapic::start_periodic_timer(10);
            println!(
                "interrupts: using APIC, lapic timer {} ticks/ms",
                ticks_per_ms
            );
        }
        None => {
            println!("interrupts: using APIC, lapic timer calibration failed");
        }
    }

    true
}
