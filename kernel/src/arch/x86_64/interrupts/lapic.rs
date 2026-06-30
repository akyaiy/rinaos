use core::{
    ptr,
    sync::atomic::{AtomicU32, AtomicUsize, Ordering},
};

use x86_64::instructions::port::Port;
use x86_64::registers::model_specific::Msr;

use super::{LOCAL_TIMER_VECTOR, SPURIOUS_VECTOR};

const IA32_APIC_BASE: u32 = 0x1b;
const APIC_BASE_ENABLE: u64 = 1 << 11;
const APIC_BASE_ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;

const REG_ID: usize = 0x020;
const REG_EOI: usize = 0x0b0;
const REG_SPURIOUS: usize = 0x0f0;
const REG_LVT_TIMER: usize = 0x320;
const REG_TIMER_INITIAL_COUNT: usize = 0x380;
const REG_TIMER_CURRENT_COUNT: usize = 0x390;
const REG_TIMER_DIVIDE: usize = 0x3e0;

const TIMER_MASKED: u32 = 1 << 16;
const TIMER_PERIODIC: u32 = 1 << 17;
const TIMER_DIVIDE_BY_16: u32 = 0b0011;

const PIT_COMMAND: u16 = 0x43;
const PIT_CHANNEL_2: u16 = 0x42;
const PIT_GATE: u16 = 0x61;
const PIT_FREQUENCY_HZ: u64 = 1_193_182;
const CALIBRATION_MS: u64 = 10;
const CALIBRATION_PIT_TICKS: u16 = ((PIT_FREQUENCY_HZ * CALIBRATION_MS) / 1000) as u16;

static LAPIC_BASE: AtomicUsize = AtomicUsize::new(0);
static TIMER_TICKS_PER_MS: AtomicU32 = AtomicU32::new(0);
static TIMER_INITIAL_COUNT: AtomicU32 = AtomicU32::new(0);

pub unsafe fn init(local_apic_phys: u64, hhdm_offset: u64) {
    let base_phys = if local_apic_phys == 0 {
        read_base_phys()
    } else {
        local_apic_phys
    };

    enable_base(base_phys);
    LAPIC_BASE.store((base_phys + hhdm_offset) as usize, Ordering::Release);

    write(REG_SPURIOUS, 0x100 | SPURIOUS_VECTOR as u32);
    write(REG_LVT_TIMER, TIMER_MASKED | LOCAL_TIMER_VECTOR as u32);
    write(REG_TIMER_DIVIDE, TIMER_DIVIDE_BY_16);
}

pub fn id() -> u8 {
    unsafe { (read(REG_ID) >> 24) as u8 }
}

pub unsafe fn eoi() {
    write(REG_EOI, 0);
}

pub unsafe fn calibrate_timer() -> Option<u32> {
    write(REG_LVT_TIMER, TIMER_MASKED | LOCAL_TIMER_VECTOR as u32);
    write(REG_TIMER_DIVIDE, TIMER_DIVIDE_BY_16);
    write(REG_TIMER_INITIAL_COUNT, u32::MAX);

    pit_wait(CALIBRATION_PIT_TICKS);

    let elapsed = u32::MAX.wrapping_sub(read(REG_TIMER_CURRENT_COUNT));
    write(REG_TIMER_INITIAL_COUNT, 0);

    let ticks_per_ms = elapsed / CALIBRATION_MS as u32;
    if ticks_per_ms == 0 {
        None
    } else {
        TIMER_TICKS_PER_MS.store(ticks_per_ms, Ordering::Release);
        Some(ticks_per_ms)
    }
}

pub unsafe fn start_periodic_timer(period_ms: u32) {
    let ticks_per_ms = TIMER_TICKS_PER_MS.load(Ordering::Acquire);
    if ticks_per_ms == 0 {
        return;
    }

    let initial_count = ticks_per_ms.saturating_mul(period_ms.max(1));
    TIMER_INITIAL_COUNT.store(initial_count, Ordering::Release);
    write(REG_TIMER_DIVIDE, TIMER_DIVIDE_BY_16);
    write(REG_LVT_TIMER, TIMER_PERIODIC | LOCAL_TIMER_VECTOR as u32);
    write(REG_TIMER_INITIAL_COUNT, initial_count);
}

pub fn timer_elapsed_ns() -> u64 {
    let ticks_per_ms = TIMER_TICKS_PER_MS.load(Ordering::Acquire);
    let initial_count = TIMER_INITIAL_COUNT.load(Ordering::Acquire);

    if ticks_per_ms == 0 || initial_count == 0 {
        return 0;
    }

    let current_count = unsafe { read(REG_TIMER_CURRENT_COUNT) };
    let elapsed_ticks = initial_count
        .saturating_sub(current_count)
        .min(initial_count);

    (elapsed_ticks as u64 * 1_000_000) / ticks_per_ms as u64
}

unsafe fn pit_wait(ticks: u16) {
    let mut command = Port::<u8>::new(PIT_COMMAND);
    let mut channel = Port::<u8>::new(PIT_CHANNEL_2);
    let mut gate = Port::<u8>::new(PIT_GATE);

    let gate_value = gate.read();
    gate.write((gate_value & !0x02) | 0x01);
    command.write(0b1011_0000);
    channel.write((ticks & 0xff) as u8);
    channel.write((ticks >> 8) as u8);
    gate.write(gate_value | 0x03);

    while gate.read() & 0x20 == 0 {
        core::hint::spin_loop();
    }

    gate.write(gate_value);
}

unsafe fn read_base_phys() -> u64 {
    Msr::new(IA32_APIC_BASE).read() & APIC_BASE_ADDR_MASK
}

unsafe fn enable_base(base_phys: u64) {
    let mut msr = Msr::new(IA32_APIC_BASE);
    let value = msr.read();
    let flags = value & !(APIC_BASE_ADDR_MASK);
    msr.write((base_phys & APIC_BASE_ADDR_MASK) | flags | APIC_BASE_ENABLE);
}

unsafe fn read(offset: usize) -> u32 {
    let base = LAPIC_BASE.load(Ordering::Acquire);
    ptr::read_volatile((base + offset) as *const u32)
}

unsafe fn write(offset: usize, value: u32) {
    let base = LAPIC_BASE.load(Ordering::Acquire);
    ptr::write_volatile((base + offset) as *mut u32, value);
    let _ = read(REG_ID);
}
