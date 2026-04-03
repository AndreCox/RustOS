use crate::io::ata_driver::AtaPio;
use embedded_io::{ErrorKind, ErrorType};
// Access traits and types directly from the root to avoid private module errors
use simple_fatfs::block_io::{BlockBase, BlockRead, BlockSize, BlockWrite};

pub struct AtaIoWrapper {
    pub driver: AtaPio,
}

impl AtaIoWrapper {
    pub fn new(driver: AtaPio) -> Self {
        Self { driver }
    }
}

impl ErrorType for AtaIoWrapper {
    type Error = ErrorKind;
}

impl BlockBase for AtaIoWrapper {
    fn block_size(&self) -> BlockSize {
        512
    }

    fn block_count(&self) -> u32 {
        // Disk size is 1024 MiB = 2,097,152 sectors (512 bytes each)
        0x200000
    }
}

impl BlockRead for AtaIoWrapper {
    fn read(&mut self, block: u32, buf: &mut [u8]) -> Result<(), Self::Error> {
        if buf.is_empty() {
            return Ok(());
        }

        if buf.len() % 512 != 0 {
            crate::serial_println!(
                "AtaIoWrapper: Sub-sector READ requested! len={}, block={}",
                buf.len(),
                block
            );
        }

        let count = (buf.len() / 512) as usize;
        if count == 0 {
            crate::serial_println!("AtaIoWrapper: Read error: buf too small ({})", buf.len());
            return Err(ErrorKind::Other);
        }

        let mut remaining = count;
        let mut cur_block = block;
        let mut offset = 0;

        while remaining > 0 {
            let chunk = core::cmp::min(remaining, 255) as u8;
            self.driver.read_sectors(
                cur_block,
                chunk,
                &mut buf[offset..offset + (chunk as usize) * 512],
            );
            cur_block += chunk as u32;
            offset += (chunk as usize) * 512;
            remaining -= chunk as usize;
        }

        Ok(())
    }
}

impl BlockWrite for AtaIoWrapper {
    fn write(&mut self, block: u32, buf: &[u8]) -> Result<(), Self::Error> {
        if buf.is_empty() {
            return Ok(());
        }

        if buf.len() % 512 != 0 {
            crate::serial_println!(
                "AtaIoWrapper: Sub-sector WRITE requested! len={}, block={}",
                buf.len(),
                block
            );
        }

        let count = (buf.len() / 512) as usize;
        if count == 0 {
            crate::serial_println!("AtaIoWrapper: Write error: buf too small ({})", buf.len());
            return Err(ErrorKind::Other);
        }

        // Handle possible overflow of u8 if simple_fatfs passes > 255 sectors
        let mut remaining = count;
        let mut cur_block = block;
        let mut offset = 0;

        while remaining > 0 {
            let chunk = core::cmp::min(remaining, 255) as u8;
            self.driver
                .write_sectors(cur_block, chunk, &buf[offset..offset + (chunk as usize) * 512]);
            cur_block += chunk as u32;
            offset += (chunk as usize) * 512;
            remaining -= chunk as usize;
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
