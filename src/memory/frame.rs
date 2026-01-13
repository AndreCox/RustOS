use crate::{helpers::hcf, screen_println, serial_println};
use limine::request::MemoryMapRequest;
use spin::Mutex;

pub static FRAME_ALLOCATOR: Mutex<Option<BitmapAllocator>> = Mutex::new(None);

pub struct BitmapAllocator {
    pub bitmap: &'static mut [u8],
    pub total_pages: usize,
    pub highest_address: u64,

    last_allocated: usize,
}

impl BitmapAllocator {
    fn mark_used(&mut self, addr: u64) {
        let frame_index = (addr / 0x1000) as usize;
        self.bitmap[frame_index / 8] |= 1 << (frame_index % 8);
    }

    fn mark_free(&mut self, addr: u64) {
        let frame_index = (addr / 0x1000) as usize;
        let byte_index = frame_index / 8;
        if byte_index < self.bitmap.len() {
            self.bitmap[byte_index] &= !(1 << (frame_index % 8));
        }
    }

    fn alloc_frame(&mut self) -> Option<u64> {
        let start_frame = 256; // Skip first 1MB (256 pages)

        // We will check 'total_pages' number of bits, starting from our last success
        for i in 0..self.total_pages {
            // Use modulo to wrap back to 0 if we hit the end of the bitmap
            let frame_idx = (self.last_allocated + i) % self.total_pages;
            if frame_idx < start_frame {
                continue; // Skip reserved low memory
            }

            let byte_idx = frame_idx / 8;
            let bit_idx = frame_idx % 8;

            let mask = 1 << bit_idx;
            if (self.bitmap[byte_idx] & mask) == 0 {
                // Found a free page!
                let addr = (frame_idx as u64) * 0x1000;

                // Safety check against the edge of physical RAM
                if addr >= self.highest_address {
                    continue; // Try the next bit instead of giving up
                }

                self.mark_used(addr);

                // UPDATE THE HINT: Start here next time!
                self.last_allocated = frame_idx;

                return Some(addr);
            }
        }

        serial_println!("Out of memory!");
        screen_println!("Out of memory!");
        None
    }
}

#[used]
#[unsafe(link_section = ".limine_requests")]
pub static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

// length is the size of the memory region in bytes
fn calculate_bitmap_size(length: u64) -> usize {
    // Each bit represents a 4KiB page
    let total_pages = (length + 0xFFF) / 0x1000; // Round up to nearest page
    let bitmap_bytes = (total_pages + 7) / 8; // Round up to nearest byte
    ((bitmap_bytes + 0xFFF) / 0x1000 * 0x1000) as usize // Round up to nearest page size
}

pub fn mem_map_init() {
    if !MEMORY_MAP_REQUEST.get_response().is_some() {
        screen_println!("Memory Map Request not supported.");
        screen_println!("Halting Something has gone horribly wrong!");
        hcf();
    }

    let mem_map_response = MEMORY_MAP_REQUEST.get_response().unwrap();

    let hhdm_response = crate::HHDM_REQUEST.get_response().unwrap();
    let hhdm_offset = hhdm_response.offset();

    // Determine the highest addressable memory
    let mut highest_address: u64 = 0;
    mem_map_response.entries().iter().for_each(|entry| {
        if !matches!(
            entry.entry_type,
            limine::memory_map::EntryType::USABLE
                | limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE
        ) {
            return;
        }

        let end_address = entry.base + entry.length;
        if end_address > highest_address {
            highest_address = end_address;
        }
    });

    serial_println!("Highest addressable memory: {:#x} bytes", highest_address);

    let bitmap_size = calculate_bitmap_size(highest_address);

    serial_println!("Bitmap size: {} bytes", bitmap_size);

    // determine the first usable memory region to place the bitmap
    let bitmap_location = mem_map_response
        .entries()
        .iter()
        .find(|entry| {
            matches!(
                entry.entry_type,
                limine::memory_map::EntryType::USABLE
                    | limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE
            ) && entry.length >= bitmap_size as u64
        })
        .map(|entry| entry.base)
        .expect("No suitable memory region found for bitmap");

    if bitmap_location % 0x1000 != 0 {
        screen_println!("Bitmap location is not page aligned. Halting.");
        hcf();
    }

    serial_println!("Placing bitmap at address: {:#x}", bitmap_location);

    let bitmap_ptr = (bitmap_location + hhdm_offset) as *mut u8;

    let bitmap_slice: &'static mut [u8] =
        unsafe { core::slice::from_raw_parts_mut(bitmap_ptr, bitmap_size) };

    // Initialize the bitmap allocator
    let mut allocator = BitmapAllocator {
        bitmap: bitmap_slice,
        total_pages: (highest_address as usize + 0xFFF) / 0x1000,
        highest_address,
        last_allocated: 0,
    };

    // Mark all pages as used initially
    allocator.bitmap.fill(0xFF);

    // Now mark usable pages as free
    for entry in mem_map_response.entries() {
        if !matches!(
            entry.entry_type,
            limine::memory_map::EntryType::USABLE
                | limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE
        ) {
            continue;
        }
        for addr in (entry.base..entry.base + entry.length).step_by(0x1000) {
            allocator.mark_free(addr);
        }
    }

    // Mark the bitmap's own pages as used
    for addr in (bitmap_location..bitmap_location + bitmap_size as u64).step_by(0x1000) {
        allocator.mark_used(addr);
    }

    // Store the allocator in the global mutex
    *FRAME_ALLOCATOR.lock() = Some(allocator);

    screen_println!("Memory Map Entries:");
    for entry in mem_map_response.entries() {
        let base = entry.base;
        let length = entry.length;
        let entry_type = entry.entry_type;

        let entry_type_str = match entry_type {
            limine::memory_map::EntryType::USABLE => "Usable",
            limine::memory_map::EntryType::RESERVED => "Reserved",
            limine::memory_map::EntryType::ACPI_RECLAIMABLE => "ACPI Reclaimable",
            limine::memory_map::EntryType::ACPI_NVS => "ACPI NVS",
            limine::memory_map::EntryType::BAD_MEMORY => "Bad Memory",
            limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE => "Bootloader Reclaimable",
            limine::memory_map::EntryType::EXECUTABLE_AND_MODULES => "Executable and Modules",
            limine::memory_map::EntryType::FRAMEBUFFER => "Framebuffer",
            _ => "Unknown",
        };

        screen_println!(
            "Base: {:#016x}, Length: {:#016x}, Type: {:?}",
            base,
            length,
            entry_type_str
        );

        serial_println!(
            "Base: {:#016x}, Length: {:#016x}, Type: {:?}",
            base,
            length,
            entry_type_str
        );
    }
}

pub fn allocate_frame() -> Option<u64> {
    if let Some(ref mut allocator) = *FRAME_ALLOCATOR.lock() {
        allocator.alloc_frame()
    } else {
        None
    }
}

pub fn deallocate_frame(addr: u64) {
    let mut lock = FRAME_ALLOCATOR.lock();
    if let Some(ref mut allocator) = *lock {
        allocator.mark_free(addr);
    }
}
