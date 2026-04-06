use core::arch::asm;
use core::sync::atomic::{AtomicI32, AtomicU8, Ordering};

static mut PACKET: [u8; 3] = [0; 3];
static mut PACKET_IDX: usize = 0;
static MOUSE_DX: AtomicI32 = AtomicI32::new(0);
static MOUSE_DY: AtomicI32 = AtomicI32::new(0);
static MOUSE_BUTTONS: AtomicU8 = AtomicU8::new(0);

#[inline]
fn clamp_i32_to_i16(v: i32) -> i16 {
    if v > i16::MAX as i32 {
        i16::MAX
    } else if v < i16::MIN as i32 {
        i16::MIN
    } else {
        v as i16
    }
}

pub fn take_deltas_packed() -> u32 {
    let dx = clamp_i32_to_i16(MOUSE_DX.swap(0, Ordering::AcqRel));
    let dy = clamp_i32_to_i16(MOUSE_DY.swap(0, Ordering::AcqRel));

    (dx as u16 as u32) | ((dy as u16 as u32) << 16)
}

pub fn get_buttons_mask() -> u8 {
    MOUSE_BUTTONS.load(Ordering::Acquire)
}

#[inline]
fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe {
        asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    v
}

#[inline]
fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
    }
}

fn wait_input_empty() {
    for _ in 0..100_000 {
        if (inb(0x64) & 0x02) == 0 {
            return;
        }
    }
}

fn wait_output_full() -> bool {
    for _ in 0..100_000 {
        if (inb(0x64) & 0x01) != 0 {
            return true;
        }
    }
    false
}

fn read_data_timeout() -> Option<u8> {
    if wait_output_full() {
        Some(inb(0x60))
    } else {
        None
    }
}

fn write_cmd(cmd: u8) {
    wait_input_empty();
    outb(0x64, cmd);
}

fn write_data(data: u8) {
    wait_input_empty();
    outb(0x60, data);
}

fn write_mouse_cmd(cmd: u8) {
    write_cmd(0xD4);
    write_data(cmd);
}

pub fn init_ps2_mouse() {
    // Enable auxiliary (mouse) device on PS/2 controller.
    write_cmd(0xA8);

    // Read controller config byte.
    write_cmd(0x20);
    // If read times out, start from a conservative config that keeps IRQ1 on.
    let mut cfg = read_data_timeout().unwrap_or(0x01);

    // Keep keyboard IRQ1 enabled and enable mouse IRQ12.
    // Do not force the translation bit (bit 6): keyboard code expects current controller mode.
    cfg |= 1 << 0;
    cfg |= 1 << 1;

    // Write updated config byte.
    write_cmd(0x60);
    write_data(cfg);

    // Set defaults and enable streaming on the mouse device.
    write_mouse_cmd(0xF6);
    let _ = read_data_timeout(); // Expect ACK 0xFA.

    write_mouse_cmd(0xF4);
    let _ = read_data_timeout();
}

pub fn on_irq_byte(byte: u8) {
    unsafe {
        // First packet byte bit3 is always 1. Re-sync if stream got misaligned.
        if PACKET_IDX == 0 && (byte & 0x08) == 0 {
            return;
        }

        PACKET[PACKET_IDX] = byte;
        PACKET_IDX += 1;

        if PACKET_IDX < 3 {
            return;
        }

        PACKET_IDX = 0;

        let flags = PACKET[0];
        // Drop overflowed packets to avoid bad deltas and keep parser in sync.
        if (flags & 0xC0) != 0 {
            return;
        }

        // Intentionally keep IRQ work minimal. Packet consumers can be added later.
        let _dx = PACKET[1] as i8 as i16;
        let _dy = PACKET[2] as i8 as i16;

        MOUSE_DX.fetch_add(_dx as i32, Ordering::AcqRel);
        MOUSE_DY.fetch_add(_dy as i32, Ordering::AcqRel);
        MOUSE_BUTTONS.store(flags & 0x07, Ordering::Release);
    }
}
