use crate::{globals, println};
use core::arch::asm;

use super::asm_stubs::{isr_stub_128, isr_stub_table};

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const fn missing() -> Self {
        IdtEntry {
            offset_low: 0,
            selector: 0,
            ist: 0,
            attributes: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    fn set_handler(&mut self, handler_addr: u64, code_segment: u16) {
        self.offset_low = handler_addr as u16;
        self.offset_mid = (handler_addr >> 16) as u16;
        self.offset_high = (handler_addr >> 32) as u32;
        self.selector = code_segment;
        self.attributes = 0x8E;
        self.ist = 0;
        self.reserved = 0;
    }
}

#[repr(C, packed)]
struct IdtPointer {
    limit: u16,
    base: u64,
}

static mut IDT: [IdtEntry; 256] = [IdtEntry::missing(); 256];

pub unsafe fn init_idt() {
    let cs: u16;
    unsafe {
        asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags));
    }

    globals::KERNEL_CODE_SEGMENT = cs;

    unsafe {
        asm!("mov {0:x}, ss", out(reg) globals::KERNEL_DATA_SEGMENT, options(nomem, nostack, preserves_flags));
    }

    println!("Current CS is: {:#x}", cs);

    unsafe {
        for i in 0..48 {
            IDT[i].set_handler(isr_stub_table[i] as u64, cs);
        }
        IDT[0x80].set_handler(isr_stub_128 as u64, cs);
    }

    let idt_ptr = IdtPointer {
        limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: core::ptr::addr_of!(IDT) as u64,
    };

    unsafe {
        asm!("lidt [{}]", in(reg) &idt_ptr, options(readonly, nostack, preserves_flags));
    }
}

pub unsafe fn init_pic() {
    const PIC1_COMMAND: u16 = 0x20;
    const PIC1_DATA: u16 = 0x21;
    const PIC2_COMMAND: u16 = 0xA0;
    const PIC2_DATA: u16 = 0xA1;

    outb(PIC1_COMMAND, 0x11);
    outb(PIC2_COMMAND, 0x11);

    outb(PIC1_DATA, 0x20);
    outb(PIC2_DATA, 0x28);

    outb(PIC1_DATA, 4);
    outb(PIC2_DATA, 2);

    outb(PIC1_DATA, 0x01);
    outb(PIC2_DATA, 0x01);

    outb(PIC1_DATA, 0xFC);
    outb(PIC2_DATA, 0xFF);
}

pub(super) fn send_eoi(interrupt_number: u64) {
    outb(0x20, 0x20);
    if interrupt_number >= 40 {
        outb(0xA0, 0x20);
    }
}

fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
    }
}
