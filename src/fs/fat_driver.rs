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
        // Calculate how many 512-byte sectors fit in this buffer
        let count = (buf.len() / 512) as u8;

        if count == 0 {
            // If someone asks for less than 512 bytes, we have a problem
            return Err(ErrorKind::Other);
        }

        self.driver.read_sectors(block, count, buf);
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
