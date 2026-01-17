pub mod c_mem_bridge;
pub mod frame;
mod heap;
pub mod paging;

pub use frame::allocate_frame;
pub use heap::{get_heap_size, get_heap_usage};

use heap::{ALLOCATOR, HEAP_START};

pub fn init() {
    frame::mem_map_init();
    let mut mapper = paging::init_paging();
    heap::init_heap(&mut mapper);
}

pub fn sys_sbrk(increment: isize) -> *mut u8 {
    let heap_size = ALLOCATOR.0.lock().size();
    let old_end_of_heap = HEAP_START + heap_size as u64;

    if increment == 0 {
        return old_end_of_heap as *mut u8;
    }

    if increment > 0 {
        let mut mapper = paging::get_active_mapper();

        let new_end_of_heap = old_end_of_heap + increment as u64;
        let mut map_ptr = (old_end_of_heap + 0xFFF) & !0xFFF; // align to page boundary

        while map_ptr < new_end_of_heap {
            let phy_fram = allocate_frame().expect("Failed to allocate frame for sys_sbrk");

            let flags: paging::PageTableFlags =
                paging::PageTableFlags::PRESENT | paging::PageTableFlags::WRITABLE;
            mapper.map(map_ptr, phy_fram, flags);
            map_ptr += 0x1000;
        }

        unsafe {
            ALLOCATOR.0.lock().extend(increment as usize);
        }
    }

    old_end_of_heap as *mut u8
}
