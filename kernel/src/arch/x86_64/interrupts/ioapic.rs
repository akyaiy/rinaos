use core::ptr;

use crate::arch::x86_64::acpi::{InterruptOverride, Madt};

use super::IRQ_BASE;

const IOREGSEL: usize = 0x00;
const IOWIN: usize = 0x10;

const REG_IOAPICVER: u8 = 0x01;
const REG_REDTBL_BASE: u8 = 0x10;

const REDTBL_MASKED: u64 = 1 << 16;
const REDTBL_TRIGGER_LEVEL: u64 = 1 << 15;
const REDTBL_POLARITY_LOW: u64 = 1 << 13;

const POLARITY_ACTIVE_LOW: u16 = 0b11;
const TRIGGER_LEVEL: u16 = 0b11 << 2;
const MAX_IO_APICS: usize = 8;
const MAX_INTERRUPT_OVERRIDES: usize = 32;

#[derive(Clone, Copy)]
struct IoApicState {
    base: usize,
    gsi_base: u32,
    redirection_entries: u32,
}

static mut IO_APICS: [IoApicState; MAX_IO_APICS] = [IoApicState {
    base: 0,
    gsi_base: 0,
    redirection_entries: 0,
}; 8];
static mut IO_APIC_COUNT: usize = 0;
static mut INTERRUPT_OVERRIDES: [InterruptOverride; MAX_INTERRUPT_OVERRIDES] = [InterruptOverride {
    source_irq: 0,
    gsi: 0,
    flags: 0,
}; 32];
static mut INTERRUPT_OVERRIDE_COUNT: usize = 0;
static mut DESTINATION_APIC_ID: u8 = 0;

pub unsafe fn init(madt: &Madt, hhdm_offset: u64, destination_apic_id: u8) -> bool {
    IO_APIC_COUNT = 0;
    INTERRUPT_OVERRIDE_COUNT = 0;
    DESTINATION_APIC_ID = destination_apic_id;

    for entry in madt.io_apics[..madt.io_apic_count].iter() {
        if IO_APIC_COUNT >= MAX_IO_APICS {
            break;
        }

        let base = entry.address as u64 + hhdm_offset;
        let mut state = IoApicState {
            base: base as usize,
            gsi_base: entry.gsi_base,
            redirection_entries: 0,
        };
        state.redirection_entries = ((read(&state, REG_IOAPICVER) >> 16) & 0xff) + 1;
        mask_all(&state);

        IO_APICS[IO_APIC_COUNT] = state;
        IO_APIC_COUNT += 1;
    }

    for entry in madt.interrupt_overrides[..madt.interrupt_override_count].iter() {
        if INTERRUPT_OVERRIDE_COUNT >= MAX_INTERRUPT_OVERRIDES {
            break;
        }

        INTERRUPT_OVERRIDES[INTERRUPT_OVERRIDE_COUNT] = *entry;
        INTERRUPT_OVERRIDE_COUNT += 1;
    }

    IO_APIC_COUNT > 0
}

pub unsafe fn enable_irq(irq: u8) {
    let (gsi, flags) = mapped_irq(irq);
    let Some((apic, index)) = find_gsi(gsi) else {
        return;
    };

    let vector = IRQ_BASE as u64 + irq as u64;
    let mut entry = vector | ((DESTINATION_APIC_ID as u64) << 56);

    if flags & POLARITY_ACTIVE_LOW == POLARITY_ACTIVE_LOW {
        entry |= REDTBL_POLARITY_LOW;
    }

    if flags & TRIGGER_LEVEL == TRIGGER_LEVEL {
        entry |= REDTBL_TRIGGER_LEVEL;
    }

    write_redirection(&apic, index, entry);
}

pub unsafe fn disable_irq(irq: u8) {
    let (gsi, _) = mapped_irq(irq);
    let Some((apic, index)) = find_gsi(gsi) else {
        return;
    };

    let entry = read_redirection(&apic, index) | REDTBL_MASKED;
    write_redirection(&apic, index, entry);
}

unsafe fn mask_all(apic: &IoApicState) {
    for index in 0..apic.redirection_entries {
        write_redirection(apic, index, REDTBL_MASKED);
    }
}

unsafe fn mapped_irq(irq: u8) -> (u32, u16) {
    for entry in INTERRUPT_OVERRIDES[..INTERRUPT_OVERRIDE_COUNT].iter() {
        if entry.source_irq == irq {
            return (entry.gsi, entry.flags);
        }
    }

    (irq as u32, 0)
}

unsafe fn find_gsi(gsi: u32) -> Option<(IoApicState, u32)> {
    for apic in IO_APICS[..IO_APIC_COUNT].iter().copied() {
        if gsi >= apic.gsi_base && gsi < apic.gsi_base + apic.redirection_entries {
            return Some((apic, gsi - apic.gsi_base));
        }
    }

    None
}

unsafe fn read_redirection(apic: &IoApicState, index: u32) -> u64 {
    let low = read(apic, REG_REDTBL_BASE + (index as u8 * 2)) as u64;
    let high = read(apic, REG_REDTBL_BASE + (index as u8 * 2) + 1) as u64;
    low | (high << 32)
}

unsafe fn write_redirection(apic: &IoApicState, index: u32, value: u64) {
    write(
        apic,
        REG_REDTBL_BASE + (index as u8 * 2) + 1,
        (value >> 32) as u32,
    );
    write(apic, REG_REDTBL_BASE + (index as u8 * 2), value as u32);
}

unsafe fn read(apic: &IoApicState, reg: u8) -> u32 {
    ptr::write_volatile((apic.base + IOREGSEL) as *mut u32, reg as u32);
    ptr::read_volatile((apic.base + IOWIN) as *const u32)
}

unsafe fn write(apic: &IoApicState, reg: u8, value: u32) {
    ptr::write_volatile((apic.base + IOREGSEL) as *mut u32, reg as u32);
    ptr::write_volatile((apic.base + IOWIN) as *mut u32, value);
}
