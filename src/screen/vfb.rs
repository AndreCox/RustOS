use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

pub struct VirtualFb {
    pub ptr: usize,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub owner: u64,
    pub dirty_min_y: AtomicU64,
    pub dirty_max_y: AtomicU64,
}

impl VirtualFb {
    fn new(ptr: usize, width: usize, height: usize, owner: u64) -> Self {
        VirtualFb {
            ptr,
            width,
            height,
            pitch: width * 4,
            owner,
            dirty_min_y: AtomicU64::new(u64::MAX),
            dirty_max_y: AtomicU64::new(0),
        }
    }
}

pub static VFB_LIST: Mutex<Vec<VirtualFb>> = Mutex::new(Vec::new());

/// Allocate a virtual framebuffer (RGBX/u32) and register it.
/// Returns a pointer that the guest code can write into.
pub fn create_virtual_fb(owner: u64, width: usize, height: usize) -> *mut u32 {
    let size = width * height * 4;
    let mut v = Vec::with_capacity(size);
    v.resize(size, 0u8);
    let slice: &'static mut [u8] = v.leak();
    let ptr = slice.as_mut_ptr() as *mut u32;

    let mut list = VFB_LIST.lock();
    list.push(VirtualFb::new(ptr as usize, width, height, owner));
    ptr
}

pub fn mark_dirty(buf_ptr: *mut u32, y: u64, height: u64) {
    let mut list = VFB_LIST.lock();
    for fb in list.iter_mut() {
        if fb.ptr == buf_ptr as usize {
            let y0 = y as u64;
            let y1 = (y + height) as u64;
            let prev_min = fb.dirty_min_y.load(Ordering::Relaxed);
            if y0 < prev_min {
                fb.dirty_min_y.store(y0, Ordering::Relaxed);
            }
            let prev_max = fb.dirty_max_y.load(Ordering::Relaxed);
            if y1 > prev_max {
                fb.dirty_max_y.store(y1, Ordering::Relaxed);
            }
            return;
        }
    }
}

pub fn snapshot_meta() -> Vec<(usize, usize, usize, u64, u64, u64)> {
    let list = VFB_LIST.lock();
    list.iter()
        .map(|fb| {
            (
                fb.ptr,
                fb.width,
                fb.height,
                fb.owner,
                fb.dirty_min_y.load(Ordering::Relaxed),
                fb.dirty_max_y.load(Ordering::Relaxed),
            )
        })
        .collect()
}

pub fn clear_dirty(buf_ptr: *mut u32) {
    let mut list = VFB_LIST.lock();
    for fb in list.iter_mut() {
        if fb.ptr == buf_ptr as usize {
            fb.dirty_min_y.store(u64::MAX, Ordering::Relaxed);
            fb.dirty_max_y.store(0, Ordering::Relaxed);
            return;
        }
    }
}

/// Release any virtual framebuffers owned by `owner_id`.
/// This will zero the buffer memory and clear ownership so compositor
/// can stop attempting to composite it.
pub fn release_owner(owner_id: u64) {
    crate::println!("[VFB] release_owner start {}", owner_id);
    let mut list = VFB_LIST.lock();
    for fb in list.iter_mut() {
        if fb.owner == owner_id {
            // Avoid touching the framebuffer memory directly (it may be
            // invalid if the task crashed). Just release ownership and
            // clear dirty markers so the compositor ignores it.
            fb.owner = 0;
            fb.dirty_min_y.store(u64::MAX, Ordering::Relaxed);
            fb.dirty_max_y.store(0, Ordering::Relaxed);
        }
    }
    crate::println!("[VFB] release_owner done {}", owner_id);
}
