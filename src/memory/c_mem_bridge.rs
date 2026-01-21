use crate::memory::heap::ALLOCATOR;
use core::alloc::{GlobalAlloc, Layout};

#[repr(C, align(16))]
struct MallocHeader {
    size: usize,
}

const HEADER_SIZE: usize = core::mem::size_of::<MallocHeader>();
const ALIGNMENT: usize = 16;

#[inline]
unsafe fn layout_for_size(size: usize) -> Option<Layout> {
    Layout::from_size_align(size.checked_add(HEADER_SIZE)?, ALIGNMENT).ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn malloc(size: usize) -> *mut u8 {
    if size == 0 {
        return ALIGNMENT as *mut u8; // Return aligned non-null for zero-size
    }

    let Some(layout) = (unsafe { layout_for_size(size) }) else {
        return core::ptr::null_mut();
    };

    let ptr = unsafe { ALLOCATOR.alloc(layout) };
    if ptr.is_null() {
        return core::ptr::null_mut();
    }

    unsafe {
        // Write header
        (ptr as *mut MallocHeader).write(MallocHeader { size });

        // Return data pointer (already zeroed by write_bytes in original, keep if needed)
        let data_ptr = ptr.add(HEADER_SIZE);
        core::ptr::write_bytes(data_ptr, 0, size);
        data_ptr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn free(ptr: *mut u8) {
    if ptr.is_null() || ptr as usize == ALIGNMENT {
        return; // Handle zero-size sentinel
    }

    unsafe {
        let raw_ptr = ptr.sub(HEADER_SIZE);
        let size = (*(raw_ptr as *const MallocHeader)).size;

        // Reconstruct layout - use unwrap_unchecked since we know it's valid
        let layout = Layout::from_size_align_unchecked(size + HEADER_SIZE, ALIGNMENT);
        ALLOCATOR.dealloc(raw_ptr, layout);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn calloc(nmemb: usize, size: usize) -> *mut u8 {
    // Check for overflow
    let Some(total) = nmemb.checked_mul(size) else {
        return core::ptr::null_mut();
    };

    // malloc already zeros memory, so we can just call it directly
    malloc(total)
}

#[unsafe(no_mangle)]
pub extern "C" fn realloc(ptr: *mut u8, new_size: usize) -> *mut u8 {
    if ptr.is_null() || ptr as usize == ALIGNMENT {
        return malloc(new_size);
    }

    if new_size == 0 {
        free(ptr);
        return core::ptr::null_mut();
    }

    unsafe {
        let raw_ptr = ptr.sub(HEADER_SIZE);
        let old_size = (*(raw_ptr as *const MallocHeader)).size;

        // If new size is smaller or equal and fits in same allocation class,
        // we could potentially reuse the allocation (optional optimization)
        if new_size <= old_size {
            // Update header size
            (raw_ptr as *mut MallocHeader).write(MallocHeader { size: new_size });
            // Zero out freed portion
            core::ptr::write_bytes(ptr.add(new_size), 0, old_size - new_size);
            return ptr;
        }

        // Allocate new space
        let new_ptr = malloc(new_size);
        if new_ptr.is_null() {
            return core::ptr::null_mut();
        }

        // Copy old data (malloc already zeroed the rest)
        core::ptr::copy_nonoverlapping(ptr, new_ptr, old_size);

        // Free old pointer
        let layout = Layout::from_size_align_unchecked(old_size + HEADER_SIZE, ALIGNMENT);
        ALLOCATOR.dealloc(raw_ptr, layout);

        new_ptr
    }
}
