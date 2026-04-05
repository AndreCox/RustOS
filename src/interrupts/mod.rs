mod asm_stubs;
mod fs_syscalls;
mod handlers;
mod idt;
mod syscall;

use core::sync::atomic::{AtomicU64, Ordering};

pub use idt::{init_idt, init_pic};

static BUSY_TICKS: AtomicU64 = AtomicU64::new(0);
static TOTAL_TICKS: AtomicU64 = AtomicU64::new(0);

pub(crate) fn on_timer_tick() {
    TOTAL_TICKS.fetch_add(1, Ordering::Relaxed);
}

pub fn get_cpu_usage() -> u32 {
    let total = TOTAL_TICKS.swap(0, Ordering::Relaxed);
    let busy = BUSY_TICKS.swap(0, Ordering::Relaxed);

    if total == 0 {
        return 0;
    }

    ((busy as u64 * 100) / total as u64) as u32
}
