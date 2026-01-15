use super::paging::{OffsetPageTable, PageTableFlags};
use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub const HEAP_START: u64 = 0x_4444_4444_0000;
pub const HEAP_SIZE: u64 = 1024 * 1024 * 128; // allocate 10MB for now so we have enough space for the double buffer

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
            .lock()
            .init(HEAP_START as *mut u8, HEAP_SIZE as usize);
    }
}

pub fn get_heap_usage() -> usize {
    ALLOCATOR.lock().used()
}

pub fn get_heap_size() -> usize {
    ALLOCATOR.lock().size()
}
