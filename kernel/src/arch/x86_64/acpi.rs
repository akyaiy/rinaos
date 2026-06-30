use core::{mem, ptr};

const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";
const XSDT_SIGNATURE: &[u8; 4] = b"XSDT";
const RSDT_SIGNATURE: &[u8; 4] = b"RSDT";
const MADT_SIGNATURE: &[u8; 4] = b"APIC";

const MAX_IO_APICS: usize = 8;
const MAX_INTERRUPT_OVERRIDES: usize = 32;

#[derive(Clone, Copy)]
pub struct Madt {
    pub local_apic_addr: u64,
    pub io_apics: [IoApic; MAX_IO_APICS],
    pub io_apic_count: usize,
    pub interrupt_overrides: [InterruptOverride; MAX_INTERRUPT_OVERRIDES],
    pub interrupt_override_count: usize,
}

#[derive(Clone, Copy)]
pub struct IoApic {
    pub id: u8,
    pub address: u32,
    pub gsi_base: u32,
}

#[derive(Clone, Copy)]
pub struct InterruptOverride {
    pub source_irq: u8,
    pub gsi: u32,
    pub flags: u16,
}

impl Madt {
    const fn empty() -> Self {
        Self {
            local_apic_addr: 0,
            io_apics: [IoApic {
                id: 0,
                address: 0,
                gsi_base: 0,
            }; MAX_IO_APICS],
            io_apic_count: 0,
            interrupt_overrides: [InterruptOverride {
                source_irq: 0,
                gsi: 0,
                flags: 0,
            }; MAX_INTERRUPT_OVERRIDES],
            interrupt_override_count: 0,
        }
    }

    pub fn override_for_irq(&self, irq: u8) -> Option<InterruptOverride> {
        self.interrupt_overrides[..self.interrupt_override_count]
            .iter()
            .copied()
            .find(|entry| entry.source_irq == irq)
    }
}

#[repr(C, packed)]
struct RsdpV1 {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_address: u32,
}

