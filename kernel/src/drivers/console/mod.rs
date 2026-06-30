mod ansi;
mod font;

use core::ptr;

use crate::config::CONFIG;
use crate::drivers::console::ansi::{AnsiAction, EscapeState};
use crate::drivers::console::font::{Font, FALLBACK_FONT};
use crate::drivers::framebuffer::{self, Color};
use crate::sync::spinlock::SpinLock;

const MAX_COLS: usize = 240;
const MAX_ROWS: usize = 135;
const MAX_CELLS: usize = MAX_COLS * MAX_ROWS;
const TAB_WIDTH: usize = 4;

static CONSOLE: SpinLock<Option<Console>> = SpinLock::new(None);
static mut CELLS: [Cell; MAX_CELLS] = [Cell::blank(); MAX_CELLS];

#[derive(Clone, Copy)]
struct Cell {
    ch: u8,
    fg: Color,
    bg: Color,
}

pub struct Console {
    font: Font,
    cols: usize,
    rows: usize,
    cursor_x: usize,
    cursor_y: usize,
    cursor_visible: bool,
    fg: Color,
    bg: Color,
    default_fg: Color,
    default_bg: Color,
    escape_state: EscapeState,
}

impl Cell {
    const fn blank() -> Self {
        Self {
            ch: b' ',
            fg: Color::WHITE,
            bg: Color::BLACK,
        }
    }

    const fn with_colors(fg: Color, bg: Color) -> Self {
        Self { ch: b' ', fg, bg }
    }
}

pub fn init() {
    let font = Font::from_bytes(CONFIG.framebuffer.font.bytes).unwrap_or(FALLBACK_FONT);
    let framebuffer_cols = framebuffer::width() / font.width;
    let framebuffer_rows = framebuffer::height() / font.height;
    let configured_cols = CONFIG.framebuffer.columns;
    let configured_rows = CONFIG.framebuffer.rows;
    let cols = configured_limit(framebuffer_cols, configured_cols).min(MAX_COLS);
    let rows = configured_limit(framebuffer_rows, configured_rows).min(MAX_ROWS);
    let default_fg = config_color(CONFIG.framebuffer.foreground);
    let default_bg = config_color(CONFIG.framebuffer.background);

    let mut console = Console {
        font,
        cols,
        rows,
        cursor_x: 0,
        cursor_y: 0,
        cursor_visible: false,
        fg: default_fg,
        bg: default_bg,
        default_fg,
        default_bg,
        escape_state: EscapeState::new(),
    };

    console.clear();
    console.show_cursor();

    *CONSOLE.lock() = Some(console);
}

pub fn put_byte(byte: u8) {
    if let Some(console) = CONSOLE.lock().as_mut() {
        console.write_byte(byte);
    }
}

pub fn write_str(text: &str) {
    if let Some(console) = CONSOLE.lock().as_mut() {
        for byte in text.bytes() {
            console.write_byte(byte);
        }
    }
}

pub fn clear() {
    if let Some(console) = CONSOLE.lock().as_mut() {
        console.hide_cursor();
        console.clear();
        console.show_cursor();
    }
}

impl Console {
    fn write_byte(&mut self, byte: u8) {
        self.hide_cursor();

        match self.escape_state.feed(byte) {
            AnsiAction::None => {}
            AnsiAction::Put(byte) => self.write_plain_byte(byte),
            AnsiAction::ClearScreen => self.clear(),
            AnsiAction::ClearLine => self.clear_line_from_cursor(),
            AnsiAction::CursorHome => self.move_cursor(0, 0),
            AnsiAction::CursorUp(count) => {
                self.cursor_y = self.cursor_y.saturating_sub(count);
            }
            AnsiAction::CursorDown(count) => {
                self.cursor_y = self
                    .cursor_y
                    .saturating_add(count)
                    .min(self.rows.saturating_sub(1));
            }
            AnsiAction::CursorRight(count) => {
                self.cursor_x = self
                    .cursor_x
                    .saturating_add(count)
                    .min(self.cols.saturating_sub(1));
            }
            AnsiAction::CursorLeft(count) => {
                self.cursor_x = self.cursor_x.saturating_sub(count);
            }
            AnsiAction::Sgr { params, len } => self.apply_sgr(params, len),
        }

        self.show_cursor();
    }

