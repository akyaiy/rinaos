const MAX_PARAMS: usize = 4;

#[derive(Clone, Copy)]
pub enum EscapeState {
    Ground,
    Escape,
    Csi {
        params: [u16; MAX_PARAMS],
        current: usize,
        has_value: bool,
    },
}

#[derive(Clone, Copy)]
pub enum AnsiAction {
    None,
    Put(u8),
    ClearScreen,
    ClearLine,
    CursorHome,
    CursorUp(usize),
    CursorDown(usize),
    CursorRight(usize),
    CursorLeft(usize),
    Sgr {
        params: [u16; MAX_PARAMS],
        len: usize,
    },
}

impl EscapeState {
    pub const fn new() -> Self {
        Self::Ground
    }

    pub fn feed(&mut self, byte: u8) -> AnsiAction {
        match *self {
            Self::Ground => {
                if byte == 0x1b {
                    *self = Self::Escape;
                    AnsiAction::None
                } else {
                    AnsiAction::Put(byte)
                }
            }
            Self::Escape => {
                if byte == b'[' {
                    *self = Self::Csi {
                        params: [0; MAX_PARAMS],
                        current: 0,
                        has_value: false,
                    };
                    AnsiAction::None
                } else {
                    *self = Self::Ground;
                    AnsiAction::None
                }
            }
            Self::Csi {
                mut params,
                mut current,
                mut has_value,
            } => {
                if byte.is_ascii_digit() {
                    has_value = true;
                    params[current] = params[current]
                        .saturating_mul(10)
                        .saturating_add((byte - b'0') as u16);
                    *self = Self::Csi {
                        params,
                        current,
                        has_value,
                    };
                    return AnsiAction::None;
                }

                if byte == b';' {
                    current = (current + 1).min(MAX_PARAMS - 1);
                    has_value = false;
                    *self = Self::Csi {
                        params,
                        current,
                        has_value,
                    };
                    return AnsiAction::None;
                }

                *self = Self::Ground;

                let len = if has_value || current > 0 {
                    current + 1
                } else {
                    0
                };

                match byte {
                    b'H' | b'f' => AnsiAction::CursorHome,
                    b'J' if param_or_default(params, 0, 0) == 2 => AnsiAction::ClearScreen,
                    b'K' => AnsiAction::ClearLine,
                    b'A' => AnsiAction::CursorUp(param_or_default(params, 0, 1) as usize),
                    b'B' => AnsiAction::CursorDown(param_or_default(params, 0, 1) as usize),
                    b'C' => AnsiAction::CursorRight(param_or_default(params, 0, 1) as usize),
                    b'D' => AnsiAction::CursorLeft(param_or_default(params, 0, 1) as usize),
                    b'm' => AnsiAction::Sgr { params, len },
                    _ => AnsiAction::None,
                }
            }
        }
    }
}

fn param_or_default(params: [u16; MAX_PARAMS], index: usize, default: u16) -> u16 {
    if params[index] == 0 {
        default
    } else {
        params[index]
    }
}
