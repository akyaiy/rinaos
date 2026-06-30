use crate::sync::spinlock::SpinLock;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::mapper::{MapToError, Mapper, OffsetPageTable, Translate};
use x86_64::structures::paging::page_table::PageTableFlags;
use x86_64::structures::paging::{
    FrameAllocator, FrameDeallocator, Page, PageTable, PhysFrame as X86PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr as X86PhysAddr, VirtAddr};

use super::{free_frames, PhysAddr, PhysFrame};

static PAGING: SpinLock<PagingState> = SpinLock::new(PagingState::new());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PagingError {
    NotInitialized,
    InvalidRange,
    OutOfMemory,
    AlreadyMapped,
    MapFailed,
}

struct PagingState {
    hhdm_offset: u64,
    initialized: bool,
}

pub struct BuddyFrameAllocator;

impl PagingState {
    const fn new() -> Self {
        Self {
            hhdm_offset: 0,
            initialized: false,
        }
    }
}

pub fn init(hhdm_offset: u64) {
    let mut state = PAGING.lock();
    state.hhdm_offset = hhdm_offset;
    state.initialized = true;
}

pub fn translate_addr(addr: u64) -> Option<u64> {
    with_mapper(|mapper| {
        mapper
            .translate_addr(VirtAddr::new(addr))
            .map(|addr| addr.as_u64())
    })
    .ok()
    .flatten()
}

pub fn map_allocated_range(
    start_addr: u64,
    byte_len: u64,
    flags: PageTableFlags,
) -> Result<(), PagingError> {
    if byte_len == 0 {
        return Ok(());
    }

    let end_addr = start_addr
        .checked_add(byte_len - 1)
        .ok_or(PagingError::InvalidRange)?;

    with_mapper(|mapper| {
        let start_page = Page::<Size4KiB>::containing_address(VirtAddr::new(start_addr));
        let end_page = Page::<Size4KiB>::containing_address(VirtAddr::new(end_addr));
        let mut frame_allocator = BuddyFrameAllocator;

        for page in Page::range_inclusive(start_page, end_page) {
            if mapper.translate_addr(page.start_address()).is_some() {
                return Err(PagingError::AlreadyMapped);
            }

            let frame = frame_allocator
                .allocate_frame()
                .ok_or(PagingError::OutOfMemory)?;

            match unsafe { mapper.map_to(page, frame, flags, &mut frame_allocator) } {
                Ok(flush) => flush.flush(),
                Err(error) => {
                    unsafe {
                        frame_allocator.deallocate_frame(frame);
                    }

                    return Err(map_to_error(error));
                }
            }
        }

        Ok(())
    })?
}

/// # Safety
///
/// The caller must ensure that `frame` is not already aliased by an active
/// mutable mapping and that `addr` points to virtual memory owned by the kernel.
pub unsafe fn map_frame(
    addr: u64,
    frame: PhysFrame,
    flags: PageTableFlags,
) -> Result<(), PagingError> {
    with_mapper(|mapper| {
        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(addr));
        if mapper.translate_addr(page.start_address()).is_some() {
            return Err(PagingError::AlreadyMapped);
        }

        let x86_frame = x86_frame(frame);
        let mut frame_allocator = BuddyFrameAllocator;

        match mapper.map_to(page, x86_frame, flags, &mut frame_allocator) {
            Ok(flush) => {
                flush.flush();
                Ok(())
            }
            Err(error) => Err(map_to_error(error)),
        }
    })?
}

pub const fn kernel_data_flags() -> PageTableFlags {
    PageTableFlags::PRESENT
        .union(PageTableFlags::WRITABLE)
        .union(PageTableFlags::GLOBAL)
}

fn with_mapper<T>(f: impl FnOnce(&mut OffsetPageTable) -> T) -> Result<T, PagingError> {
    let state = PAGING.lock();
    if !state.initialized {
        return Err(PagingError::NotInitialized);
    }

    let mut mapper = unsafe { active_mapper(state.hhdm_offset) };
    Ok(f(&mut mapper))
}

unsafe fn active_mapper(hhdm_offset: u64) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(hhdm_offset);
    OffsetPageTable::new(level_4_table, VirtAddr::new(hhdm_offset))
}

unsafe fn active_level_4_table(hhdm_offset: u64) -> &'static mut PageTable {
    let (level_4_frame, _) = Cr3::read();
    let phys = level_4_frame.start_address();
    let virt = VirtAddr::new(hhdm_offset + phys.as_u64());
    &mut *virt.as_mut_ptr()
}

fn x86_frame(frame: PhysFrame) -> X86PhysFrame<Size4KiB> {
    X86PhysFrame::containing_address(X86PhysAddr::new(frame.start_address().as_u64()))
}

fn buddy_frame(frame: X86PhysFrame<Size4KiB>) -> PhysFrame {
    PhysFrame::from_start_address(PhysAddr::new(frame.start_address().as_u64())).unwrap()
}

fn map_to_error(error: MapToError<Size4KiB>) -> PagingError {
    match error {
        MapToError::FrameAllocationFailed => PagingError::OutOfMemory,
        MapToError::ParentEntryHugePage => PagingError::MapFailed,
        MapToError::PageAlreadyMapped(_) => PagingError::AlreadyMapped,
    }
}

unsafe impl FrameAllocator<Size4KiB> for BuddyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<X86PhysFrame<Size4KiB>> {
        super::alloc_frame().map(x86_frame)
    }
}

impl FrameDeallocator<Size4KiB> for BuddyFrameAllocator {
    unsafe fn deallocate_frame(&mut self, frame: X86PhysFrame<Size4KiB>) {
        free_frames(buddy_frame(frame), 0);
    }
}
