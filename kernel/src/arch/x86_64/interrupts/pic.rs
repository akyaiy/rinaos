use x86_64::instructions::port::Port;

use super::IRQ_BASE;

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xa0;
const PIC2_DATA: u16 = 0xa1;
const PIT_COMMAND: u16 = 0x43;
const PIT_CHANNEL_0: u16 = 0x40;
const PIT_FREQUENCY_HZ: u64 = 1_193_182;

const PIC_EOI: u8 = 0x20;

const ICW1_ICW4: u8 = 0x01;
const ICW1_INIT: u8 = 0x10;
const ICW4_8086: u8 = 0x01;

const PIT_CHANNEL_0_ACCESS_LOHI: u8 = 0b0011_0000;
const PIT_MODE_RATE_GENERATOR: u8 = 0b0000_0100;

pub unsafe fn init() {
    let mut pic1_command = Port::<u8>::new(PIC1_COMMAND);
    let mut pic1_data = Port::<u8>::new(PIC1_DATA);
    let mut pic2_command = Port::<u8>::new(PIC2_COMMAND);
    let mut pic2_data = Port::<u8>::new(PIC2_DATA);

    pic1_command.write(ICW1_INIT | ICW1_ICW4);
    io_wait();
    pic2_command.write(ICW1_INIT | ICW1_ICW4);
    io_wait();

    pic1_data.write(IRQ_BASE);
    io_wait();
    pic2_data.write(IRQ_BASE + 8);
    io_wait();

    pic1_data.write(4);
    io_wait();
    pic2_data.write(2);
    io_wait();

    pic1_data.write(ICW4_8086);
    io_wait();
    pic2_data.write(ICW4_8086);
    io_wait();

    mask_all();
}

pub unsafe fn init_pit_timer(frequency_hz: u32) -> u64 {
    let divisor = pit_divisor(frequency_hz);
    let mut command = Port::<u8>::new(PIT_COMMAND);
    let mut channel = Port::<u8>::new(PIT_CHANNEL_0);

    command.write(PIT_CHANNEL_0_ACCESS_LOHI | PIT_MODE_RATE_GENERATOR);
    channel.write((divisor & 0xff) as u8);
    channel.write((divisor >> 8) as u8);

    (1_000_000_000u64 * divisor as u64) / PIT_FREQUENCY_HZ
}

pub unsafe fn mask_all() {
    Port::<u8>::new(PIC1_DATA).write(0xff);
    Port::<u8>::new(PIC2_DATA).write(0xff);
}

pub unsafe fn enable_irq(irq: u8) {
    let (port, bit) = if irq < 8 {
        (PIC1_DATA, irq)
    } else {
        (PIC2_DATA, irq - 8)
    };

    let mut data = Port::<u8>::new(port);
    let mask = data.read() & !(1 << bit);
    data.write(mask);
}

pub unsafe fn disable_irq(irq: u8) {
    let (port, bit) = if irq < 8 {
        (PIC1_DATA, irq)
    } else {
        (PIC2_DATA, irq - 8)
    };

    let mut data = Port::<u8>::new(port);
    let mask = data.read() | (1 << bit);
    data.write(mask);
}

pub unsafe fn eoi(irq: u8) {
    if irq >= 8 {
        Port::<u8>::new(PIC2_COMMAND).write(PIC_EOI);
    }

    Port::<u8>::new(PIC1_COMMAND).write(PIC_EOI);
}

unsafe fn io_wait() {
    Port::<u8>::new(0x80).write(0);
}

fn pit_divisor(frequency_hz: u32) -> u16 {
    let frequency_hz = frequency_hz.max(19).min(PIT_FREQUENCY_HZ as u32);
    let divisor = ((PIT_FREQUENCY_HZ + frequency_hz as u64 / 2) / frequency_hz as u64)
        .max(1)
        .min(u16::MAX as u64);

    divisor as u16
}
