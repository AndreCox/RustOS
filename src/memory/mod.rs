pub mod frame;
pub mod paging;

pub use frame::{allocate_frame, deallocate_frame};

pub fn init() {
    frame::mem_map_init();
    paging::init_paging();
}
