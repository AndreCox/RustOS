use core::sync::atomic::AtomicU64;

pub static mut KERNEL_CODE_SEGMENT: u16 = 0;
pub static mut KERNEL_DATA_SEGMENT: u16 = 0;

pub static IDLE_TICKS: AtomicU64 = AtomicU64::new(0);
