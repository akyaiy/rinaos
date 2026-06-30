use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, KeyCode, ScancodeSet1};
use x86_64::instructions::interrupts;
use x86_64::instructions::port::Port;

use crate::drivers::console;
use crate::sync::spinlock::SpinLock;

const PS2_DATA_PORT: u16 = 0x60;
const SCANCODE_QUEUE_SIZE: usize = 256;

static KEYBOARD: SpinLock<Option<Keyboard<layouts::Us104Key, ScancodeSet1>>> =
    SpinLock::new(None);
static SCANCODE_QUEUE: SpinLock<ScancodeQueue> = SpinLock::new(ScancodeQueue::new());

struct ScancodeQueue {
    bytes: [u8; SCANCODE_QUEUE_SIZE],
    read: usize,
    write: usize,
    len: usize,
    dropped: usize,
}

impl ScancodeQueue {
    const fn new() -> Self {
        Self {
            bytes: [0; SCANCODE_QUEUE_SIZE],
            read: 0,
            write: 0,
            len: 0,
            dropped: 0,
        }
    }

    fn push(&mut self, byte: u8) {
        if self.len == SCANCODE_QUEUE_SIZE {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }

        self.bytes[self.write] = byte;
        self.write = (self.write + 1) % SCANCODE_QUEUE_SIZE;
        self.len += 1;
    }

    fn pop(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }

        let byte = self.bytes[self.read];
        self.read = (self.read + 1) % SCANCODE_QUEUE_SIZE;
        self.len -= 1;

        Some(byte)
    }
}

pub fn init() {
    *KEYBOARD.lock() = Some(Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::Ignore,
    ));
}

pub fn handle_interrupt() {
    let scancode = unsafe { Port::<u8>::new(PS2_DATA_PORT).read() };
    push_scancode(scancode);
}

pub fn poll() {
    while let Some(scancode) = pop_scancode() {
        decode_scancode(scancode);
    }
}

fn push_scancode(scancode: u8) {
    interrupts::without_interrupts(|| {
        SCANCODE_QUEUE.lock().push(scancode);
    });
}

fn pop_scancode() -> Option<u8> {
    interrupts::without_interrupts(|| SCANCODE_QUEUE.lock().pop())
}

fn decode_scancode(scancode: u8) {
    let mut keyboard = KEYBOARD.lock();
    let Some(keyboard) = keyboard.as_mut() else {
        return;
    };

    if let Ok(Some(event)) = keyboard.add_byte(scancode) {
        if let Some(decoded_key) = keyboard.process_keyevent(event) {
            dispatch_key(decoded_key);
        }
    }
}

fn dispatch_key(key: DecodedKey) {
    match key {
        DecodedKey::Unicode(character) if character.is_ascii() => {
            let mut buffer = [0; 4];
            console::write_str(character.encode_utf8(&mut buffer));
        }
        DecodedKey::Unicode(_) => {}
        DecodedKey::RawKey(KeyCode::Return | KeyCode::NumpadEnter) => console::put_byte(b'\n'),
        DecodedKey::RawKey(KeyCode::Backspace) => console::put_byte(b'\x08'),
        DecodedKey::RawKey(KeyCode::Tab) => console::put_byte(b'\t'),
        DecodedKey::RawKey(KeyCode::Escape) => console::write_str("^["),
        DecodedKey::RawKey(_) => {}
    }
}
