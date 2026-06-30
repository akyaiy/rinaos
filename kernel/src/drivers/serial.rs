use core::fmt::{self, Write};

use x86_64::instructions::port::Port;

use crate::config::CONFIG;
use crate::krnl::klog;
use crate::sync::spinlock::IrqSpinLock;

const COM1: u16 = 0x3f8;

static SERIAL: IrqSpinLock<SerialPort> = IrqSpinLock::new(SerialPort::new(COM1));

pub struct SerialPort {
    base: u16,
    initialized: bool,
    last_klog_sequence: u64,
}

impl SerialPort {
    pub const fn new(base: u16) -> Self {
        Self {
            base,
            initialized: false,
            last_klog_sequence: 0,
        }
    }

    unsafe fn init(&mut self) {
        let mut data = Port::<u8>::new(self.base);
        let mut interrupt_enable = Port::<u8>::new(self.base + 1);
        let mut fifo_control = Port::<u8>::new(self.base + 2);
        let mut line_control = Port::<u8>::new(self.base + 3);
        let mut modem_control = Port::<u8>::new(self.base + 4);

        interrupt_enable.write(0x00);
        line_control.write(0x80);
        data.write(0x03);
        interrupt_enable.write(0x00);
        line_control.write(0x03);
        fifo_control.write(0xc7);
        modem_control.write(0x0b);

        self.initialized = true;
    }

    fn write_byte(&mut self, byte: u8) {
        if !self.initialized {
            return;
        }

        unsafe {
            wait_transmit_empty(self.base);
            Port::<u8>::new(self.base).write(byte);
        }
    }
}

impl Write for SerialPort {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }

            self.write_byte(byte);
        }

        Ok(())
    }
}

pub fn init() {
    if CONFIG.serial.enabled {
        unsafe {
            SERIAL.lock().init();
        }
    }
}

pub fn write_str(text: &str) {
    let _ = SERIAL.lock().write_str(text);
}

pub fn _print(args: fmt::Arguments) {
    let _ = SERIAL.lock().write_fmt(args);
}

pub fn flush_klog() {
    let mut serial = SERIAL.lock();

    if !serial.initialized {
        return;
    }

    while let Some(entry) = klog::next_after(serial.last_klog_sequence) {
        let timestamp_us = entry.timestamp_ns / 1_000;
        let _ = serial.write_fmt(format_args!(
            "[{:>6}.{:06}] [{}] {}\n",
            timestamp_us / 1_000_000,
            timestamp_us % 1_000_000,
            entry.level.as_str(),
            entry.message(),
        ));
        serial.last_klog_sequence = entry.sequence;
    }
}

unsafe fn wait_transmit_empty(base: u16) {
    let mut line_status = Port::<u8>::new(base + 5);

    for _ in 0..100_000 {
        if line_status.read() & 0x20 != 0 {
            break;
        }

        core::hint::spin_loop();
    }
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::drivers::serial::_print(core::format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! serial_println {
    () => {
        $crate::serial_print!("\n")
    };
    ($fmt:literal $(, $($arg:tt)+)?) => {
        $crate::serial_print!(concat!($fmt, "\n") $(, $($arg)+)?)
    };
}