#[repr(C, packed)]
struct RsdpV2 {
    v1: RsdpV1,
    length: u32,
    xsdt_address: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

#[repr(C, packed)]
struct SdtHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

#[repr(C, packed)]
struct MadtHeader {
    header: SdtHeader,
    local_apic_addr: u32,
    flags: u32,
}

pub unsafe fn find_madt(rsdp_addr: u64, hhdm_offset: u64) -> Option<Madt> {
    let rsdp = direct_addr(rsdp_addr, hhdm_offset) as *const RsdpV2;
    let rsdp_v1 = &(*rsdp).v1;

    if &rsdp_v1.signature != RSDP_SIGNATURE {
        return None;
    }

    if !checksum(rsdp.cast(), mem::size_of::<RsdpV1>()) {
        return None;
    }

    let revision = ptr::read(ptr::addr_of!(rsdp_v1.revision));
    if revision >= 2 {
        let length = read_unaligned(ptr::addr_of!((*rsdp).length)) as usize;
        if length >= mem::size_of::<RsdpV2>() && checksum(rsdp.cast(), length) {
            let xsdt_addr = read_unaligned(ptr::addr_of!((*rsdp).xsdt_address));
            if xsdt_addr != 0 {
                if let Some(madt) = find_in_xsdt(xsdt_addr, hhdm_offset) {
                    return Some(madt);
                }
            }
        }
    }

    let rsdt_addr = read_unaligned(ptr::addr_of!(rsdp_v1.rsdt_address));
    if rsdt_addr != 0 {
        find_in_rsdt(rsdt_addr as u64, hhdm_offset)
    } else {
        None
    }
}

unsafe fn find_in_xsdt(xsdt_phys: u64, hhdm_offset: u64) -> Option<Madt> {
    let xsdt = phys_addr(xsdt_phys, hhdm_offset) as *const SdtHeader;
    if !valid_sdt(xsdt, XSDT_SIGNATURE) {
        return None;
    }

    let length = read_unaligned(ptr::addr_of!((*xsdt).length)) as usize;
    let entries = (length - mem::size_of::<SdtHeader>()) / mem::size_of::<u64>();
    let entry_base = (xsdt as usize + mem::size_of::<SdtHeader>()) as *const u64;

    for index in 0..entries {
        let table_phys = ptr::read_unaligned(entry_base.add(index));
        if let Some(madt) = table_if_madt(table_phys, hhdm_offset) {
            return Some(madt);
        }
    }

    None
}

unsafe fn find_in_rsdt(rsdt_phys: u64, hhdm_offset: u64) -> Option<Madt> {
    let rsdt = phys_addr(rsdt_phys, hhdm_offset) as *const SdtHeader;
    if !valid_sdt(rsdt, RSDT_SIGNATURE) {
        return None;
    }

    let length = read_unaligned(ptr::addr_of!((*rsdt).length)) as usize;
    let entries = (length - mem::size_of::<SdtHeader>()) / mem::size_of::<u32>();
    let entry_base = (rsdt as usize + mem::size_of::<SdtHeader>()) as *const u32;

    for index in 0..entries {
        let table_phys = ptr::read_unaligned(entry_base.add(index)) as u64;
        if let Some(madt) = table_if_madt(table_phys, hhdm_offset) {
            return Some(madt);
        }
    }

    None
}

unsafe fn table_if_madt(table_phys: u64, hhdm_offset: u64) -> Option<Madt> {
    let table = phys_addr(table_phys, hhdm_offset) as *const SdtHeader;
    if valid_sdt(table, MADT_SIGNATURE) {
        Some(parse_madt(table.cast()))
    } else {
        None
    }
}

unsafe fn parse_madt(madt_ptr: *const MadtHeader) -> Madt {
    let mut madt = Madt::empty();
    madt.local_apic_addr = read_unaligned(ptr::addr_of!((*madt_ptr).local_apic_addr)) as u64;

    let length = read_unaligned(ptr::addr_of!((*madt_ptr).header.length)) as usize;
    let mut offset = mem::size_of::<MadtHeader>();

    while offset + 2 <= length {
        let entry = (madt_ptr as usize + offset) as *const u8;
        let entry_type = ptr::read(entry);
        let entry_len = ptr::read(entry.add(1)) as usize;

        if entry_len < 2 || offset + entry_len > length {
            break;
        }

        match entry_type {
            1 if entry_len >= 12 => {
                if madt.io_apic_count < MAX_IO_APICS {
                    madt.io_apics[madt.io_apic_count] = IoApic {
                        id: ptr::read(entry.add(2)),
                        address: ptr::read_unaligned(entry.add(4).cast::<u32>()),
                        gsi_base: ptr::read_unaligned(entry.add(8).cast::<u32>()),
                    };
                    madt.io_apic_count += 1;
                }
            }
            2 if entry_len >= 10 => {
                if madt.interrupt_override_count < MAX_INTERRUPT_OVERRIDES {
                    madt.interrupt_overrides[madt.interrupt_override_count] = InterruptOverride {
                        source_irq: ptr::read(entry.add(3)),
                        gsi: ptr::read_unaligned(entry.add(4).cast::<u32>()),
                        flags: ptr::read_unaligned(entry.add(8).cast::<u16>()),
                    };
                    madt.interrupt_override_count += 1;
                }
            }
            5 if entry_len >= 12 => {
                madt.local_apic_addr = ptr::read_unaligned(entry.add(4).cast::<u64>());
            }
            _ => {}
        }

        offset += entry_len;
    }

    madt
}

unsafe fn valid_sdt(table: *const SdtHeader, signature: &[u8; 4]) -> bool {
    let length = read_unaligned(ptr::addr_of!((*table).length)) as usize;
    &(*table).signature == signature
        && length >= mem::size_of::<SdtHeader>()
        && checksum(table.cast(), length)
}

unsafe fn checksum(ptr: *const u8, len: usize) -> bool {
    let mut sum = 0u8;

    for index in 0..len {
        sum = sum.wrapping_add(ptr::read(ptr.add(index)));
    }

    sum == 0
}

fn phys_addr(phys: u64, hhdm_offset: u64) -> usize {
    phys.saturating_add(hhdm_offset) as usize
}

fn direct_addr(addr: u64, hhdm_offset: u64) -> usize {
    if addr >= hhdm_offset {
        addr as usize
    } else {
        phys_addr(addr, hhdm_offset)
    }
}

unsafe fn read_unaligned<T: Copy>(value: *const T) -> T {
    ptr::read_unaligned(value)
}
