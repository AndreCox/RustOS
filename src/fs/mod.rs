mod fat_driver;

use crate::fs::fat_driver::AtaIoWrapper;
use crate::io::ata_driver::AtaPio;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::interrupts;
// Import DefaultClock as indicated by the compiler
use simple_fatfs::{DefaultClock, FSOptions, FileSystem};

lazy_static! {
    // Change () to DefaultClock
    pub static ref FILESYSTEM: Mutex<Option<FileSystem<AtaIoWrapper, DefaultClock>>> = Mutex::new(None);
}

pub fn with_filesystem<R>(
    f: impl FnOnce(&mut Option<FileSystem<AtaIoWrapper, DefaultClock>>) -> R,
) -> R {
    interrupts::without_interrupts(|| {
        let mut fs_lock = FILESYSTEM.lock();
        f(&mut fs_lock)
    })
}

pub fn init_fs() {
    let mut ata = AtaPio::init();

    let mut buf = [0u8; 512];

    // Read the very first sector of the drive
    ata.read_sectors(0, 1, &mut buf);

    // Print the first 16 bytes.
    // FAT32 usually starts with EB 58 90 or EB 3C 90.
    // MBR (partitioned disk) starts with various code but ends in 55 AA.
    crate::println!(
        "DISK HEX: {:02X} {:02X} {:02X} {:02X} | {:02X} {:02X} {:02X} {:02X}",
        buf[0],
        buf[1],
        buf[2],
        buf[3],
        buf[4],
        buf[5],
        buf[6],
        buf[7]
    );
    crate::println!("DISK SIGNATURE: {:02X} {:02X}", buf[510], buf[511]);
    let wrapper = AtaIoWrapper::new(ata);

    // FSOptions::new() defaults to DefaultClock
    let options = FSOptions::new();

    match FileSystem::new(wrapper, options) {
        Ok(fs) => {
            with_filesystem(|slot| *slot = Some(fs));
            crate::println!("Filesystem: FAT32 mounted successfully.");
        }
        Err(e) => {
            crate::println!("Mount Error: {:?}", e); // See if it's 'InvalidSignature' or 'IoError'
        }
    }
}
