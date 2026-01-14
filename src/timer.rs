use core::sync::atomic::{AtomicU64, Ordering};

use crate::{multitasker::yield_now, serial_println};
static TICKS: AtomicU64 = AtomicU64::new(0);

pub const TICKS_PER_SECOND: u64 = 1000;
const PIT_BASE_FREQUENCY: u64 = 1193180;

pub fn tick() {
    // Relaxed ordering is fine here because we're just incrementing a counter

    let old = TICKS.fetch_add(1, Ordering::Relaxed);
    if old % 1000 == 0 {
        serial_println!("Uptime: {} seconds", old / TICKS_PER_SECOND);
    }
}

pub fn get_uptime_ms() -> u64 {
    let current_ticks = TICKS.load(Ordering::Relaxed);
    // (ticks * 1000) / TICKS_PER_SECOND gives us milliseconds
    (current_ticks * 1000) / TICKS_PER_SECOND
}

pub fn sleep_ms(ms: u64) {
    let wake_at = get_uptime_ms() + ms;

    // Set the wake_at time for the current task
    if let Some(mut sched) = crate::multitasker::scheduler::SCHEDULER.try_lock() {
        if let Some(ref mut task) = sched.current_task {
            task.wake_at = wake_at;
        }
    }

    // Yield immediately so the Idle task can take over
    crate::multitasker::yield_now();
}
pub fn init_timer() {
    let divisor: u16 = (PIT_BASE_FREQUENCY / TICKS_PER_SECOND) as u16;

    unsafe {
        // Command port: Select Channel 0, Square Wave Mode
        core::arch::asm!("out 0x43, al", in("al") 0x36u8, options(nomem, nostack));

        // Data port: Send low byte then high byte of divisor
        core::arch::asm!("out 0x40, al", in("al") (divisor & 0xFF) as u8, options(nomem, nostack));
        core::arch::asm!("out 0x40, al", in("al") ((divisor >> 8) & 0xFF) as u8, options(nomem, nostack));
    }
}
