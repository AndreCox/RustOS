const SERIAL_PORT: u16 = 0x3F8; // COM1 port address

pub fn is_transmit_empty() -> bool {
    let status: u8;
    unsafe {
        core::arch::asm!(
            "in al, dx",
            out("al") status,
            in("dx") SERIAL_PORT + 5, // Status register is at offset 5
            options(nomem, nostack, preserves_flags)
        );
    }
    // Bit 5 (0x20) is the "Transmitter Holding Register Empty" flag
    (status & 0x20) != 0
}

pub fn serial_write_byte(byte: u8) {
    // Wait for the hardware to be ready for the next byte
    while !is_transmit_empty() {
        core::hint::spin_loop();
    }

    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") SERIAL_PORT,
            in("al") byte,
            options(nomem, nostack, preserves_flags)
        );
    }
}
