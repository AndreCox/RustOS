use core::alloc::{GlobalAlloc, Layout};

use crate::memory::heap::ALLOCATOR;

#[repr(C)]
struct MallocHeader {
    size: usize,
}

#[unsafe(no_mangle)]
pub extern "C" fn malloc(size: usize) -> *mut u8 {
    let layout = Layout::from_size_align(size + core::mem::size_of::<MallocHeader>(), 8).unwrap();
    let ptr = unsafe { ALLOCATOR.alloc(layout) };
    if ptr.is_null() {
        return core::ptr::null_mut();
    }

    let header_ptr = ptr as *mut MallocHeader;
    unsafe {
        (*header_ptr).size = size;
    }
    unsafe { ptr.add(core::mem::size_of::<MallocHeader>()) }
}

#[unsafe(no_mangle)]
pub extern "C" fn free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let raw_ptr = unsafe { ptr.sub(core::mem::size_of::<MallocHeader>()) };
    let header = unsafe { &*(raw_ptr as *const MallocHeader) };
    let layout =
        Layout::from_size_align(header.size + core::mem::size_of::<MallocHeader>(), 8).unwrap();
    unsafe {
        ALLOCATOR.dealloc(raw_ptr, layout);
    }
}

// clear and allocate
#[unsafe(no_mangle)]
pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut u8 {
    let total = nmemb * size;
    let ptr = malloc(total);
    if !ptr.is_null() {
        core::ptr::write_bytes(ptr, 0, total);
    }
    ptr
}

// reallocate
#[unsafe(no_mangle)]
pub unsafe extern "C" fn realloc(ptr: *mut u8, new_size: usize) -> *mut u8 {
    if ptr.is_null() {
        return malloc(new_size);
    }
    if new_size == 0 {
        free(ptr);
        return core::ptr::null_mut();
    }

    // 1. Find the old size from our header
    let raw_ptr = ptr.sub(core::mem::size_of::<MallocHeader>());
    let header = &*(raw_ptr as *const MallocHeader);
    let old_size = header.size;

    // 2. Allocate new space
    let new_ptr = malloc(new_size);
    if new_ptr.is_null() {
        return core::ptr::null_mut();
    }

    // 3. Copy the data (only as much as fits)
    let copy_size = if old_size < new_size {
        old_size
    } else {
        new_size
    };
    core::ptr::copy_nonoverlapping(ptr, new_ptr, copy_size);

    // 4. Free the old pointer
    free(ptr);

    new_ptr
}
