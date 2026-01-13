pub mod frame;
mod heap;
pub mod paging;

pub use frame::allocate_frame;
pub use heap::{get_heap_size, get_heap_usage};

pub fn init() {
    frame::mem_map_init();
    let mut mapper = paging::init_paging();
    heap::init_heap(&mut mapper);
}
