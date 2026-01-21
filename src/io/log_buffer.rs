use core::fmt::Write;
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

const LOG_BUFFER_SIZE: usize = 4096 * 100;

pub static DISPLAY_QUEUE: LogQueue = LogQueue::new();
pub static SERIAL_QUEUE: LogQueue = LogQueue::new();

pub struct LogQueue {
    buffer: [AtomicU8; LOG_BUFFER_SIZE],
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl LogQueue {
    pub const fn new() -> Self {
        // Create an array of AtomicU8s initialized to 0
        const ATOMIC_ZERO: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);
        Self {
            buffer: [ATOMIC_ZERO; LOG_BUFFER_SIZE],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    pub fn push_char(&self, c: u8) {
        let h = self.head.load(Ordering::Relaxed);
        let next = (h + 1) % LOG_BUFFER_SIZE;

        // If full, move the tail forward to make room (Overwrite Mode)
        if next == self.tail.load(Ordering::Acquire) {
            let t = self.tail.load(Ordering::Relaxed);
            self.tail
                .store((t + 1) % LOG_BUFFER_SIZE, Ordering::Release);
        }

        // Write the data
        self.buffer[h].store(c, Ordering::Release);
        self.head.store(next, Ordering::Release);
    }

    pub fn pop_char(&self) -> Option<u8> {
        let t = self.tail.load(Ordering::Relaxed);
        if t == self.head.load(Ordering::Acquire) {
            return None;
        }

        // Read the data
        let c = self.buffer[t].load(Ordering::Acquire);

        // Move tail forward
        self.tail
            .store((t + 1) % LOG_BUFFER_SIZE, Ordering::Release);
        Some(c)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogTarget {
    Display,
    Serial,
    Both,
}

/// The internal function called by all print macros.
/// It routes characters to the appropriate Atomic queues without locking hardware.
pub fn _log_print(args: core::fmt::Arguments, target: LogTarget) {
    struct DeferredWriter(LogTarget);

    impl Write for DeferredWriter {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            for b in s.bytes() {
                match self.0 {
                    LogTarget::Display => {
                        super::log_buffer::DISPLAY_QUEUE.push_char(b);
                    }
                    LogTarget::Serial => {
                        super::log_buffer::SERIAL_QUEUE.push_char(b);
                    }
                    LogTarget::Both => {
                        super::log_buffer::DISPLAY_QUEUE.push_char(b);
                        super::log_buffer::SERIAL_QUEUE.push_char(b);
                    }
                }
            }
            Ok(())
        }
    }

    let mut writer = DeferredWriter(target);
    let _ = writer.write_fmt(args);
}
