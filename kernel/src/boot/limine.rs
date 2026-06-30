use limine::request::{FramebufferRequest, HhdmRequest, MemmapRequest};

use crate::boot::boot_info::{convert_memory_map, BootInfo, Framebuffer};

#[used]
#[link_section = ".requests"]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[link_section = ".requests"]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[used]
#[link_section = ".requests"]
static MEMORY_MAP_REQUEST: MemmapRequest = MemmapRequest::new();

pub fn init() -> BootInfo {
    let framebuffer = FRAMEBUFFER_REQUEST.response().and_then(|response| {
        response.framebuffers().first().map(|fb| Framebuffer {
            addr: fb.address() as usize,
            width: fb.width as usize,
            height: fb.height as usize,
            pitch: fb.pitch as usize,
            bpp: fb.bpp as usize,

            red_shift: fb.red_mask_shift,
            green_shift: fb.green_mask_shift,
            blue_shift: fb.blue_mask_shift,
        })
    });

    let hhdm_offset = HHDM_REQUEST
        .response()
        .expect("Limine did not provide HHDM")
        .offset as u64;

    let memory_map = MEMORY_MAP_REQUEST
        .response()
        .expect("Limine did not provide memory map")
        .entries();

    // TODO: non permanent solution
    let memory_regions = convert_memory_map(memory_map);

    BootInfo {
        framebuffer,
        memory_map: memory_regions,
        hhdm_offset,
    }
}
