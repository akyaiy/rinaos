use crate::boot::Framebuffer as FramebufferInfo;

static mut FRAMEBUFFER: Option<Framebuffer> = None;

pub struct Framebuffer {
    addr: usize,
    width: usize,
    height: usize,
    pitch: usize,
    bpp: usize,

    red_shift: u8,
    green_shift: u8,
    blue_shift: u8,
}

pub fn init(info: FramebufferInfo) {
    unsafe {
        FRAMEBUFFER = Some(Framebuffer {
            addr: info.addr,
            width: info.width,
            height: info.height,
            pitch: info.pitch,
            bpp: info.bpp,
            red_shift: info.red_shift,
            green_shift: info.green_shift,
            blue_shift: info.blue_shift,
        });
    }

    clear(0, 0, 0);
}

impl Framebuffer {
    fn pack_color(&self, r: u8, g: u8, b: u8) -> u32 {
        ((r as u32) << self.red_shift)
            | ((g as u32) << self.green_shift)
            | ((b as u32) << self.blue_shift)
    }

    unsafe fn put_pixel(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8) {
        if x >= self.width || y >= self.height {
            return;
        }

        let bytes_per_pixel = self.bpp / 8;
        let offset = y * self.pitch + x * bytes_per_pixel;
        let color = self.pack_color(r, g, b);

        match self.bpp {
            32 => {
                let ptr = (self.addr + offset) as *mut u32;
                ptr.write_volatile(color);
            }
            24 => {
                let ptr = (self.addr + offset) as *mut u8;
                ptr.add(0).write_volatile((color & 0xff) as u8);
                ptr.add(1).write_volatile(((color >> 8) & 0xff) as u8);
                ptr.add(2).write_volatile(((color >> 16) & 0xff) as u8);
            }
            _ => {}
        }
    }
}

pub fn put_pixel(x: usize, y: usize, r: u8, g: u8, b: u8) {
    unsafe {
        if let Some(fb) = FRAMEBUFFER.as_mut() {
            fb.put_pixel(x, y, r, g, b);
        }
    }
}

pub fn clear(r: u8, g: u8, b: u8) {
    unsafe {
        if let Some(fb) = FRAMEBUFFER.as_mut() {
            for y in 0..fb.height {
                for x in 0..fb.width {
                    fb.put_pixel(x, y, r, g, b);
                }
            }
        }
    }
}
