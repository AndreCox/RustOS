use x86_64::instructions::port::Port;

pub struct AtaPio {
    data_port: Port<u16>,
    error_port: Port<u8>,
    sector_count_port: Port<u8>,
    lba_low_port: Port<u8>,
    lba_mid_port: Port<u8>,
    lba_high_port: Port<u8>,
    device_port: Port<u8>,
    command_port: Port<u8>,
    status_port: Port<u8>,
}

impl AtaPio {
    pub fn read_sectors(&mut self, lba: u32, count: u8, buffer: &mut [u8]) {
        let flags_were_enabled = interrupts_enabled();
        // Prevent context switches during timing-sensitive disk IO
        unsafe { core::arch::asm!("cli") };

        unsafe {
            // 1. Prepare the drive for a multi-sector read
            self.device_port.write(0xE0 | ((lba >> 24) & 0x0F) as u8);
            self.sector_count_port.write(count);
            self.lba_low_port.write(lba as u8);
            self.lba_mid_port.write((lba >> 8) as u8);
            self.lba_high_port.write((lba >> 16) as u8);
            self.command_port.write(0x20); // 0x20 = Read Sectors
        }

        // 2. We MUST loop 'count' times
        for sector in 0..count as usize {
            // Wait for the drive to finish seeking and fill its internal buffer
            while self.is_busy() {}
            if self.has_error() {
                crate::serial_println!("ATA Error during read_sectors (LBA: {})", lba);
                if flags_were_enabled { unsafe { core::arch::asm!("sti") }; }
                return;
            }

            while !self.is_ready() {
                if self.has_error() {
                    crate::serial_println!("ATA Error while waiting for ready (LBA: {})", lba);
                    if flags_were_enabled { unsafe { core::arch::asm!("sti") }; }
                    return;
                }
            } // DRQ must be set for each sector

            // 3. Transfer 256 words (512 bytes) for THIS sector
            for i in 0..256 {
                let data = unsafe { self.data_port.read() };
                let offset = (sector * 512) + (i * 2);

                // Safety check: ensure we don't overflow the provided buffer
                if offset + 1 < buffer.len() {
                    buffer[offset] = data as u8;
                    buffer[offset + 1] = (data >> 8) as u8;
                }
            }
        }

        if flags_were_enabled { unsafe { core::arch::asm!("sti") }; }
    }

    pub fn write_sectors(&mut self, lba: u32, count: u8, buffer: &[u8]) {
        let flags_were_enabled = interrupts_enabled();
        unsafe { core::arch::asm!("cli") };

        unsafe {
            self.device_port.write(0xE0 | ((lba >> 24) & 0x0F) as u8);
            self.sector_count_port.write(count);
            self.lba_low_port.write(lba as u8);
            self.lba_mid_port.write((lba >> 8) as u8);
            self.lba_high_port.write((lba >> 16) as u8);
            self.command_port.write(0x30); // Write Sectors command
        }

        for sector in 0..count as usize {
            while self.is_busy() {}
            if self.has_error() {
                crate::serial_println!("ATA Error during write_sectors (LBA: {})", lba);
                if flags_were_enabled { unsafe { core::arch::asm!("sti") }; }
                return;
            }

            while !self.is_ready() {
                if self.has_error() {
                    crate::serial_println!("ATA Error while waiting for ready in write (LBA: {})", lba);
                    if flags_were_enabled { unsafe { core::arch::asm!("sti") }; }
                    return;
                }
            } // Drive says "Okay, give me the next 512 bytes"

            for i in 0..256 {
                let offset = (sector * 512) + (i * 2);
                let data = (buffer[offset] as u16) | ((buffer[offset + 1] as u16) << 8);
                unsafe { self.data_port.write(data) };
            }
        }

        // Always flush after a write operation
        unsafe {
            self.command_port.write(0xE7);
        }
        while self.is_busy() {}
        if flags_were_enabled { unsafe { core::arch::asm!("sti") }; }
    }
}

impl AtaPio {
    pub fn init() -> Self {
        let mut bus = Self {
            data_port: Port::new(0x1F0),
            error_port: Port::new(0x1F1),
            sector_count_port: Port::new(0x1F2),
            lba_low_port: Port::new(0x1F3),
            lba_mid_port: Port::new(0x1F4),
            lba_high_port: Port::new(0x1F5),
            device_port: Port::new(0x1F6),
            command_port: Port::new(0x1F7),
            status_port: Port::new(0x1F7),
        };

        // Select the master drive
        unsafe {
            bus.device_port.write(0xA0);
        }

        bus
    }

    pub fn is_busy(&mut self) -> bool {
        unsafe { (self.status_port.read() & 0x80) != 0 }
    }

    pub fn is_ready(&mut self) -> bool {
        unsafe { (self.status_port.read() & 0x08) != 0 }
    }

    pub fn has_error(&mut self) -> bool {
        unsafe { (self.status_port.read() & 0x01) != 0 }
    }
}

fn interrupts_enabled() -> bool {
    let rflags: u64;
    unsafe { core::arch::asm!("pushfq; pop {}", out(reg) rflags, options(nomem, preserves_flags)) };
    (rflags & (1 << 9)) != 0
}
