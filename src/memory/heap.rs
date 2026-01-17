use core::alloc::GlobalAlloc;

use crate::memory::sys_sbrk;

use super::paging::{OffsetPageTable, PageTableFlags};
use linked_list_allocator::LockedHeap;

pub struct DynamicLockedHeap(pub LockedHeap);

unsafe impl GlobalAlloc for DynamicLockedHeap {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut heap = self.0.lock();

        // try to allocate memory
        match heap.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => {
                drop(heap); // release the lock before growing the heap
                let grow_size = layout.size().max(1024 * 1024); // grow by at least 1 MiB this stops frequent small allocations from growing the heap
                sys_sbrk(grow_size as isize);

                // try to allocate again
                self.0
                    .lock()
                    .allocate_first_fit(layout)
                    .map(|ptr| ptr.as_ptr())
                    .unwrap_or(core::ptr::null_mut()) // return null if allocation fails again
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        unsafe { self.0.dealloc(ptr, layout) }
    }
}

#[global_allocator]
pub static ALLOCATOR: DynamicLockedHeap = DynamicLockedHeap(LockedHeap::empty());

pub const HEAP_START: u64 = 0x_4444_0000_0000;
pub const HEAP_SIZE: u64 = 1024 * 1024 * 20; // allocate 20 MiB for the heap

pub fn init_heap(mapper: &mut OffsetPageTable) {
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    // Map every page in the heap region to a physical frame
    for offset in (0..HEAP_SIZE).step_by(0x1000) {
        let virt_addr = HEAP_START + offset;
        let phy_frame = crate::memory::frame::allocate_frame()
            .expect("Failed to allocate frame for heap initialization");

        mapper.map(virt_addr, phy_frame, flags);
    }

    unsafe {
        ALLOCATOR
            .0
            .lock()
            .init(HEAP_START as *mut u8, HEAP_SIZE as usize);
    }
}

pub fn get_heap_usage() -> usize {
    ALLOCATOR.0.lock().used()
}

pub fn get_heap_size() -> usize {
    ALLOCATOR.0.lock().size()
}
