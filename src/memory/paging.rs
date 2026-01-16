use bitflags::bitflags;
use core::arch::asm;

use crate::{memory::allocate_frame, serial_println};

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct PageTableFlags: u64 {
        const PRESENT = 1 << 0;         // Page is present in memory
        const WRITABLE = 1 << 1;        // Page is writable
        const USER_ACCESSIBLE = 1 << 2; // User-mode can access this page
        const WRITE_THROUGH = 1 << 3;   // Write-through caching enabled
        const NO_CACHE = 1 << 4;        // Caching disabled
        const ACCESSED = 1 << 5;        // Set by CPU on read
        const DIRTY = 1 << 6;           // Set by CPU on write
        const HUGE_PAGE = 1 << 7;       // Map 2MB/1GB page
        const GLOBAL = 1 << 8;          // Global page (not flushed from TLB)
        const NO_EXECUTE = 1 << 63;     // Execute disable
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    // Check if the entry is unused
    pub fn is_unused(&self) -> bool {
        self.0 == 0
    }

    // Set the entry to unused
    pub fn set_unused(&mut self) {
        self.0 = 0;
    }

    // Get the flags of the entry
    pub fn flags(&self) -> PageTableFlags {
        PageTableFlags::from_bits_truncate(self.0)
    }

    pub fn address(&self) -> u64 {
        self.0 & 0x000fffff_fffff000
    }

    pub fn set_address(&mut self, phys_addr: u64, flags: PageTableFlags) {
        assert!(
            phys_addr % 0x1000 == 0,
            "Physical address must be 4KiB aligned"
        );
        self.0 = (phys_addr & 0x000fffff_fffff000) | flags.bits();
    }
}

#[repr(align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    pub fn zero(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.set_unused();
        }
    }
}

pub struct OffsetPageTable {
    level_4_table: &'static mut PageTable,
    hhdm_offset: u64,
}

impl OffsetPageTable {
    pub unsafe fn new(phys_level_4_addr: u64, hhdm_offset: u64) -> Self {
        let virt_addr_raw = phys_level_4_addr + hhdm_offset;

        let virt_ptr = virt_addr_raw as *mut PageTable;

        Self {
            level_4_table: unsafe { &mut *virt_ptr },
            hhdm_offset,
        }
    }

    pub fn map(&mut self, virt: u64, phys: u64, flags: PageTableFlags) {
        let p4_idx = ((virt >> 39) & 0x1ff) as usize;
        let p3_idx = ((virt >> 30) & 0x1ff) as usize;
        let p2_idx = ((virt >> 21) & 0x1ff) as usize;
        let p1_idx = ((virt >> 12) & 0x1ff) as usize;

        let l3 =
            Self::next_table_or_create(&mut self.level_4_table.entries[p4_idx], self.hhdm_offset);
        let l2 = Self::next_table_or_create(&mut l3.entries[p3_idx], self.hhdm_offset);
        let l1 = Self::next_table_or_create(&mut l2.entries[p2_idx], self.hhdm_offset);

        l1.entries[p1_idx].set_address(phys, flags | PageTableFlags::PRESENT);

        unsafe {
            asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));
        }
    }

    // navigates one level down, creates it if it doesn't exist
    fn next_table_or_create(
        entry: &mut PageTableEntry,
        hhdm_offset: u64,
    ) -> &'static mut PageTable {
        if entry.is_unused() {
            let frame = allocate_frame().expect("Out of memory");
            let virt_addr = frame + hhdm_offset;

            // serial_println!(
            //     "Creating new page table at phys: {:#x} (virt: {:#x})",
            //     frame,
            //     virt_addr
            // );

            let table = unsafe { &mut *(virt_addr as *mut PageTable) };

            table.zero();
            // serial_println!("Table zeroed successfully.");
            entry.set_address(frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
            table
        } else {
            let virt_addr = entry.address() + hhdm_offset;
            unsafe { &mut *(virt_addr as *mut PageTable) }
        }
    }
}

pub fn init_paging() -> OffsetPageTable {
    // get hhdm offset from limine
    let hhdm_response = crate::HHDM_REQUEST.get_response().unwrap();
    let hhdm_offset = hhdm_response.offset();

    let mut cr3: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) cr3);
    }

    // Mask out the flags from cr3 to get the physical address of the level 4 page table
    cr3 &= 0x000fffff_fffff000;

    // create the mapper
    let mut mapper = unsafe { OffsetPageTable::new(cr3, hhdm_offset) };

    // Test map VGA buffer to see if paging works
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
    mapper.map(0xb8000, 0xb8000, flags);

    serial_println!("Paging initialized.");
    mapper
}

pub fn get_active_mapper() -> OffsetPageTable {
    let hhdm_response = crate::HHDM_REQUEST.get_response().unwrap();
    let hhdm_offset = hhdm_response.offset();

    let mut cr3: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) cr3);
    }
    cr3 &= 0x000fffff_fffff000;

    unsafe { OffsetPageTable::new(cr3, hhdm_offset) }
}
