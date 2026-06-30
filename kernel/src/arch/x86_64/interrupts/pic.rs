use x86_64::instructions::port::Port;

use super::IRQ_BASE;

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xa0;
const PIC2_DATA: u16 = 0xa1;

const PIC_EOI: u8 = 0x20;

const ICW1_ICW4: u8 = 0x01;
const ICW1_INIT: u8 = 0x10;
const ICW4_8086: u8 = 0x01;

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
