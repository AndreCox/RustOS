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
        // For now, we'll hardcode a common size or a large enough value.
        // Ideally, the AtaPio driver should return the disk size.
        0x400000
    }
}

impl BlockRead for AtaIoWrapper {
    fn read(&mut self, block: u32, buf: &mut [u8]) -> Result<(), Self::Error> {
        if buf.is_empty() { return Ok(()); }
        let count = (buf.len() / 512) as usize;
        
        crate::serial_println!("ATA_READ: block={}, buf.len()={}, count={}", block, buf.len(), count);
        
        if count == 0 {
            return Err(ErrorKind::Other);
        }

        let mut remaining = count;
        let mut cur_block = block;
        let mut offset = 0;

        while remaining > 0 {
            let chunk = core::cmp::min(remaining, 255) as u8;
            self.driver.read_sectors(cur_block, chunk, &mut buf[offset .. offset + (chunk as usize) * 512]);
            cur_block += chunk as u32;
            offset += (chunk as usize) * 512;
            remaining -= chunk as usize;
        }

        Ok(())
    }
}

impl BlockWrite for AtaIoWrapper {
    fn write(&mut self, block: u32, buf: &[u8]) -> Result<(), Self::Error> {
        if buf.is_empty() { return Ok(()); }
        let count = (buf.len() / 512) as u8;
        if count == 0 { return Err(ErrorKind::Other); }
        // Make sure your AtaPio struct in ata_driver.rs has this method!
        // If it's missing, you'll need to implement it similar to read_sectors.
        self.driver.write_sectors(block, count, buf);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
