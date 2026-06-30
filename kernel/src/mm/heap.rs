use core::alloc::{GlobalAlloc, Layout};
use core::mem;
use core::ptr::null_mut;

use crate::sync::spinlock::SpinLock;

use super::{paging, PAGE_SIZE};

pub const KERNEL_HEAP_START: u64 = 0xffff_9000_0000_0000;
pub const KERNEL_HEAP_SIZE: usize = 64 * 1024 * 1024;

const SLAB_SIZE: usize = PAGE_SIZE as usize;
const SLAB_CLASS_COUNT: usize = 9;
const LARGE_ALLOC_THRESHOLD: usize = 2048;

#[global_allocator]
static ALLOCATOR: LockedSlabAllocator = LockedSlabAllocator::new();

#[repr(C)]
struct FreeNode {
    next: *mut FreeNode,
}

struct LockedSlabAllocator {
    inner: SpinLock<SlabAllocator>,
}

struct SlabAllocator {
    heap_start: usize,
    heap_end: usize,
    next_page: usize,
    mapped_bytes: usize,
    classes: [SlabClass; SLAB_CLASS_COUNT],
    large_allocations: usize,
    initialized: bool,
}

#[derive(Clone, Copy)]
struct SlabClass {
    block_size: usize,
    free_list: *mut FreeNode,
    slab_count: usize,
    allocations: usize,
}

pub struct HeapStats {
    pub start: usize,
    pub end: usize,
    pub next_page: usize,
    pub mapped_bytes: usize,
    pub small_allocations: usize,
    pub large_allocations: usize,
    pub slabs: usize,
    pub initialized: bool,
}

unsafe impl Send for SlabAllocator {}

impl LockedSlabAllocator {
    const fn new() -> Self {
        Self {
            inner: SpinLock::new(SlabAllocator::new()),
        }
    }

    unsafe fn init(&self, heap_start: usize, heap_size: usize) {
        self.inner.lock().init(heap_start, heap_size);
    }
}

impl SlabAllocator {
    const fn new() -> Self {
        Self {
            heap_start: 0,
            heap_end: 0,
            next_page: 0,
            mapped_bytes: 0,
            classes: [
                SlabClass::new(8),
                SlabClass::new(16),
                SlabClass::new(32),
                SlabClass::new(64),
                SlabClass::new(128),
                SlabClass::new(256),
                SlabClass::new(512),
                SlabClass::new(1024),
                SlabClass::new(2048),
            ],
            large_allocations: 0,
            initialized: false,
        }
    }

    fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next_page = heap_start;
        self.mapped_bytes = 0;
        self.large_allocations = 0;
        self.initialized = true;

        for class in self.classes.iter_mut() {
            class.free_list = null_mut();
            class.slab_count = 0;
            class.allocations = 0;
        }
    }

    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        if !self.initialized {
            return null_mut();
        }

        if let Some(class_index) = self.class_index(layout) {
            if self.classes[class_index].free_list.is_null() && !self.grow_class(class_index) {
                return null_mut();
            }

            return self.alloc_from_class(class_index);
        }

        self.alloc_large(layout)
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        if let Some(class_index) = self.class_index(layout) {
            let class = &mut self.classes[class_index];
            let node = ptr as *mut FreeNode;
            (*node).next = class.free_list;
            class.free_list = node;
            class.allocations = class.allocations.saturating_sub(1);
        } else {
            self.large_allocations = self.large_allocations.saturating_sub(1);
        }
    }

    fn class_index(&self, layout: Layout) -> Option<usize> {
        let required = layout
            .size()
            .max(layout.align())
            .max(mem::size_of::<FreeNode>());

        if required > LARGE_ALLOC_THRESHOLD {
            return None;
        }

        self.classes
            .iter()
            .position(|class| class.block_size >= required)
    }

    fn grow_class(&mut self, class_index: usize) -> bool {
        let page = align_up(self.next_page, SLAB_SIZE);
        let Some(next_page) = page.checked_add(SLAB_SIZE) else {
            return false;
        };

        if next_page > self.heap_end {
            return false;
        }

        if map_heap_range(page, SLAB_SIZE).is_err() {
            return false;
        }

        self.next_page = next_page;
        self.mapped_bytes += SLAB_SIZE;

        let class = &mut self.classes[class_index];
        let block_count = SLAB_SIZE / class.block_size;

        for index in 0..block_count {
            let node = (page + index * class.block_size) as *mut FreeNode;
            unsafe {
                (*node).next = class.free_list;
            }
            class.free_list = node;
        }

        class.slab_count += 1;
        true
    }

    fn alloc_from_class(&mut self, class_index: usize) -> *mut u8 {
        let class = &mut self.classes[class_index];
        let node = class.free_list;

        if node.is_null() {
            return null_mut();
        }

        unsafe {
            class.free_list = (*node).next;
        }
        class.allocations += 1;
        node as *mut u8
    }

    fn alloc_large(&mut self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(SLAB_SIZE);
        let alloc_start = align_up(self.next_page, align);
        let alloc_size = align_up(layout.size(), SLAB_SIZE);
        let Some(alloc_end) = alloc_start.checked_add(alloc_size) else {
            return null_mut();
        };

        if alloc_end > self.heap_end {
            return null_mut();
        }

        if map_heap_range(alloc_start, alloc_size).is_err() {
            return null_mut();
        }

        self.next_page = alloc_end;
        self.mapped_bytes += alloc_size;
        self.large_allocations += 1;
        alloc_start as *mut u8
    }
}

impl SlabClass {
    const fn new(block_size: usize) -> Self {
        Self {
            block_size,
            free_list: null_mut(),
            slab_count: 0,
            allocations: 0,
        }
    }
}

pub fn init() {
    unsafe {
        ALLOCATOR.init(KERNEL_HEAP_START as usize, KERNEL_HEAP_SIZE);
    }
}

pub fn stats() -> HeapStats {
    let allocator = ALLOCATOR.inner.lock();
    let mut small_allocations = 0;
    let mut slabs = 0;

    for class in allocator.classes.iter() {
        small_allocations += class.allocations;
        slabs += class.slab_count;
    }

    HeapStats {
        start: allocator.heap_start,
        end: allocator.heap_end,
        next_page: allocator.next_page,
        mapped_bytes: allocator.mapped_bytes,
        small_allocations,
        large_allocations: allocator.large_allocations,
        slabs,
        initialized: allocator.initialized,
    }
}

unsafe impl GlobalAlloc for LockedSlabAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.inner.lock().alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner.lock().dealloc(ptr, layout);
    }
}

const fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

fn map_heap_range(start: usize, len: usize) -> Result<(), paging::PagingError> {
    paging::map_allocated_range(start as u64, len as u64, paging::kernel_data_flags())
}

#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout);
}
