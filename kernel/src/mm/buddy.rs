use crate::boot::{MemoryRegion, MemoryRegionKind};
use crate::klog_warn;

pub const PAGE_SIZE: u64 = 4096;
const MAX_ORDER: usize = 20;
const NO_ADDR: u64 = u64::MAX;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysAddr(u64);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PhysFrame {
    start: PhysAddr,
}

#[derive(Clone, Copy)]
pub struct BuddyStats {
    pub free_frames: usize,
    pub free_bytes: u64,
    pub largest_free_order: usize,
    pub usable_regions: usize,
}

#[repr(C)]
struct FreeBlock {
    next: u64,
}

#[derive(Clone, Copy)]
struct FreeList {
    head: u64,
}

pub struct BuddyAllocator {
    free_lists: [FreeList; MAX_ORDER + 1],
    hhdm_offset: u64,
    free_frames: usize,
    usable_regions: usize,
    initialized: bool,
}

impl PhysAddr {
    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl PhysFrame {
    pub const fn from_start_address(addr: PhysAddr) -> Option<Self> {
        if addr.0 & (PAGE_SIZE - 1) == 0 {
            Some(Self { start: addr })
        } else {
            None
        }
    }

    pub const fn containing_address(addr: PhysAddr) -> Self {
        Self {
            start: PhysAddr(addr.0 & !(PAGE_SIZE - 1)),
        }
    }

    pub const fn start_address(self) -> PhysAddr {
        self.start
    }
}

impl BuddyStats {
    const fn empty() -> Self {
        Self {
            free_frames: 0,
            free_bytes: 0,
            largest_free_order: 0,
            usable_regions: 0,
        }
    }
}

impl FreeList {
    const fn new() -> Self {
        Self { head: NO_ADDR }
    }
}

impl BuddyAllocator {
    pub const fn new() -> Self {
        Self {
            free_lists: [FreeList::new(); MAX_ORDER + 1],
            hhdm_offset: 0,
            free_frames: 0,
            usable_regions: 0,
            initialized: false,
        }
    }

    /// # Safety
    ///
    /// The Limine HHDM must map every usable physical page. This function writes
    /// free-list links into pages marked usable by the bootloader.
    pub unsafe fn init(&mut self, memory_map: &[MemoryRegion], hhdm_offset: u64) {
        *self = Self::new();
        self.hhdm_offset = hhdm_offset;
        self.initialized = true;

        for region in memory_map {
            if matches!(region.kind, MemoryRegionKind::Usable) {
                self.usable_regions += 1;
                self.add_range(region.base, region.base.saturating_add(region.length));
            }
        }
    }

    pub fn alloc(&mut self, order: usize) -> Option<PhysFrame> {
        if !self.initialized || order > MAX_ORDER {
            return None;
        }

        let mut current_order = order;
        while current_order <= MAX_ORDER && self.free_lists[current_order].head == NO_ADDR {
            current_order += 1;
        }

        if current_order > MAX_ORDER {
            return None;
        }

        let addr = unsafe { self.pop(current_order) };

        while current_order > order {
            current_order -= 1;
            let buddy = addr + block_size(current_order);
            unsafe {
                self.push(buddy, current_order);
            }
        }

        self.free_frames -= frames_for_order(order);
        Some(PhysFrame {
            start: PhysAddr(addr),
        })
    }

    /// # Safety
    ///
    /// The caller must return a block allocated from this allocator with the
    /// same order, and must not use it after freeing.
    pub unsafe fn free(&mut self, frame: PhysFrame, mut order: usize) {
        if !self.initialized || order > MAX_ORDER {
            return;
        }

        let mut addr = frame.start_address().as_u64();
        self.free_frames += frames_for_order(order);

        while order < MAX_ORDER {
            let buddy = addr ^ block_size(order);
            if !self.remove(buddy, order) {
                break;
            }

            addr = addr.min(buddy);
            order += 1;
        }

        self.push(addr, order);
    }

    pub fn stats(&self) -> BuddyStats {
        if !self.initialized {
            return BuddyStats::empty();
        }

        let mut largest_free_order = 0;
        for order in (0..=MAX_ORDER).rev() {
            if self.free_lists[order].head != NO_ADDR {
                largest_free_order = order;
                break;
            }
        }

        BuddyStats {
            free_frames: self.free_frames,
            free_bytes: (self.free_frames as u64) * PAGE_SIZE,
            largest_free_order,
            usable_regions: self.usable_regions,
        }
    }

    unsafe fn add_range(&mut self, start: u64, end: u64) {
        let mut current = align_up(start, PAGE_SIZE);
        let end = align_down(end, PAGE_SIZE);

        while current < end {
            let remaining = end - current;
            let order = largest_order_for(current, remaining);

            if order > MAX_ORDER {
                klog_warn!("memory: skipped oversized buddy block at {:#x}", current);
                break;
            }

            self.push(current, order);
            self.free_frames += frames_for_order(order);
            current += block_size(order);
        }
    }

    unsafe fn push(&mut self, addr: u64, order: usize) {
        let node = self.node_mut(addr);
        (*node).next = self.free_lists[order].head;
        self.free_lists[order].head = addr;
    }

    unsafe fn pop(&mut self, order: usize) -> u64 {
        let addr = self.free_lists[order].head;
        debug_assert!(addr != NO_ADDR);

        let node = self.node_mut(addr);
        self.free_lists[order].head = (*node).next;
        addr
    }

    unsafe fn remove(&mut self, addr: u64, order: usize) -> bool {
        let mut previous = NO_ADDR;
        let mut current = self.free_lists[order].head;

        while current != NO_ADDR {
            let current_node = self.node_mut(current);
            let next = (*current_node).next;

            if current == addr {
                if previous == NO_ADDR {
                    self.free_lists[order].head = next;
                } else {
                    (*self.node_mut(previous)).next = next;
                }

                return true;
            }

            previous = current;
            current = next;
        }

        false
    }

    unsafe fn node_mut(&self, addr: u64) -> *mut FreeBlock {
        (self.hhdm_offset + addr) as *mut FreeBlock
    }
}

const fn block_size(order: usize) -> u64 {
    PAGE_SIZE << order
}

const fn frames_for_order(order: usize) -> usize {
    1usize << order
}

const fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

const fn align_up(value: u64, align: u64) -> u64 {
    align_down(value.saturating_add(align - 1), align)
}

fn largest_order_for(addr: u64, len: u64) -> usize {
    let mut order = MAX_ORDER;

    loop {
        let size = block_size(order);
        if len >= size && addr % size == 0 {
            return order;
        }

        if order == 0 {
            return 0;
        }

        order -= 1;
    }
}
