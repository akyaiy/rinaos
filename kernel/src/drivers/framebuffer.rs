use core::ptr;

use crate::boot::Framebuffer as FramebufferInfo;
use crate::sync::spinlock::IrqSpinLock;

static FRAMEBUFFER: IrqSpinLock<Option<Framebuffer>> = IrqSpinLock::new(None);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const RED: Self = Self::rgb(255, 0, 0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

pub struct Framebuffer {
    addr: usize,
    width: usize,
    height: usize,
    pitch: usize,
    bpp: usize,
    bytes_per_pixel: usize,

    red_shift: u8,
    green_shift: u8,
    blue_shift: u8,
}

pub fn init(info: FramebufferInfo) {
    let bytes_per_pixel = info.bpp / 8;

    *FRAMEBUFFER.lock() = Some(Framebuffer {
        addr: info.addr,
        width: info.width,
        height: info.height,
        pitch: info.pitch,
        bpp: info.bpp,
        bytes_per_pixel,
        red_shift: info.red_shift,
        green_shift: info.green_shift,
        blue_shift: info.blue_shift,
    });

    clear(Color::BLACK);
}

pub fn width() -> usize {
    FRAMEBUFFER.lock().as_ref().map_or(0, |fb| fb.width)
}

pub fn height() -> usize {
    FRAMEBUFFER.lock().as_ref().map_or(0, |fb| fb.height)
}

pub fn pitch() -> usize {
    FRAMEBUFFER.lock().as_ref().map_or(0, |fb| fb.pitch)
}

pub fn bpp() -> usize {
    FRAMEBUFFER.lock().as_ref().map_or(0, |fb| fb.bpp)
}

pub fn put_pixel(x: usize, y: usize, color: Color) {
    if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
        unsafe {
            fb.put_pixel(x, y, color);
        }
    }
}

pub fn fill_rect(x: usize, y: usize, width: usize, height: usize, color: Color) {
    if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
        unsafe {
            fb.fill_rect(x, y, width, height, color);
        }
    }
}

pub fn copy_rect(
    src_x: usize,
    src_y: usize,
    dst_x: usize,
    dst_y: usize,
    width: usize,
    height: usize,
) {
    if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
        unsafe {
            fb.copy_rect(src_x, src_y, dst_x, dst_y, width, height);
        }
    }
}

pub fn draw_glyph<F>(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    mut has_pixel: F,
    fg: Color,
    bg: Color,
) where
    F: FnMut(usize, usize) -> bool,
{
    if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
        unsafe {
            for row in 0..height {
                for col in 0..width {
                    let color = if has_pixel(row, col) { fg } else { bg };
                    fb.put_pixel(x + col, y + row, color);
                }
            }
        }
    }
}

pub fn clear(color: Color) {
    if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
        unsafe {
            fb.fill_rect(0, 0, fb.width, fb.height, color);
        }
    }
}

pub fn flush_region(_x: usize, _y: usize, _width: usize, _height: usize) {}

impl Framebuffer {
    fn pack_color(&self, color: Color) -> u32 {
        ((color.r as u32) << self.red_shift)
            | ((color.g as u32) << self.green_shift)
            | ((color.b as u32) << self.blue_shift)
    }

    unsafe fn put_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }

        let offset = y * self.pitch + x * self.bytes_per_pixel;
        self.write_color(offset, color);
    }

    unsafe fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: Color) {
        let end_x = x.saturating_add(width).min(self.width);
        let end_y = y.saturating_add(height).min(self.height);

        if x >= end_x || y >= end_y {
            return;
        }

        match self.bpp {
            32 => {
                let packed = self.pack_color(color);

                for row in y..end_y {
                    let row_ptr = (self.addr + row * self.pitch + x * 4) as *mut u32;

                    for col in 0..(end_x - x) {
                        row_ptr.add(col).write_volatile(packed);
                    }
                }
            }
            24 => {
                for row in y..end_y {
                    for col in x..end_x {
                        self.put_pixel(col, row, color);
                    }
                }
            }
            _ => {}
        }
    }

    unsafe fn copy_rect(
        &mut self,
        src_x: usize,
        src_y: usize,
        dst_x: usize,
        dst_y: usize,
        width: usize,
        height: usize,
    ) {
        let width = width
            .min(self.width.saturating_sub(src_x))
            .min(self.width.saturating_sub(dst_x));
        let height = height
            .min(self.height.saturating_sub(src_y))
            .min(self.height.saturating_sub(dst_y));

        if width == 0 || height == 0 {
            return;
        }

        let bytes = width * self.bytes_per_pixel;

        if dst_y > src_y {
            for row in (0..height).rev() {
                self.copy_row(src_x, src_y + row, dst_x, dst_y + row, bytes);
            }
        } else {
            for row in 0..height {
                self.copy_row(src_x, src_y + row, dst_x, dst_y + row, bytes);
            }
        }
    }

    unsafe fn copy_row(
        &mut self,
        src_x: usize,
        src_y: usize,
        dst_x: usize,
        dst_y: usize,
        bytes: usize,
    ) {
        let src = (self.addr + src_y * self.pitch + src_x * self.bytes_per_pixel) as *const u8;
        let dst = (self.addr + dst_y * self.pitch + dst_x * self.bytes_per_pixel) as *mut u8;

        ptr::copy(src, dst, bytes);
    }

    unsafe fn write_color(&mut self, offset: usize, color: Color) {
        let packed = self.pack_color(color);

        match self.bpp {
            32 => {
                let ptr = (self.addr + offset) as *mut u32;
                ptr.write_volatile(packed);
            }
            24 => {
                let ptr = (self.addr + offset) as *mut u8;
                ptr.add(0).write_volatile((packed & 0xff) as u8);
                ptr.add(1).write_volatile(((packed >> 8) & 0xff) as u8);
                ptr.add(2).write_volatile(((packed >> 16) & 0xff) as u8);
            }
            _ => {}
        }
    }
}
