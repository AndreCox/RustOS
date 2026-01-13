use core::sync::atomic::{AtomicU64, Ordering};

use crate::multitasker::yield_now;
static TICKS: AtomicU64 = AtomicU64::new(0);
pub const TICKS_PER_SECOND: u64 = 1000;

pub fn tick() {
    // Relaxed ordering is fine here because we're just incrementing a counter
    TICKS.fetch_add(1, Ordering::Relaxed);
}

pub fn get_uptime_ms() -> u64 {
    let current_ticks = TICKS.load(Ordering::Relaxed);
    // (ticks * 1000) / 60 gives us milliseconds
    (current_ticks * 1000) / TICKS_PER_SECOND
}

pub fn sleep_ms(ms: u64) {
    let start_time = get_uptime_ms();
    while get_uptime_ms() < start_time + ms {
        yield_now();
    }
}

pub fn init_timer() {
    let divisor: u16 = (1193180 / TICKS_PER_SECOND) as u16;

    unsafe {
        // Command port: Select Channel 0, Square Wave Mode
        core::arch::asm!("out 0x43, al", in("al") 0x36u8);

        // Data port: Send low byte then high byte of divisor
        core::arch::asm!("out 0x40, al", in("al") (divisor & 0xFF) as u8);
        core::arch::asm!("out 0x40, al", in("al") ((divisor >> 8) & 0xFF) as u8);
    }
}
