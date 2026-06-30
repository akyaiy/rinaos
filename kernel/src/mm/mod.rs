mod buddy;

use crate::boot::BootInfo;
use crate::sync::spinlock::SpinLock;
use crate::{klog_debug, klog_info};

pub use buddy::{BuddyStats, PhysFrame};

pub const PAGE_SIZE: u64 = buddy::PAGE_SIZE;

static FRAME_ALLOCATOR: SpinLock<buddy::BuddyAllocator> =
    SpinLock::new(buddy::BuddyAllocator::new());

pub fn init(boot_info: &BootInfo) {
    let mut allocator = FRAME_ALLOCATOR.lock();

    unsafe {
        allocator.init(boot_info.memory_map, boot_info.hhdm_offset);
    }

    let stats = allocator.stats();
    klog_info!(
        "memory: initialized physical buddy allocator, {} KiB free in {} frames",
        stats.free_bytes / 1024,
        stats.free_frames
    );
    klog_debug!(
        "memory: largest free order {}, usable regions {}",
        stats.largest_free_order,
        stats.usable_regions
    );
}

pub fn alloc_frame() -> Option<PhysFrame> {
    alloc_frames(0)
}

pub fn alloc_frames(order: usize) -> Option<PhysFrame> {
    FRAME_ALLOCATOR.lock().alloc(order)
}

/// # Safety
///
/// The caller must pass a frame and order returned by this allocator exactly
/// once, and must ensure the memory is no longer mapped or used elsewhere.
pub unsafe fn free_frames(frame: PhysFrame, order: usize) {
    FRAME_ALLOCATOR.lock().free(frame, order);
}

pub fn stats() -> BuddyStats {
    FRAME_ALLOCATOR.lock().stats()
}
