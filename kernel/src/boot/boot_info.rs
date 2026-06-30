#[derive(Clone, Copy)]
pub struct BootInfo {
    pub framebuffer: Option<Framebuffer>,
    pub memory_map: &'static [MemoryRegion],
    pub hhdm_offset: u64,
    pub rsdp_addr: Option<u64>,
}

#[derive(Clone, Copy)]
pub struct Framebuffer {
    pub addr: usize, // as *mut u32
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub bpp: usize,

    pub red_shift: u8,
    pub green_shift: u8,
    pub blue_shift: u8,
}

#[derive(Clone, Copy)]
pub struct MemoryRegion {
    pub base: u64,
    pub length: u64,
    pub kind: MemoryRegionKind,
}

#[derive(Clone, Copy)]
pub enum MemoryRegionKind {
    Usable,
    Reserved,
    Kernel,
    Framebuffer,
    BootloaderReclaimable,
    Unknown,
}

impl MemoryRegion {
    pub const fn empty() -> Self {
        Self {
            base: 0,
            length: 0,
            kind: MemoryRegionKind::Unknown,
        }
    }
}

const MAX_MEMORY_REGIONS: usize = 128;

static mut MEMORY_REGIONS: [MemoryRegion; MAX_MEMORY_REGIONS] =
    [MemoryRegion::empty(); MAX_MEMORY_REGIONS];

static mut MEMORY_REGION_COUNT: usize = 0;

pub(super) fn convert_memory_map(
    entries: &'static [&'static limine::memmap::Entry],
) -> &'static [MemoryRegion] {
    unsafe {
        MEMORY_REGION_COUNT = 0;

        for entry in entries.iter() {
            if MEMORY_REGION_COUNT >= MAX_MEMORY_REGIONS {
                break;
            }

            MEMORY_REGIONS[MEMORY_REGION_COUNT] = MemoryRegion {
                base: entry.base,
                length: entry.length,
                kind: convert_memory_kind(entry.type_),
            };

            MEMORY_REGION_COUNT += 1;
        }

        &MEMORY_REGIONS[..MEMORY_REGION_COUNT]
    }
}

fn convert_memory_kind(kind: u64) -> MemoryRegionKind {
    match kind {
        limine::memmap::MEMMAP_USABLE => MemoryRegionKind::Usable,
        limine::memmap::MEMMAP_BOOTLOADER_RECLAIMABLE => MemoryRegionKind::BootloaderReclaimable,
        limine::memmap::MEMMAP_EXECUTABLE_AND_MODULES => MemoryRegionKind::Kernel,
        limine::memmap::MEMMAP_FRAMEBUFFER => MemoryRegionKind::Framebuffer,
        _ => MemoryRegionKind::Reserved,
    }
}
