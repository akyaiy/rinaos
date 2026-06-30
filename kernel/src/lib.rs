#![no_std]

pub mod config;

pub fn serial_enabled() -> bool {
    config::CONFIG.serial.enabled
}

#[cfg(rinasys_serial_enabled)]
pub fn serial_compiled() -> bool {
    true
}

#[cfg(not(rinasys_serial_enabled))]
pub fn serial_compiled() -> bool {
    false
}

pub fn serial_irq() -> u8 {
    config::CONFIG.serial.irq
}

pub fn tty_size() -> (usize, usize) {
    (config::CONFIG.tty.columns, config::CONFIG.tty.rows)
}

pub fn tty_font() -> &'static [u8] {
    config::CONFIG.tty.font.bytes
}