    fn write_plain_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.newline(),
            b'\r' => self.cursor_x = 0,
            b'\x08' => self.backspace(),
            b'\t' => self.tab(),
            0x20..=0x7e => self.put_char(byte),
            _ => self.put_char(b'?'),
        }
    }

    fn put_char(&mut self, ch: u8) {
        if self.cols == 0 || self.rows == 0 {
            return;
        }

        if self.cursor_x >= self.cols {
            self.newline();
        }

        let cell = Cell {
            ch,
            fg: self.fg,
            bg: self.bg,
        };

        self.set_cell(self.cursor_x, self.cursor_y, cell);
        self.draw_cell(self.cursor_x, self.cursor_y, cell, false);

        self.cursor_x += 1;

        if self.cursor_x >= self.cols {
            self.newline();
        }
    }

    fn newline(&mut self) {
        self.cursor_x = 0;

        if self.cursor_y + 1 >= self.rows {
            self.scroll_up();
        } else {
            self.cursor_y += 1;
        }
    }

    fn backspace(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        }
    }

    fn tab(&mut self) {
        let next_tab = (self.cursor_x + TAB_WIDTH) & !(TAB_WIDTH - 1);

        while self.cursor_x < next_tab {
            self.put_char(b' ');
        }
    }

    fn clear(&mut self) {
        let blank = Cell::with_colors(self.fg, self.bg);

        for index in 0..self.cell_count() {
            self.write_cell_index(index, blank);
        }

        framebuffer::clear(self.bg);
        self.move_cursor(0, 0);
    }

    fn clear_line_from_cursor(&mut self) {
        let blank = Cell::with_colors(self.fg, self.bg);

        for x in self.cursor_x..self.cols {
            self.set_cell(x, self.cursor_y, blank);
            self.draw_cell(x, self.cursor_y, blank, false);
        }
    }

    fn scroll_up(&mut self) {
        if self.rows == 0 || self.cols == 0 {
            return;
        }

        let row_cells = self.cols;
        let visible_cells = self.cell_count();
        let blank = Cell::with_colors(self.fg, self.bg);

        unsafe {
            let base = cell_ptr();
            ptr::copy(base.add(row_cells), base, visible_cells - row_cells);

            for index in (visible_cells - row_cells)..visible_cells {
                base.add(index).write(blank);
            }
        }

        let copy_height = (self.rows - 1) * self.font.height;
        framebuffer::copy_rect(
            0,
            self.font.height,
            0,
            0,
            self.cols * self.font.width,
            copy_height,
        );
        framebuffer::fill_rect(
            0,
            copy_height,
            self.cols * self.font.width,
            self.font.height,
            self.bg,
        );
    }

    fn move_cursor(&mut self, x: usize, y: usize) {
        self.cursor_x = x.min(self.cols.saturating_sub(1));
        self.cursor_y = y.min(self.rows.saturating_sub(1));
    }

    fn show_cursor(&mut self) {
        if self.cols == 0 || self.rows == 0 || self.cursor_visible {
            return;
        }

        let cell = self.cell(self.cursor_x, self.cursor_y);
        self.draw_cell(self.cursor_x, self.cursor_y, cell, true);
        self.cursor_visible = true;
    }

    fn hide_cursor(&mut self) {
        if self.cols == 0 || self.rows == 0 || !self.cursor_visible {
            return;
        }

        let cell = self.cell(self.cursor_x, self.cursor_y);
        self.draw_cell(self.cursor_x, self.cursor_y, cell, false);
        self.cursor_visible = false;
    }

    fn draw_cell(&self, cell_x: usize, cell_y: usize, cell: Cell, inverted: bool) {
        let x = cell_x * self.font.width;
        let y = cell_y * self.font.height;
        let fg = if inverted { cell.bg } else { cell.fg };
        let bg = if inverted { cell.fg } else { cell.bg };

        framebuffer::draw_glyph(
            x,
            y,
            self.font.width,
            self.font.height,
            |row, col| self.font.has_pixel(cell.ch, row, col),
            fg,
            bg,
        );
    }

    fn apply_sgr(&mut self, params: [u16; 4], len: usize) {
        if len == 0 {
            self.reset_colors();
            return;
        }

        for param in params.iter().take(len) {
            match *param {
                0 => self.reset_colors(),
                30..=37 => self.fg = ansi_color(*param - 30),
                40..=47 => self.bg = ansi_color(*param - 40),
                _ => {}
            }
        }
    }

    fn reset_colors(&mut self) {
        self.fg = self.default_fg;
        self.bg = self.default_bg;
    }

    fn cell(&self, x: usize, y: usize) -> Cell {
        unsafe { cell_ptr().add(self.index(x, y)).read() }
    }

    fn set_cell(&mut self, x: usize, y: usize, cell: Cell) {
        self.write_cell_index(self.index(x, y), cell);
    }

    fn write_cell_index(&mut self, index: usize, cell: Cell) {
        unsafe {
            cell_ptr().add(index).write(cell);
        }
    }

    fn index(&self, x: usize, y: usize) -> usize {
        y * self.cols + x
    }

    fn cell_count(&self) -> usize {
        self.cols * self.rows
    }
}

fn ansi_color(index: u16) -> Color {
    match index {
        0 => Color::rgb(0, 0, 0),
        1 => Color::rgb(170, 0, 0),
        2 => Color::rgb(0, 170, 0),
        3 => Color::rgb(170, 85, 0),
        4 => Color::rgb(0, 0, 170),
        5 => Color::rgb(170, 0, 170),
        6 => Color::rgb(0, 170, 170),
        _ => Color::rgb(170, 170, 170),
    }
}

fn configured_limit(actual: usize, configured: usize) -> usize {
    if configured == 0 {
        actual
    } else {
        actual.min(configured)
    }
}

fn config_color(color: rinasys_config_schema::FramebufferColorConfig) -> Color {
    Color::rgb(color.r, color.g, color.b)
}

fn cell_ptr() -> *mut Cell {
    core::ptr::addr_of_mut!(CELLS).cast::<Cell>()
}
